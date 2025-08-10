//! WebSocket连接管理器
//! 
//! 提供高级的WebSocket连接管理功能，包括：
//! - 多stream复用策略
//! - 独立连接模式
//! - 连接池管理
//! - 连接质量监控
//! - 自动重连机制

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{RwLock, mpsc, broadcast},
    time::sleep,
};
use serde::{Serialize, Deserialize};
use thiserror::Error;
use chrono::{DateTime, Utc};
use log::{info, warn, error};

/// WebSocket连接管理器错误
#[derive(Debug, Error)]
pub enum WebSocketManagerError {
    #[error("连接失败: {0}")]
    ConnectionFailed(String),
    #[error("连接已存在: {0}")]
    ConnectionExists(String),
    #[error("连接不存在: {0}")]
    ConnectionNotFound(String),
    #[error("连接池已满")]
    PoolFull,
    #[error("消息发送失败: {0}")]
    MessageSendFailed(String),
    #[error("配置错误: {0}")]
    ConfigError(String),
}

/// 连接策略
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStrategy {
    /// 多路复用：在单个连接上处理多个数据流
    Multiplexed,
    /// 独立连接：每个数据类型使用独立连接
    Independent,
    /// 混合模式：根据数据类型智能选择
    Hybrid,
}

/// 数据流类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamType {
    /// 市场数据流
    MarketData,
    /// 用户数据流
    UserData,
    /// 订单簿数据流
    OrderBook,
    /// 交易数据流
    Trade,
    /// K线数据流
    Kline,
    /// Ticker数据流
    Ticker,
}

/// 连接质量指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionQuality {
    /// 延迟（毫秒）
    pub latency_ms: f64,
    /// 丢包率
    pub packet_loss_rate: f64,
    /// 稳定性评分 (0.0 - 1.0)
    pub stability_score: f64,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
    /// 连接时长
    pub uptime_seconds: u64,
    /// 重连次数
    pub reconnect_count: u32,
}

impl Default for ConnectionQuality {
    fn default() -> Self {
        Self {
            latency_ms: 0.0,
            packet_loss_rate: 0.0,
            stability_score: 1.0,
            last_updated: Utc::now(),
            uptime_seconds: 0,
            reconnect_count: 0,
        }
    }
}

/// WebSocket连接信息
#[derive(Debug)]
pub struct WebSocketConnection {
    /// 连接ID
    pub id: String,
    /// 连接URL
    pub url: String,
    /// 连接状态
    pub status: ConnectionStatus,
    /// 支持的数据流类型
    pub stream_types: Vec<StreamType>,
    /// 连接质量
    pub quality: ConnectionQuality,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
    /// 消息发送器
    pub message_tx: Option<mpsc::Sender<String>>,
    /// 消息接收器
    pub message_rx: Option<mpsc::Receiver<String>>,
}

/// 连接状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// 断开连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 重连中
    Reconnecting { attempt: u32, next_retry: DateTime<Utc> },
    /// 连接失败
    Failed { error: String, last_attempt: DateTime<Utc> },
}

/// WebSocket管理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketManagerConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// 每个连接的最大数据流数
    pub max_streams_per_connection: usize,
    /// 连接超时时间
    pub connection_timeout: Duration,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
    /// 重连间隔
    pub reconnect_interval: Duration,
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 连接质量检查间隔
    pub quality_check_interval: Duration,
    /// 连接策略
    pub strategy: ConnectionStrategy,
}

impl Default for WebSocketManagerConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            max_streams_per_connection: 200,
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(5),
            max_reconnect_attempts: 5,
            quality_check_interval: Duration::from_secs(60),
            strategy: ConnectionStrategy::Hybrid,
        }
    }
}

/// WebSocket连接管理器
pub struct WebSocketManager {
    /// 配置
    config: WebSocketManagerConfig,
    /// 连接池
    connections: Arc<RwLock<HashMap<String, WebSocketConnection>>>,
    /// 数据流到连接的映射
    stream_to_connection: Arc<RwLock<HashMap<StreamType, String>>>,
    /// 事件发送器
    event_tx: broadcast::Sender<WebSocketEvent>,
    /// 管理器状态
    is_running: Arc<RwLock<bool>>,
}

