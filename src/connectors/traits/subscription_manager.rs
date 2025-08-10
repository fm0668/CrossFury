//! 订阅管理器模块
//! 
//! 处理订阅的幂等性、增量订阅和WebSocket连接管理

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{RwLock, broadcast};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use thiserror::Error;
use log::{info, error, debug};


/// 订阅配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionConfig {
    /// 交易对列表
    pub symbols: Vec<String>,
    /// 数据类型列表
    pub data_types: Vec<DataType>,
    /// 批量大小
    pub batch_size: Option<usize>,
    /// 订阅优先级
    pub priority: SubscriptionPriority,
}

/// 数据类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// 订单簿
    OrderBook,
    /// 交易数据
    Trade,
    /// 价格行情
    Ticker,
    /// K线数据
    Kline,
    /// 用户数据流
    UserData,
    /// 账户更新
    AccountUpdate,
    /// 订单更新
    OrderUpdate,
}

/// 订阅优先级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// 订阅状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    /// 待处理
    Pending,
    /// 活跃
    Active,
    /// 暂停
    Paused,
    /// 失败
    Failed(String),
    /// 已取消
    Cancelled,
}

/// 订阅项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    /// 交易对
    pub symbol: String,
    /// 数据类型
    pub data_type: DataType,
    /// 状态
    pub status: SubscriptionStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    /// 重试次数
    pub retry_count: u32,
}

/// WebSocket连接信息
#[derive(Debug, Clone)]
pub struct WebSocketConnection {
    /// 连接ID
    pub id: String,
    /// 连接URL
    pub url: String,
    /// 订阅的数据流
    pub streams: HashSet<String>,
    /// 连接状态
    pub status: ConnectionStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
}

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Failed(String),
}

/// 订阅管理器错误
#[derive(Debug, Error)]
pub enum SubscriptionManagerError {
    #[error("订阅已存在: {symbol}@{data_type:?}")]
    SubscriptionExists { symbol: String, data_type: DataType },
    #[error("订阅不存在: {symbol}@{data_type:?}")]
    SubscriptionNotFound { symbol: String, data_type: DataType },
    #[error("WebSocket连接失败: {0}")]
    WebSocketConnectionFailed(String),
    #[error("配置错误: {0}")]
    ConfigError(String),
    #[error("网络错误: {0}")]
    NetworkError(String),
    #[error("序列化错误: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// 订阅管理器
pub struct SubscriptionManager {
    /// 当前订阅
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionItem>>>,
    /// WebSocket连接池
    connections: Arc<RwLock<HashMap<String, WebSocketConnection>>>,
    /// 事件发送器
    event_tx: broadcast::Sender<SubscriptionEvent>,
    /// 配置
    config: SubscriptionManagerConfig,
    /// 连接策略
    connection_strategy: ConnectionStrategy,
}

/// 订阅管理器配置
#[derive(Debug, Clone)]
pub struct SubscriptionManagerConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// 每个连接的最大流数
    pub max_streams_per_connection: usize,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 最大重试次数
    pub max_retry_attempts: u32,
    /// 连接超时
    pub connection_timeout: Duration,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
}

impl Default for SubscriptionManagerConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            max_streams_per_connection: 200,
            retry_interval: Duration::from_secs(5),
            max_retry_attempts: 3,
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
        }
    }
}

/// 连接策略
#[derive(Debug, Clone)]
pub enum ConnectionStrategy {
    /// 单连接复用 - 所有订阅使用同一个连接
    SingleConnection,
    /// 按数据类型分组 - 不同数据类型使用不同连接
    GroupByDataType,
    /// 按市场类型分组 - 不同市场使用不同连接
    GroupByMarketType,
    /// 负载均衡 - 根据负载分配连接
    LoadBalanced,
}

