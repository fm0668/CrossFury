//! 现代化Binance连接器实现示例
//! 
//! 展示如何使用现代化的trait设计实现具体的连接器

use async_trait::async_trait;
use std::{
    sync::Arc,
    time::Duration,
    collections::HashMap,
};
use tokio::sync::{RwLock, mpsc, broadcast};
use serde::{Serialize, Deserialize};
use serde_json;
use thiserror::Error;
use chrono::Utc;

use super::modern::*;
use super::subscription_manager::{
    SubscriptionManager, SubscriptionConfig as SubMgrConfig, SubscriptionManagerConfig,
    ConnectionStrategy, SubscriptionPriority as SubMgrPriority
};
use super::websocket_manager::{
    WebSocketManager, WebSocketManagerConfig, ConnectionStrategy as WSConnectionStrategy,
    StreamType, WebSocketEvent,
};
use crate::types::*;
use crate::types::common::{ExchangeType, MarketType};
use crate::types::market_data::{Ticker, Kline};
use super::batch_writer::{BatchProcessor, BatchWriterConfig, FileBatchWriter};

/// 现代化Binance连接器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernBinanceConfig {
    /// API密钥
    pub api_key: Option<String>,
    /// 密钥
    pub secret_key: Option<String>,
    /// 是否使用测试网
    pub testnet: bool,
    /// WebSocket URL
    pub websocket_url: String,
    /// REST API URL
    pub rest_api_url: String,
    /// 连接超时
    pub connection_timeout: Duration,
    /// 重连间隔
    pub reconnect_interval: Duration,
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
    /// 批量订阅大小
    pub batch_size: usize,
    /// 通道缓冲区大小
    pub channel_buffer_size: usize,
}

impl Default for ModernBinanceConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            secret_key: None,
            testnet: false,
            websocket_url: "wss://stream.binance.com:9443/ws".to_string(),
            rest_api_url: "https://api.binance.com".to_string(),
            connection_timeout: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(5),
            max_reconnect_attempts: 5,
            heartbeat_interval: Duration::from_secs(30),
            batch_size: 10,
            channel_buffer_size: 1000,
        }
    }
}

/// Binance配置错误
#[derive(Debug, Error)]
pub enum BinanceConfigError {
    #[error("无效的WebSocket URL: {0}")]
    InvalidWebSocketUrl(String),
    #[error("无效的REST API URL: {0}")]
    InvalidRestApiUrl(String),
    #[error("连接超时必须大于0")]
    InvalidConnectionTimeout,
    #[error("重连间隔必须大于0")]
    InvalidReconnectInterval,
    #[error("批量大小必须大于0")]
    InvalidBatchSize,
}

impl ConnectorConfig for ModernBinanceConfig {
    type Error = BinanceConfigError;
    
    fn validate(&self) -> Result<(), Self::Error> {
        // 验证WebSocket URL
        if !self.websocket_url.starts_with("wss://") && !self.websocket_url.starts_with("ws://") {
            return Err(BinanceConfigError::InvalidWebSocketUrl(self.websocket_url.clone()));
        }
        
        // 验证REST API URL
        if !self.rest_api_url.starts_with("https://") && !self.rest_api_url.starts_with("http://") {
            return Err(BinanceConfigError::InvalidRestApiUrl(self.rest_api_url.clone()));
        }
        
        // 验证超时设置
        if self.connection_timeout.is_zero() {
            return Err(BinanceConfigError::InvalidConnectionTimeout);
        }
        
        if self.reconnect_interval.is_zero() {
            return Err(BinanceConfigError::InvalidReconnectInterval);
        }
        
        // 验证批量大小
        if self.batch_size == 0 {
            return Err(BinanceConfigError::InvalidBatchSize);
        }
        
        Ok(())
    }
    
    fn connection_timeout(&self) -> Duration {
        self.connection_timeout
    }
    
    fn reconnect_interval(&self) -> Duration {
        self.reconnect_interval
    }
    