/// WebSocket事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSocketEvent {
    /// 连接建立
    Connected { connection_id: String, url: String },
    /// 连接断开
    Disconnected { connection_id: String, reason: String },
    /// 消息接收
    MessageReceived { connection_id: String, message: String },
    /// 连接错误
    Error { connection_id: String, error: String },
    /// 质量更新
    QualityUpdated { connection_id: String, quality: ConnectionQuality },
}

impl WebSocketManager {
    /// 创建新的WebSocket管理器
    pub fn new(config: WebSocketManagerConfig) -> Self {
        let (event_tx, _) = broadcast::channel(1000);
        
        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            stream_to_connection: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            is_running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 启动管理器
    pub async fn start(&self) -> Result<(), WebSocketManagerError> {
        let mut running = self.is_running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        
        info!("WebSocket管理器启动");
        
        // 启动质量监控任务
        self.start_quality_monitor().await;
        
        // 启动重连任务
        self.start_reconnect_monitor().await;
        
        Ok(())
    }
    
    /// 停止管理器
    pub async fn stop(&self) -> Result<(), WebSocketManagerError> {
        let mut running = self.is_running.write().await;
        if !*running {
            return Ok(());
        }
        *running = false;
        
        info!("WebSocket管理器停止");
        
        // 断开所有连接
        let connection_ids: Vec<String> = {
            let connections = self.connections.read().await;
            connections.keys().cloned().collect()
        };
        
        for connection_id in connection_ids {
            if let Err(e) = self.disconnect(&connection_id).await {
                warn!("断开连接 {connection_id} 失败: {e}");
            }
        }
        
        Ok(())
    }
    
    /// 创建连接
    pub async fn connect(
        &self,
        connection_id: String,
        url: String,
        stream_types: Vec<StreamType>,
    ) -> Result<(), WebSocketManagerError> {
        // 检查连接是否已存在
        {
            let connections = self.connections.read().await;
            if connections.contains_key(&connection_id) {
                return Err(WebSocketManagerError::ConnectionExists(connection_id));
            }
        }
        
        // 检查连接池是否已满
        {
            let connections = self.connections.read().await;
            if connections.len() >= self.config.max_connections {
                return Err(WebSocketManagerError::PoolFull);
            }
        }
        
        info!("创建WebSocket连接: {connection_id} -> {url}");
        
        // 创建消息通道
        let (message_tx, message_rx) = mpsc::channel(1000);
        
        // 创建连接对象
        let connection = WebSocketConnection {
            id: connection_id.clone(),
            url: url.clone(),
            status: ConnectionStatus::Connecting,
            stream_types: stream_types.clone(),
            quality: ConnectionQuality::default(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            message_tx: Some(message_tx),
            message_rx: Some(message_rx),
        };
        
        // 添加到连接池
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id.clone(), connection);
        }
        
        // 更新数据流映射
        {
            let mut stream_mapping = self.stream_to_connection.write().await;
            for stream_type in &stream_types {
                stream_mapping.insert(stream_type.clone(), connection_id.clone());
            }
        }
        
        // 启动实际的WebSocket连接（这里是模拟实现）
        self.start_websocket_connection(connection_id.clone(), url.clone()).await?;
        
        // 发送连接事件
        let _ = self.event_tx.send(WebSocketEvent::Connected {
            connection_id: connection_id.clone(),
            url,
        });
        
        Ok(())
    }
    
    /// 断开连接
    pub async fn disconnect(&self, connection_id: &str) -> Result<(), WebSocketManagerError> {
        info!("断开WebSocket连接: {connection_id}");
        
        // 获取连接信息
        let connection = {
            let mut connections = self.connections.write().await;
            connections.remove(connection_id)
        };
        
        if let Some(connection) = connection {
            // 清理数据流映射
            {
                let mut stream_mapping = self.stream_to_connection.write().await;
                for stream_type in &connection.stream_types {
                    stream_mapping.remove(stream_type);
                }
            }
            
            // 发送断开连接事件
            let _ = self.event_tx.send(WebSocketEvent::Disconnected {
                connection_id: connection_id.to_string(),
                reason: "主动断开".to_string(),
            });
            
            Ok(())
        } else {
            Err(WebSocketManagerError::ConnectionNotFound(connection_id.to_string()))
        }
    }
    