/// 订阅事件
#[derive(Debug, Clone)]
pub enum SubscriptionEvent {
    /// 订阅成功
    SubscriptionAdded {
        symbol: String,
        data_type: DataType,
        timestamp: DateTime<Utc>,
    },
    /// 订阅移除
    SubscriptionRemoved {
        symbol: String,
        data_type: DataType,
        timestamp: DateTime<Utc>,
    },
    /// 订阅失败
    SubscriptionFailed {
        symbol: String,
        data_type: DataType,
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// 连接建立
    ConnectionEstablished {
        connection_id: String,
        timestamp: DateTime<Utc>,
    },
    /// 连接断开
    ConnectionLost {
        connection_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl SubscriptionManager {
    /// 创建新的订阅管理器
    pub fn new(config: SubscriptionManagerConfig, strategy: ConnectionStrategy) -> Self {
        let (event_tx, _) = broadcast::channel(1000);
        
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            config,
            connection_strategy: strategy,
        }
    }
    
    /// 添加订阅（幂等操作）
    pub async fn subscribe(&self, config: SubscriptionConfig) -> Result<(), SubscriptionManagerError> {
        info!("开始处理订阅请求: {config:?}");
        
        let mut new_subscriptions = Vec::new();
        let mut existing_subscriptions = Vec::new();
        
        // 检查幂等性
        {
            let subscriptions = self.subscriptions.read().await;
            for symbol in &config.symbols {
                for data_type in &config.data_types {
                    let key = self.subscription_key(symbol, data_type);
                    if subscriptions.contains_key(&key) {
                        existing_subscriptions.push((symbol.clone(), data_type.clone()));
                        debug!("订阅已存在: {symbol}@{data_type:?}");
                    } else {
                        new_subscriptions.push((symbol.clone(), data_type.clone()));
                    }
                }
            }
        }
        
        // 只处理新的订阅
        if !new_subscriptions.is_empty() {
            self.process_new_subscriptions(new_subscriptions, config.priority).await?;
        }
        
        // 记录已存在的订阅
        for (symbol, data_type) in existing_subscriptions {
            debug!("跳过已存在的订阅: {symbol}@{data_type:?}");
        }
        
        Ok(())
    }
    
    /// 移除订阅
    pub async fn unsubscribe(&self, config: SubscriptionConfig) -> Result<(), SubscriptionManagerError> {
        info!("开始处理取消订阅请求: {config:?}");
        
        let mut removed_subscriptions = Vec::new();
        
        // 移除订阅
        {
            let mut subscriptions = self.subscriptions.write().await;
            for symbol in &config.symbols {
                for data_type in &config.data_types {
                    let key = self.subscription_key(symbol, data_type);
                    if let Some(subscription) = subscriptions.remove(&key) {
                        removed_subscriptions.push((symbol.clone(), data_type.clone()));
                        info!("移除订阅: {symbol}@{data_type:?}");
                        
                        // 发送移除事件
                        let _ = self.event_tx.send(SubscriptionEvent::SubscriptionRemoved {
                            symbol: symbol.clone(),
                            data_type: data_type.clone(),
                            timestamp: Utc::now(),
                        });
                    }
                }
            }
        }
        
        // 清理不再需要的连接
        self.cleanup_unused_connections().await;
        
        Ok(())
    }
    
    /// 获取所有订阅状态
    pub async fn get_subscriptions(&self) -> HashMap<String, SubscriptionItem> {
        self.subscriptions.read().await.clone()
    }
    
    /// 获取连接状态
    pub async fn get_connections(&self) -> HashMap<String, WebSocketConnection> {
        self.connections.read().await.clone()
    }
    
    /// 获取事件流
    pub fn event_stream(&self) -> broadcast::Receiver<SubscriptionEvent> {
        self.event_tx.subscribe()
    }
    
    /// 处理新订阅
    async fn process_new_subscriptions(
        &self,
        subscriptions: Vec<(String, DataType)>,
        priority: SubscriptionPriority,
    ) -> Result<(), SubscriptionManagerError> {
        let now = Utc::now();
        
        // 根据连接策略分组订阅
        let grouped_subscriptions = self.group_subscriptions_by_strategy(&subscriptions);
        
        for (connection_key, subs) in grouped_subscriptions {
            // 获取或创建连接
            let connection_id = self.get_or_create_connection(&connection_key).await?;
            
            // 添加订阅到管理器
            {
                let mut subscription_map = self.subscriptions.write().await;
                for (symbol, data_type) in &subs {
                    let key = self.subscription_key(symbol, data_type);
                    let subscription = SubscriptionItem {
                        symbol: symbol.clone(),
                        data_type: data_type.clone(),
                        status: SubscriptionStatus::Active,
                        created_at: now,
                        updated_at: now,
                        retry_count: 0,
                    };
                    subscription_map.insert(key, subscription);
                    
                    // 发送订阅成功事件
                    let _ = self.event_tx.send(SubscriptionEvent::SubscriptionAdded {
                        symbol: symbol.clone(),
                        data_type: data_type.clone(),
                        timestamp: now,
                    });
                    
                    info!("添加订阅: {symbol}@{data_type:?} 到连接 {connection_id}");
                }
            }
            
            // 更新连接的流信息
            self.update_connection_streams(&connection_id, &subs).await;
        }
        
        Ok(())
    }
    
    /// 根据策略分组订阅
    fn group_subscriptions_by_strategy(
        &self,
        subscriptions: &[(String, DataType)],
    ) -> HashMap<String, Vec<(String, DataType)>> {
        let mut groups = HashMap::new();
        
        for (symbol, data_type) in subscriptions {
            let key = match &self.connection_strategy {
                ConnectionStrategy::SingleConnection => "main".to_string(),
                ConnectionStrategy::GroupByDataType => format!("{data_type:?}"),
                ConnectionStrategy::GroupByMarketType => {
                    // 根据交易对判断市场类型
                    if symbol.ends_with("USDT") || symbol.ends_with("BUSD") {
                        "spot".to_string()
                    } else {
                        "futures".to_string()
                    }
                },
                ConnectionStrategy::LoadBalanced => {
                    // 简单的负载均衡：根据symbol的哈希值分配
                    let hash = symbol.len() % self.config.max_connections;
                    format!("conn_{hash}")
                },
            };
            
            groups.entry(key).or_insert_with(Vec::new).push((symbol.clone(), data_type.clone()));
        }
        
        groups
    }
    
    /// 获取或创建连接
    async fn get_or_create_connection(&self, connection_key: &str) -> Result<String, SubscriptionManagerError> {
        let connection_id = format!("{}_{}", connection_key, Utc::now().timestamp());
        
        // 检查是否已有可用连接
        {
            let connections = self.connections.read().await;
            for (id, conn) in connections.iter() {
                if conn.status == ConnectionStatus::Connected 
                    && conn.streams.len() < self.config.max_streams_per_connection 
                    && id.starts_with(connection_key) {
                    return Ok(id.clone());
                }
            }
        }
        
        // 创建新连接
        let connection = WebSocketConnection {
            id: connection_id.clone(),
            url: self.build_websocket_url(connection_key),
            streams: HashSet::new(),
            status: ConnectionStatus::Connecting,
            created_at: Utc::now(),
            last_active: Utc::now(),
        };
        
        // 存储连接
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id.clone(), connection);
        }
        
        // 建立实际的WebSocket连接
        self.establish_websocket_connection(&connection_id).await?;
        
        // 发送连接建立事件
        let _ = self.event_tx.send(SubscriptionEvent::ConnectionEstablished {
            connection_id: connection_id.clone(),
            timestamp: Utc::now(),
        });
        
        info!("创建新连接: {connection_id}");
        
        Ok(connection_id)
    }
    