    fn max_reconnect_attempts(&self) -> u32 {
        self.max_reconnect_attempts
    }
}

/// 现代化Binance连接器错误
#[derive(Debug, Error)]
pub enum ModernBinanceError {
    #[error("连接器错误: {0}")]
    Connector(#[from] ConnectorError),
    #[error("配置错误: {0}")]
    Config(#[from] BinanceConfigError),
    #[error("WebSocket错误: {0}")]
    WebSocket(String),
    #[error("API错误: {0}")]
    Api(String),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("网络错误: {0}")]
    Network(String),
    #[error("认证错误: {0}")]
    Authentication(String),
    #[error("限流错误: {0}")]
    RateLimit(String),
    #[error("未初始化")]
    NotInitialized,
}

/// Binance市场数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinanceMarketData {
    OrderBook(StandardizedOrderBook),
    Trade(StandardizedTrade),
    Ticker(Ticker),
    Kline(Kline),
}

/// Binance用户数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinanceUserData {
    AccountUpdate(AccountBalance),
    OrderUpdate(OrderStatus),
    ExecutionReport(StandardizedTrade),
}

/// 现代化Binance连接器
pub struct ModernBinanceConnector {
    /// 配置
    config: ModernBinanceConfig,
    /// 连接状态
    status: Arc<RwLock<ModernConnectionStatus>>,
    /// 指标
    metrics: Arc<RwLock<ConnectorMetrics>>,
    /// 市场数据发送器
    market_data_tx: Option<mpsc::Sender<BinanceMarketData>>,
    /// 用户数据发送器
    user_data_tx: Option<mpsc::Sender<BinanceUserData>>,
    /// 事件发送器
    event_tx: broadcast::Sender<SystemEvent>,
    /// 订阅管理器
    subscription_manager: Arc<SubscriptionManager>,
    /// WebSocket管理器
    websocket_manager: Arc<WebSocketManager>,
    /// 重连策略
    reconnect_strategy: ReconnectStrategy,
    /// 指标收集器
    metrics_collector: Option<Arc<dyn MetricsCollector>>,
    /// 市场数据批量处理器
    market_data_batch_processor: Option<Arc<BatchProcessor<BinanceMarketData, FileBatchWriter>>>,
    /// 用户数据批量处理器
    user_data_batch_processor: Option<Arc<BatchProcessor<BinanceUserData, FileBatchWriter>>>,
}