    /// 发送消息
    pub async fn send_message(
        &self,
        connection_id: &str,
        message: String,
    ) -> Result<(), WebSocketManagerError> {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(connection_id) {
            if let Some(tx) = &connection.message_tx {
                tx.send(message).await
                    .map_err(|e| WebSocketManagerError::MessageSendFailed(e.to_string()))?;
                Ok(())
            } else {
                Err(WebSocketManagerError::MessageSendFailed(
                    "消息发送器不可用".to_string()
                ))
            }
        } else {
            Err(WebSocketManagerError::ConnectionNotFound(connection_id.to_string()))
        }
    }
    
    /// 根据数据流类型获取连接
    pub async fn get_connection_for_stream(
        &self,
        stream_type: &StreamType,
    ) -> Option<String> {
        let stream_mapping = self.stream_to_connection.read().await;
        stream_mapping.get(stream_type).cloned()
    }
    
    /// 获取连接质量
    pub async fn get_connection_quality(
        &self,
        connection_id: &str,
    ) -> Option<ConnectionQuality> {
        let connections = self.connections.read().await;
        connections.get(connection_id).map(|conn| conn.quality.clone())
    }
    
    /// 获取所有连接状态
    pub async fn get_all_connections(&self) -> HashMap<String, ConnectionStatus> {
        let connections = self.connections.read().await;
        connections.iter()
            .map(|(id, conn)| (id.clone(), conn.status.clone()))
            .collect()
    }
    
    /// 获取事件接收器
    pub fn subscribe_events(&self) -> broadcast::Receiver<WebSocketEvent> {
        self.event_tx.subscribe()
    }
    