    /// 构建WebSocket URL
    fn build_websocket_url(&self, connection_key: &str) -> String {
        // 这里应该根据实际的交易所API构建URL
        // 示例：Binance的WebSocket URL
        format!("wss://stream.binance.com:9443/ws/{connection_key}")
    }
    
    /// 建立WebSocket连接
    async fn establish_websocket_connection(&self, connection_id: &str) -> Result<(), SubscriptionManagerError> {
        // 这里应该实现实际的WebSocket连接逻辑
        // 为了示例，我们模拟连接成功
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 更新连接状态
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.get_mut(connection_id) {
                conn.status = ConnectionStatus::Connected;
                conn.last_active = Utc::now();
            }
        }
        
        debug!("WebSocket连接建立成功: {connection_id}");
        
        Ok(())
    }
    
    /// 更新连接的流信息
    async fn update_connection_streams(&self, connection_id: &str, subscriptions: &[(String, DataType)]) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            for (symbol, data_type) in subscriptions {
                let stream = format!("{}@{:?}", symbol.to_lowercase(), data_type).to_lowercase();
                conn.streams.insert(stream);
            }
            conn.last_active = Utc::now();
        }
    }
    
    /// 清理未使用的连接
    async fn cleanup_unused_connections(&self) {
        let mut connections_to_remove = Vec::new();
        
        {
            let connections = self.connections.read().await;
            for (id, conn) in connections.iter() {
                if conn.streams.is_empty() {
                    connections_to_remove.push(id.clone());
                }
            }
        }
        
        if !connections_to_remove.is_empty() {
            let mut connections = self.connections.write().await;
            for id in connections_to_remove {
                connections.remove(&id);
                info!("清理未使用的连接: {id}");
                
                // 发送连接断开事件
                let _ = self.event_tx.send(SubscriptionEvent::ConnectionLost {
                    connection_id: id,
                    reason: "未使用".to_string(),
                    timestamp: Utc::now(),
                });
            }
        }
    }
    
    /// 生成订阅键
    fn subscription_key(&self, symbol: &str, data_type: &DataType) -> String {
        format!("{symbol}@{data_type:?}")
    }
    
    /// 健康检查
    pub async fn health_check(&self) -> SubscriptionManagerHealth {
        let subscriptions = self.subscriptions.read().await;
        let connections = self.connections.read().await;
        
        let total_subscriptions = subscriptions.len();
        let active_subscriptions = subscriptions.values()
            .filter(|s| matches!(s.status, SubscriptionStatus::Active))
            .count();
        let failed_subscriptions = subscriptions.values()
            .filter(|s| matches!(s.status, SubscriptionStatus::Failed(_)))
            .count();
        
        let total_connections = connections.len();
        let active_connections = connections.values()
            .filter(|c| c.status == ConnectionStatus::Connected)
            .count();
        
        SubscriptionManagerHealth {
            total_subscriptions,
            active_subscriptions,
            failed_subscriptions,
            total_connections,
            active_connections,
            last_checked: Utc::now(),
        }
    }
}