impl Default for ModernBinanceConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl ModernBinanceConnector {
    /// 创建新的现代化Binance连接器
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1000);
        
        // 创建订阅管理器配置
        let subscription_config = SubscriptionManagerConfig {
            max_connections: 5,
            max_streams_per_connection: 200,
            retry_interval: Duration::from_secs(5),
            max_retry_attempts: 3,
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
        };
        
        // 创建订阅管理器，使用负载均衡策略
        let subscription_manager = Arc::new(SubscriptionManager::new(
            subscription_config,
            ConnectionStrategy::LoadBalanced,
        ));
        
        // 创建WebSocket管理器配置
        let ws_config = WebSocketManagerConfig {
            max_connections: 5,
            max_streams_per_connection: 200,
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(5),
            max_reconnect_attempts: 5,
            quality_check_interval: Duration::from_secs(60),
            strategy: WSConnectionStrategy::Hybrid,
        };
        
        let websocket_manager = Arc::new(WebSocketManager::new(ws_config));
        
        Self {
            config: ModernBinanceConfig::default(),
            status: Arc::new(RwLock::new(ModernConnectionStatus::Disconnected)),
            metrics: Arc::new(RwLock::new(ConnectorMetrics::default())),
            market_data_tx: None,
            user_data_tx: None,
            event_tx,
            subscription_manager,
            websocket_manager,
            reconnect_strategy: ReconnectStrategy::ExponentialBackoff {
                initial_interval: Duration::from_secs(1),
                max_interval: Duration::from_secs(60),
                multiplier: 2.0,
                max_attempts: Some(5),
            },
            metrics_collector: None,
            market_data_batch_processor: None,
            user_data_batch_processor: None,
        }
    }
    
    /// 设置重连策略
    pub fn with_reconnect_strategy(mut self, strategy: ReconnectStrategy) -> Self {
        self.reconnect_strategy = strategy;
        self
    }
    
    /// 设置指标收集器
    pub fn with_metrics_collector(mut self, collector: Arc<dyn MetricsCollector>) -> Self {
        self.metrics_collector = Some(collector);
        self
    }
    
    /// 更新连接状态
    async fn update_status(&self, new_status: ModernConnectionStatus) {
        let mut status = self.status.write().await;
        *status = new_status.clone();
        
        // 记录连接事件
        if let Some(collector) = &self.metrics_collector {
            let event = match new_status {
                ModernConnectionStatus::Connected { .. } => ConnectionEvent::Connected,
                ModernConnectionStatus::Disconnected => ConnectionEvent::Disconnected,
                ModernConnectionStatus::Reconnecting { .. } => ConnectionEvent::Reconnecting,
                ModernConnectionStatus::Failed { error, .. } => ConnectionEvent::Failed(error),
                _ => return,
            };
            collector.record_connection_event(self.name(), event).await;
        }
    }
    
    /// 更新指标
    async fn update_metrics<F>(&self, updater: F)
    where
        F: FnOnce(&mut ConnectorMetrics),
    {
        let mut metrics = self.metrics.write().await;
        updater(&mut metrics);
        
        // 发送指标到收集器
        if let Some(collector) = &self.metrics_collector {
            collector.update_metrics(self.name(), &metrics).await;
        }
    }
    
    /// 发送系统事件
    async fn send_event(&self, event: SystemEvent) {
        let _ = self.event_tx.send(event);
    }
    
    /// 启动事件处理任务
    async fn start_event_processing(&self) -> Result<(), ModernBinanceError> {
        let mut event_rx = self.websocket_manager.subscribe_events();
        let market_batch_processor = self.market_data_batch_processor.clone();
        let user_batch_processor = self.user_data_batch_processor.clone();
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            while let Ok(ws_event) = event_rx.recv().await {
                match ws_event {
                    WebSocketEvent::MessageReceived { connection_id, message } => {
                        // 解析WebSocket消息
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&message) {
                            if let Some(stream) = parsed.get("stream").and_then(|s| s.as_str()) {
                                if stream.contains("@ticker") {
                                    // 处理ticker数据
                                    if let Some(data) = parsed.get("data") {
                                        if let Ok(ticker) = serde_json::from_value::<Ticker>(data.clone()) {
                                            let market_data = BinanceMarketData::Ticker(ticker);
                                            if let Some(processor) = &market_batch_processor {
                                                if let Err(e) = processor.add(market_data).await {
                                                    log::warn!("批量处理器添加市场数据失败: {e}");
                                                }
                                            }
                                        }
                                    }
                                } else if stream.contains("@user") {
                                    // 处理用户数据
                                    if let Some(data) = parsed.get("data") {
                                        // 这里可以根据具体的用户数据类型进行解析
                                        // 暂时使用模拟数据
                                        if let Some(processor) = &user_batch_processor {
                                            // 创建模拟的用户数据
                                            // 在实际实现中，这里应该根据消息类型解析具体的用户数据
                                        }
                                    }
                                }
                            }
                        }
                    },
                    WebSocketEvent::Connected { connection_id, url } => {
                        log::info!("WebSocket连接已建立: {connection_id} -> {url}");
                        
                        // 发送连接事件
                        let _ = event_tx.send(SystemEvent::ConnectorConnected {
                            connector_id: connection_id.clone(),
                            exchange: ExchangeType::Binance,
                            market_type: MarketType::Spot,
                        });
                    },
                    WebSocketEvent::Disconnected { connection_id, reason } => {
                        log::warn!("WebSocket连接已断开: {connection_id} - {reason}");
                        
                        // 发送断开连接事件
                        let _ = event_tx.send(SystemEvent::ConnectorDisconnected {
                            connector_id: connection_id.clone(),
                            exchange: ExchangeType::Binance,
                            market_type: MarketType::Spot,
                        });
                    },
                    WebSocketEvent::Error { connection_id, error } => {
                        log::error!("WebSocket连接错误: {connection_id} - {error}");
                    },
                    WebSocketEvent::QualityUpdated { connection_id, quality } => {
                        log::debug!("连接质量更新: {} - 延迟: {:.2}ms, 稳定性: {:.2}", 
                                   connection_id, quality.latency_ms, quality.stability_score);
                    },
                }
            }
        });
        
        Ok(())
    }
}