    /// 启动WebSocket连接（模拟实现）
    async fn start_websocket_connection(
        &self,
        connection_id: String,
        url: String,
    ) -> Result<(), WebSocketManagerError> {
        // 在实际实现中，这里会建立真实的WebSocket连接
        // 现在使用模拟实现
        
        let connections = self.connections.clone();
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            // 模拟连接建立延迟
            sleep(Duration::from_millis(100)).await;
            
            // 更新连接状态为已连接
            {
                let mut conns = connections.write().await;
                if let Some(conn) = conns.get_mut(&connection_id) {
                    conn.status = ConnectionStatus::Connected;
                    conn.last_active = Utc::now();
                }
            }
            
            // 模拟消息接收
            loop {
                sleep(Duration::from_millis(1000)).await;
                
                // 检查连接是否仍然存在
                let exists = {
                    let conns = connections.read().await;
                    conns.contains_key(&connection_id)
                };
                
                if !exists {
                    break;
                }
                
                // 模拟接收消息
                let mock_message = format!(
                    r#"{{"stream":"btcusdt@ticker","data":{{"s":"BTCUSDT","c":"50000.00"}},"connection_id":"{connection_id}"}}"#
                );
                
                let _ = event_tx.send(WebSocketEvent::MessageReceived {
                    connection_id: connection_id.clone(),
                    message: mock_message,
                });
                
                // 更新最后活跃时间
                {
                    let mut conns = connections.write().await;
                    if let Some(conn) = conns.get_mut(&connection_id) {
                        conn.last_active = Utc::now();
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// 启动质量监控任务
    async fn start_quality_monitor(&self) {
        let connections = self.connections.clone();
        let event_tx = self.event_tx.clone();
        let is_running = self.is_running.clone();
        let check_interval = self.config.quality_check_interval;
        
        tokio::spawn(async move {
            while *is_running.read().await {
                sleep(check_interval).await;
                
                let connection_ids: Vec<String> = {
                    let conns = connections.read().await;
                    conns.keys().cloned().collect()
                };
                
                for connection_id in connection_ids {
                    // 计算连接质量
                    let quality = Self::calculate_connection_quality(&connection_id).await;
                    
                    // 更新连接质量
                    {
                        let mut conns = connections.write().await;
                        if let Some(conn) = conns.get_mut(&connection_id) {
                            conn.quality = quality.clone();
                        }
                    }
                    
                    // 发送质量更新事件
                    let _ = event_tx.send(WebSocketEvent::QualityUpdated {
                        connection_id,
                        quality,
                    });
                }
            }
        });
    }
    
    /// 启动重连监控任务
    async fn start_reconnect_monitor(&self) {
        let connections = self.connections.clone();
        let is_running = self.is_running.clone();
        let reconnect_interval = self.config.reconnect_interval;
        let max_attempts = self.config.max_reconnect_attempts;
        
        tokio::spawn(async move {
            while *is_running.read().await {
                sleep(reconnect_interval).await;
                
                let failed_connections: Vec<(String, String)> = {
                    let conns = connections.read().await;
                    conns.iter()
                        .filter_map(|(id, conn)| {
                            if let ConnectionStatus::Failed { .. } = conn.status {
                                Some((id.clone(), conn.url.clone()))
                            } else {
                                None
                            }
                        })
                        .collect()
                };
                
                for (connection_id, url) in failed_connections {
                    // 检查重连次数
                    let should_reconnect = {
                        let conns = connections.read().await;
                        if let Some(conn) = conns.get(&connection_id) {
                            conn.quality.reconnect_count < max_attempts
                        } else {
                            false
                        }
                    };
                    
                    if should_reconnect {
                        info!("尝试重连: {connection_id}");
                        // 这里可以实现重连逻辑
                        // 暂时只更新状态
                        {
                            let mut conns = connections.write().await;
                            if let Some(conn) = conns.get_mut(&connection_id) {
                                conn.status = ConnectionStatus::Reconnecting {
                                    attempt: conn.quality.reconnect_count + 1,
                                    next_retry: Utc::now() + chrono::Duration::seconds(reconnect_interval.as_secs() as i64),
                                };
                                conn.quality.reconnect_count += 1;
                            }
                        }
                    }
                }
            }
        });
    }
    
    /// 计算连接质量（模拟实现）
    async fn calculate_connection_quality(connection_id: &str) -> ConnectionQuality {
        // 在实际实现中，这里会计算真实的连接质量指标
        // 现在使用模拟数据
        ConnectionQuality {
            latency_ms: 50.0 + (rand::random::<f64>() * 100.0),
            packet_loss_rate: rand::random::<f64>() * 0.01,
            stability_score: 0.9 + (rand::random::<f64>() * 0.1),
            last_updated: Utc::now(),
            uptime_seconds: 3600, // 模拟1小时运行时间
            reconnect_count: 0,
        }
    }
}

/// 连接策略选择器
pub struct ConnectionStrategySelector;

impl ConnectionStrategySelector {
    /// 根据数据流类型选择最佳连接策略
    pub fn select_strategy(
        stream_types: &[StreamType],
        current_connections: usize,
        max_connections: usize,
    ) -> ConnectionStrategy {
        // 如果只有一种数据流类型，使用复用策略
        if stream_types.len() == 1 {
            return ConnectionStrategy::Multiplexed;
        }
        
        // 如果连接数接近上限，使用复用策略
        if current_connections >= max_connections * 8 / 10 {
            return ConnectionStrategy::Multiplexed;
        }
        
        // 如果包含用户数据流，建议使用独立连接
        if stream_types.contains(&StreamType::UserData) {
            return ConnectionStrategy::Independent;
        }
        
        // 默认使用混合策略
        ConnectionStrategy::Hybrid
    }
    
    /// 判断是否应该创建新连接
    pub fn should_create_new_connection(
        stream_type: &StreamType,
        strategy: &ConnectionStrategy,
        existing_connections: &HashMap<StreamType, String>,
    ) -> bool {
        match strategy {
            ConnectionStrategy::Multiplexed => {
                // 复用策略：尽量使用现有连接
                existing_connections.is_empty()
            },
            ConnectionStrategy::Independent => {
                // 独立策略：每种数据流使用独立连接
                !existing_connections.contains_key(stream_type)
            },
            ConnectionStrategy::Hybrid => {
                // 混合策略：根据数据流类型决定
                match stream_type {
                    StreamType::UserData => !existing_connections.contains_key(stream_type),
                    _ => existing_connections.is_empty(),
                }
            },
        }
    }
}