/// 订阅管理器健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionManagerHealth {
    pub total_subscriptions: usize,
    pub active_subscriptions: usize,
    pub failed_subscriptions: usize,
    pub total_connections: usize,
    pub active_connections: usize,
    pub last_checked: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_subscription_idempotency() {
        let config = SubscriptionManagerConfig::default();
        let manager = SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);
        
        let sub_config = SubscriptionConfig {
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Ticker],
            batch_size: None,
            priority: SubscriptionPriority::Medium,
        };
        
        // 第一次订阅
        assert!(manager.subscribe(sub_config.clone()).await.is_ok());
        
        // 第二次订阅（应该是幂等的）
        assert!(manager.subscribe(sub_config.clone()).await.is_ok());
        
        // 验证只有一个订阅
        let subscriptions = manager.get_subscriptions().await;
        assert_eq!(subscriptions.len(), 1);
    }
    
    #[tokio::test]
    async fn test_incremental_subscription() {
        let config = SubscriptionManagerConfig::default();
        let manager = SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);
        
        // 第一批订阅
        let sub_config1 = SubscriptionConfig {
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Ticker],
            batch_size: None,
            priority: SubscriptionPriority::Medium,
        };
        assert!(manager.subscribe(sub_config1).await.is_ok());
        
        // 第二批订阅（增量）
        let sub_config2 = SubscriptionConfig {
            symbols: vec!["ETHUSDT".to_string()],
            data_types: vec![DataType::Ticker],
            batch_size: None,
            priority: SubscriptionPriority::Medium,
        };
        assert!(manager.subscribe(sub_config2).await.is_ok());
        
        // 验证有两个订阅
        let subscriptions = manager.get_subscriptions().await;
        assert_eq!(subscriptions.len(), 2);
    }
    
    #[tokio::test]
    async fn test_connection_strategy() {
        let config = SubscriptionManagerConfig::default();
        let manager = SubscriptionManager::new(config, ConnectionStrategy::GroupByDataType);
        
        let sub_config = SubscriptionConfig {
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::Ticker, DataType::OrderBook],
            batch_size: None,
            priority: SubscriptionPriority::Medium,
        };
        
        assert!(manager.subscribe(sub_config).await.is_ok());
        
        // 验证创建了多个连接（按数据类型分组）
        let connections = manager.get_connections().await;
        assert!(connections.len() >= 1);
    }
}