#[async_trait]
impl ModernExchangeConnector for ModernBinanceConnector {
    type Config = ModernBinanceConfig;
    type Error = ModernBinanceError;
    type MarketData = BinanceMarketData;
    type UserData = BinanceUserData;
    
    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }
    
    fn market_type(&self) -> MarketType {
        MarketType::Spot
    }
    
    fn name(&self) -> &str {
        "modern_binance_spot"
    }
    
    fn version(&self) -> &str {
        "2.0.0"
    }
    
    fn supported_features(&self) -> Vec<ConnectorFeature> {
        vec![
            ConnectorFeature::MarketData,
            ConnectorFeature::UserDataStream,
            ConnectorFeature::SpotTrading,
            ConnectorFeature::BatchSubscription,
            ConnectorFeature::AutoReconnect,
            ConnectorFeature::Heartbeat,
            ConnectorFeature::Compression,
            ConnectorFeature::IncrementalUpdates,
        ]
    }
    
    async fn initialize(&mut self, config: Self::Config) -> Result<(), Self::Error> {
        // 验证配置
        config.validate()?;
        
        // 保存配置
        self.config = config;
        
        // 创建数据流通道
        let (market_tx, _) = mpsc::channel(1000);
        let (user_tx, _) = mpsc::channel(1000);
        
        self.market_data_tx = Some(market_tx);
        self.user_data_tx = Some(user_tx);
        
        // 创建批量写入器配置
        let batch_config = BatchWriterConfig {
            batch_size: self.config.batch_size,
            flush_interval: Duration::from_millis(500),
            max_buffer_size: self.config.channel_buffer_size,
            write_timeout: Duration::from_secs(5),
        };
        
        // 创建市场数据批量写入器
        let market_data_writer = FileBatchWriter::new(
            "data/market_data.jsonl".to_string(),
            "binance_market_data".to_string(),
        );
        let market_data_processor = Arc::new(BatchProcessor::new(batch_config.clone(), market_data_writer));
        market_data_processor.start().await
            .map_err(|e| ModernBinanceError::Connector(ConnectorError::InitializationFailed(e.to_string())))?;
        self.market_data_batch_processor = Some(market_data_processor);
        
        // 创建用户数据批量写入器
        let user_data_writer = FileBatchWriter::new(
            "data/user_data.jsonl".to_string(),
            "binance_user_data".to_string(),
        );
        let user_data_processor = Arc::new(BatchProcessor::new(batch_config, user_data_writer));
        user_data_processor.start().await
            .map_err(|e| ModernBinanceError::Connector(ConnectorError::InitializationFailed(e.to_string())))?;
        self.user_data_batch_processor = Some(user_data_processor);
        
        // 发送初始化事件
        self.send_event(SystemEvent::ConnectorInitialized {
            connector_id: self.name().to_string(),
            exchange: ExchangeType::Binance,
            market_type: MarketType::Spot,
        }).await;
        
        Ok(())
    }
    
    async fn start(&mut self) -> Result<(), Self::Error> {
        self.connect().await
    }
    
    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.disconnect().await
    }
    
    async fn connect(&mut self) -> Result<(), Self::Error> {
        // 当设置连接中状态时，补充 attempt 字段
        self.update_status(ModernConnectionStatus::Connecting { attempt: 0, started_at: chrono::Utc::now() }).await;
        
        // 启动WebSocket管理器
        self.websocket_manager.start().await
            .map_err(|e| ModernBinanceError::Connector(ConnectorError::ConnectionFailed(e.to_string())))?;
        
        // 创建市场数据连接
        let market_data_url = format!("{}/ws/!ticker@arr", self.config.websocket_url);
        let market_streams = vec![StreamType::MarketData, StreamType::Ticker];
        
        self.websocket_manager.connect(
            "market_data".to_string(),
            market_data_url,
            market_streams,
        ).await
            .map_err(|e| ModernBinanceError::Connector(ConnectorError::ConnectionFailed(e.to_string())))?;
        
        // 创建用户数据连接（如果需要）
        if self.config.api_key.is_some() {
            let user_data_url = format!("{}/ws", self.config.websocket_url);
            let user_streams = vec![StreamType::UserData];
            
            self.websocket_manager.connect(
                "user_data".to_string(),
                user_data_url,
                user_streams,
            ).await
                .map_err(|e| ModernBinanceError::Connector(ConnectorError::ConnectionFailed(e.to_string())))?;
        }
        
        // 启动事件处理任务
        self.start_event_processing().await?;
        
        // 模拟连接建立时间
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 更新状态为已连接
        self.update_status(ModernConnectionStatus::Connected {
            connected_at: Utc::now(),
            last_heartbeat: Utc::now(),
            attempt: 0,
        }).await;
        
        // 更新指标
        self.update_metrics(|metrics| {
            metrics.connection_established_at = Some(Utc::now());
        }).await;
        
        // 发送连接事件
        self.send_event(SystemEvent::ConnectorConnected {
            connector_id: self.name().to_string(),
            exchange: ExchangeType::Binance,
            market_type: MarketType::Spot,
        }).await;
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        // 停止批量处理器
        if let Some(processor) = &self.market_data_batch_processor {
            processor.flush().await
                .map_err(|e| ModernBinanceError::Connector(ConnectorError::DisconnectionFailed(e.to_string())))?;
            processor.stop().await;
        }
        
        if let Some(processor) = &self.user_data_batch_processor {
            processor.flush().await
                .map_err(|e| ModernBinanceError::Connector(ConnectorError::DisconnectionFailed(e.to_string())))?;
            processor.stop().await;
        }
        
        // 断开所有WebSocket连接
        let connections = self.websocket_manager.get_all_connections().await;
        for connection_id in connections.keys() {
            if let Err(e) = self.websocket_manager.disconnect(connection_id).await {
                log::warn!("断开WebSocket连接 {connection_id} 失败: {e}");
            }
        }
        
        // 停止WebSocket管理器
        if let Err(e) = self.websocket_manager.stop().await {
            log::warn!("停止WebSocket管理器失败: {e}");
        }
        
        // 更新状态为断开连接
        self.update_status(ModernConnectionStatus::Disconnected).await;
        
        // 清理订阅管理器中的所有订阅
        let subscriptions = self.subscription_manager.get_subscriptions().await;
        for (_, item) in subscriptions {
            let unsubscribe_config = SubMgrConfig {
                symbols: vec![item.symbol],
                data_types: vec![item.data_type],
                batch_size: None,
                priority: SubMgrPriority::Medium,
            };
            let _ = self.subscription_manager.unsubscribe(unsubscribe_config).await;
        }
        
        // 清理批量处理器引用
        self.market_data_batch_processor = None;
        self.user_data_batch_processor = None;
        
        // 发送断开连接事件
        self.send_event(SystemEvent::ConnectorDisconnected {
            connector_id: self.name().to_string(),
            exchange: ExchangeType::Binance,
            market_type: MarketType::Spot,
        }).await;
        
        Ok(())
    }
    
    async fn connection_status(&self) -> ModernConnectionStatus {
        self.status.read().await.clone()
    }
    
    async fn subscribe(&mut self, config: SubMgrConfig) -> Result<(), Self::Error> {
        // 检查连接状态
        if !self.is_healthy().await {
            return Err(ModernBinanceError::Connector(ConnectorError::ConnectionFailed(
                "连接器未连接".to_string()
            )));
        }
        
        // 克隆symbols以避免借用移动问题
        let symbols = config.symbols.clone();
        
        // 使用订阅管理器处理订阅
        self.subscription_manager.subscribe(config).await
            .map_err(|e| ModernBinanceError::Connector(ConnectorError::SubscriptionFailed(e.to_string())))?;
        
        // 发送订阅事件
        self.send_event(SystemEvent::Subscription {
            exchange: ExchangeType::Binance,
            market_type: MarketType::Spot,
            symbol: symbols.join(","),
            subscribed: true,
            timestamp: std::time::SystemTime::now(),
        }).await;
        
        Ok(())
    }
    
    async fn unsubscribe(&mut self, config: SubMgrConfig) -> Result<(), Self::Error> {
        // 克隆symbols以避免借用移动问题
        let symbols = config.symbols.clone();
        
        // 使用订阅管理器处理取消订阅
        self.subscription_manager.unsubscribe(config).await
            .map_err(|e| ModernBinanceError::Connector(ConnectorError::SubscriptionFailed(e.to_string())))?;
        
        // 发送取消订阅事件
        self.send_event(SystemEvent::Subscription {
            exchange: ExchangeType::Binance,
            market_type: MarketType::Spot,
            symbol: symbols.join(","),
            subscribed: false,
            timestamp: std::time::SystemTime::now(),
        }).await;
        
        Ok(())
    }
    
    async fn subscription_status(&self) -> HashMap<String, SubscriptionStatus> {
        // 从订阅管理器获取订阅状态并转换为连接器的SubscriptionStatus类型
        let manager_subscriptions = self.subscription_manager.get_subscriptions().await;
        let mut result = HashMap::new();
        
        for (key, item) in manager_subscriptions {
            let status = match item.status {
                super::subscription_manager::SubscriptionStatus::Pending => SubscriptionStatus::Pending,
                super::subscription_manager::SubscriptionStatus::Active => SubscriptionStatus::Active,
                super::subscription_manager::SubscriptionStatus::Paused => SubscriptionStatus::Paused,
                super::subscription_manager::SubscriptionStatus::Failed(msg) => SubscriptionStatus::Failed(msg),
                super::subscription_manager::SubscriptionStatus::Cancelled => SubscriptionStatus::Cancelled,
            };
            result.insert(key, status);
        }
        
        result
    }
    
    fn market_data_stream(&self) -> Option<mpsc::Receiver<Self::MarketData>> {
        // 在实际实现中，这里应该返回一个新的接收器
        // 这里为了简化，返回None
        None
    }
    
    fn user_data_stream(&self) -> Option<mpsc::Receiver<Self::UserData>> {
        // 在实际实现中，这里应该返回一个新的接收器
        // 这里为了简化，返回None
        None
    }
    
    fn event_stream(&self) -> broadcast::Receiver<SystemEvent> {
        self.event_tx.subscribe()
    }
    
    async fn orderbook_snapshot(&self, symbol: &str) -> Result<Option<StandardizedOrderBook>, Self::Error> {
        // 模拟获取订单簿快照
        // 在实际实现中，这里应该从缓存或API获取数据
        Ok(None)
    }
    
    async fn recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Result<Vec<StandardizedTrade>, Self::Error> {
        // 模拟获取最近交易
        // 在实际实现中，这里应该从缓存或API获取数据
        Ok(Vec::new())
    }
    
    async fn metrics(&self) -> ConnectorMetrics {
        self.metrics.read().await.clone()
    }
    
    async fn health_check(&self) -> Result<HealthCheckResult, Self::Error> {
        let status = self.connection_status().await;
        let metrics = self.metrics().await;
        
        let healthy = matches!(status, ModernConnectionStatus::Connected { .. });
        let mut details = HashMap::new();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        
        // 检查连接状态
        details.insert("connection_status".to_string(), format!("{status:?}"));
        
        // 检查指标
        if metrics.error_count > 10 {
            warnings.push(format!("错误计数较高: {}", metrics.error_count));
        }
        
        if metrics.avg_latency_ms > 1000.0 {
            warnings.push(format!("延迟较高: {:.2}ms", metrics.avg_latency_ms));
        }
        
        if !healthy {
            errors.push("连接器未连接".to_string());
        }
        
        Ok(HealthCheckResult {
            healthy,
            checked_at: Utc::now(),
            latency_ms: Some(metrics.avg_latency_ms),
            details,
            warnings,
            errors,
        })
    }
    
    async fn connection_quality(&self) -> Result<ConnectionQuality, Self::Error> {
        let metrics = self.metrics().await;
        let status = self.connection_status().await;
        
        let quality = match status {
            ModernConnectionStatus::Connected { .. } => {
                ConnectionQuality {
                    latency_ms: metrics.avg_latency_ms,
                    packet_loss_rate: 0.0, // 需要实际计算
                    stability_score: if metrics.reconnect_count == 0 { 1.0 } else { 0.8 },
                    last_updated: Utc::now(),
                }
            },
            _ => {
                ConnectionQuality {
                    latency_ms: 1000.0,
                    packet_loss_rate: 1.0,
                    stability_score: 0.0,
                    last_updated: Utc::now(),
                }
            }
        };
        
        Ok(quality)
    }
    
    fn config(&self) -> &Self::Config {
        &self.config
    }
    
    async fn update_config(&mut self, config: Self::Config) -> Result<(), Self::Error> {
        // 验证新配置
        config.validate()?;
        
        // 如果连接器正在运行，需要重新连接
        let was_connected = self.is_healthy().await;
        
        if was_connected {
            self.disconnect().await?;
        }
        
        // 更新配置
        self.config = config;
        
        if was_connected {
            self.connect().await?;
        }
        
        Ok(())
    }
}

/// 现代化Binance连接器构建器
pub struct ModernBinanceConnectorBuilder {
    config: Option<ModernBinanceConfig>,
    reconnect_strategy: Option<ReconnectStrategy>,
    metrics_collector: Option<Arc<dyn MetricsCollector>>,
}

impl ModernBinanceConnectorBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            reconnect_strategy: None,
            metrics_collector: None,
        }
    }
}

impl ConnectorBuilder<ModernBinanceConnector> for ModernBinanceConnectorBuilder {
    type Error = ModernBinanceError;
    
    fn with_config(mut self, config: ModernBinanceConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    fn with_reconnect_strategy(mut self, strategy: ReconnectStrategy) -> Self {
        self.reconnect_strategy = Some(strategy);
        self
    }
    
    fn with_metrics_collector(mut self, collector: Arc<dyn MetricsCollector>) -> Self {
        self.metrics_collector = Some(collector);
        self
    }
    
    fn build(self) -> Result<ModernBinanceConnector, Self::Error> {
        let mut connector = ModernBinanceConnector::new();
        
        if let Some(config) = self.config {
            connector.config = config;
        }
        
        if let Some(strategy) = self.reconnect_strategy {
            connector.reconnect_strategy = strategy;
        }
        
        if let Some(collector) = self.metrics_collector {
            connector.metrics_collector = Some(collector);
        }
        
        Ok(connector)
    }
}

impl Default for ModernBinanceConnectorBuilder {
    fn default() -> Self {
        Self::new()
    }
}