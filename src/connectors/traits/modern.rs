//! 现代化连接器trait设计
//! 
//! 基于Rust最佳实践重新设计连接器接口，提供更好的类型安全、错误处理和扩展性

use async_trait::async_trait;
use std::{
    time::Duration,
    collections::HashMap,
    sync::Arc,
};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use tokio::sync::{mpsc, broadcast};

use crate::types::common::{ExchangeType, MarketType};
use crate::types::config::{BatchSubscriptionResult, SubscriptionStatus, ConnectionQuality};
use crate::types::events::SystemEvent;
use crate::types::market_data::{StandardizedOrderBook, StandardizedTrade};
use crate::{OrderRequest, OrderResponse, OrderStatus, AccountBalance};
use crate::types::errors::ConnectorError;
use super::subscription_manager::SubscriptionConfig as SubMgrConfig;

/// 现代化的连接器配置trait
/// 使用关联类型提供类型安全的配置
pub trait ConnectorConfig: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// 验证配置的有效性
    fn validate(&self) -> Result<(), Self::Error>;
    
    /// 获取连接超时设置
    fn connection_timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
    
    /// 获取重连间隔
    fn reconnect_interval(&self) -> Duration {
        Duration::from_secs(5)
    }
    
    /// 获取最大重连次数
    fn max_reconnect_attempts(&self) -> u32 {
        5
    }
}

/// 现代化的连接器状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModernConnectionStatus {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting {
        attempt: u32,
        started_at: DateTime<Utc>,
    },
    /// 已连接
    Connected {
        connected_at: DateTime<Utc>,
        last_heartbeat: DateTime<Utc>,
    },
    /// 重连中
    Reconnecting {
        attempt: u32,
        last_error: String,
        started_at: DateTime<Utc>,
    },
    /// 连接失败
    Failed {
        error: String,
        failed_at: DateTime<Utc>,
        retry_after: Option<DateTime<Utc>>,
    },
}

/// 连接器指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorMetrics {
    /// 连接建立时间
    pub connection_established_at: Option<DateTime<Utc>>,
    /// 总接收消息数
    pub messages_received: u64,
    /// 总发送消息数
    pub messages_sent: u64,
    /// 重连次数
    pub reconnect_count: u32,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// 最后一次心跳时间
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// 错误计数
    pub error_count: u64,
    /// 最后一次错误
    pub last_error: Option<String>,
    /// 订阅数量
    pub subscription_count: u32,
    /// 有界通道缓冲区使用率
    pub channel_buffer_usage: f64,
    /// 通道溢出次数
    pub channel_overflow_count: u64,
    /// 批量处理次数
    pub batch_processing_count: u64,
    /// 平均批量大小
    pub avg_batch_size: f64,
}

impl Default for ConnectorMetrics {
    fn default() -> Self {
        Self {
            connection_established_at: None,
            messages_received: 0,
            messages_sent: 0,
            reconnect_count: 0,
            avg_latency_ms: 0.0,
            last_heartbeat: None,
            error_count: 0,
            last_error: None,
            subscription_count: 0,
            channel_buffer_usage: 0.0,
            channel_overflow_count: 0,
            batch_processing_count: 0,
            avg_batch_size: 0.0,
        }
    }
}

// 订阅配置和优先级已在subscription_manager模块中定义

/// 现代化的连接器trait
/// 使用关联类型和泛型提供更好的类型安全
#[async_trait]
pub trait ModernExchangeConnector: Send + Sync + 'static {
    /// 连接器配置类型
    type Config: ConnectorConfig;
    /// 连接器特定的错误类型
    type Error: From<ConnectorError> + std::error::Error + Send + Sync + 'static;
    /// 市场数据类型
    type MarketData: Send + Sync + 'static;
    /// 用户数据类型
    type UserData: Send + Sync + 'static;
    
    // === 基础信息 ===
    
    /// 获取交易所类型
    fn exchange_type(&self) -> ExchangeType;
    
    /// 获取市场类型
    fn market_type(&self) -> MarketType;
    
    /// 获取连接器名称
    fn name(&self) -> &str;
    
    /// 获取连接器版本
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    /// 获取支持的功能列表
    fn supported_features(&self) -> Vec<ConnectorFeature>;
    
    // === 生命周期管理 ===
    
    /// 初始化连接器
    async fn initialize(&mut self, config: Self::Config) -> Result<(), Self::Error>;
    
    /// 启动连接器
    async fn start(&mut self) -> Result<(), Self::Error>;
    
    /// 停止连接器
    async fn stop(&mut self) -> Result<(), Self::Error>;
    
    /// 优雅关闭连接器
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.stop().await
    }
    
    // === 连接管理 ===
    
    /// 建立连接
    async fn connect(&mut self) -> Result<(), Self::Error>;
    
    /// 断开连接
    async fn disconnect(&mut self) -> Result<(), Self::Error>;
    
    /// 重新连接
    async fn reconnect(&mut self) -> Result<(), Self::Error> {
        self.disconnect().await?;
        self.connect().await
    }
    
    /// 获取连接状态
    async fn connection_status(&self) -> ModernConnectionStatus;
    
    /// 检查连接是否健康
    async fn is_healthy(&self) -> bool {
        matches!(self.connection_status().await, ModernConnectionStatus::Connected { .. })
    }
    
    // === 订阅管理 ===
    
    /// 订阅市场数据
    async fn subscribe(&mut self, config: SubMgrConfig) -> Result<(), Self::Error>;
    
    /// 取消订阅
    async fn unsubscribe(&mut self, config: SubMgrConfig) -> Result<(), Self::Error>;
    
    /// 批量订阅
    async fn subscribe_batch(&mut self, configs: Vec<SubMgrConfig>) -> Result<BatchSubscriptionResult, Self::Error> {
        let mut total_requested = 0;
        let mut successful = 0;
        let mut failed_symbols = Vec::new();
        
        for config in configs {
            total_requested += config.symbols.len();
            match self.subscribe(config.clone()).await {
                Ok(_) => successful += config.symbols.len(),
                Err(e) => {
                    for symbol in config.symbols {
                        failed_symbols.push((symbol, e.to_string()));
                    }
                }
            }
        }
        
        Ok(BatchSubscriptionResult {
            total_requested,
            successful,
            failed: failed_symbols.len(),
            pending: 0,
            failed_symbols,
            results: Vec::new(),
        })
    }
    
    /// 批量取消订阅
    async fn unsubscribe_batch(&mut self, configs: Vec<SubMgrConfig>) -> Result<BatchSubscriptionResult, Self::Error> {
        let mut total_requested = 0;
        let mut successful = 0;
        let mut failed_symbols = Vec::new();
        
        for config in configs {
            total_requested += config.symbols.len();
            match self.unsubscribe(config.clone()).await {
                Ok(_) => successful += config.symbols.len(),
                Err(e) => {
                    for symbol in config.symbols {
                        failed_symbols.push((symbol, e.to_string()));
                    }
                }
            }
        }
        
        Ok(BatchSubscriptionResult {
            total_requested,
            successful,
            failed: failed_symbols.len(),
            pending: 0,
            failed_symbols,
            results: Vec::new(),
        })
    }
    
    /// 获取当前订阅状态
    async fn subscription_status(&self) -> HashMap<String, SubscriptionStatus>;
    
    // === 数据流接口 ===
    
    /// 获取市场数据流（有界通道）
    fn market_data_stream(&self) -> Option<mpsc::Receiver<Self::MarketData>>;
    
    /// 获取用户数据流（有界通道）
    fn user_data_stream(&self) -> Option<mpsc::Receiver<Self::UserData>>;
    
    /// 获取系统事件流
    fn event_stream(&self) -> broadcast::Receiver<SystemEvent>;
    
    /// 获取批量市场数据流
    fn batch_market_data_stream(&self) -> Option<mpsc::Receiver<Vec<Self::MarketData>>> {
        None
    }
    
    /// 获取批量用户数据流
    fn batch_user_data_stream(&self) -> Option<mpsc::Receiver<Vec<Self::UserData>>> {
        None
    }
    
    /// 设置通道缓冲区大小
    async fn set_channel_buffer_size(&mut self, buffer_size: usize) -> Result<(), Self::Error> {
        // 默认实现：什么都不做
        Ok(())
    }
    
    /// 获取当前通道缓冲区大小
    fn channel_buffer_size(&self) -> usize {
        1000 // 默认缓冲区大小
    }
    
    // === 快照数据 ===
    
    /// 获取订单簿快照
    async fn orderbook_snapshot(&self, symbol: &str) -> Result<Option<StandardizedOrderBook>, Self::Error>;
    
    /// 获取最近交易快照
    async fn recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Result<Vec<StandardizedTrade>, Self::Error>;
    
    /// 批量获取订单簿快照
    async fn batch_orderbook_snapshot(&self, symbols: &[String]) -> Result<HashMap<String, StandardizedOrderBook>, Self::Error> {
        let mut results = HashMap::new();
        for symbol in symbols {
            if let Ok(Some(orderbook)) = self.orderbook_snapshot(symbol).await {
                results.insert(symbol.clone(), orderbook);
            }
        }
        Ok(results)
    }
    
    /// 批量获取最近交易快照
    async fn batch_recent_trades_snapshot(&self, symbols: &[String], limit: usize) -> Result<HashMap<String, Vec<StandardizedTrade>>, Self::Error> {
        let mut results = HashMap::new();
        for symbol in symbols {
            if let Ok(trades) = self.recent_trades_snapshot(symbol, limit).await {
                results.insert(symbol.clone(), trades);
            }
        }
        Ok(results)
    }
    
    // === 交易功能 ===
    
    /// 下单
    async fn place_order(&mut self, order: &OrderRequest) -> Result<OrderResponse, Self::Error> {
        Err(Self::Error::from(ConnectorError::TradingNotImplemented))
    }
    
    /// 取消订单
    async fn cancel_order(&mut self, order_id: &str, symbol: &str) -> Result<bool, Self::Error> {
        Err(Self::Error::from(ConnectorError::TradingNotImplemented))
    }
    
    /// 获取订单状态
    async fn order_status(&self, order_id: &str, symbol: &str) -> Result<OrderStatus, Self::Error> {
        Err(Self::Error::from(ConnectorError::TradingNotImplemented))
    }
    
    /// 获取账户余额
    async fn account_balance(&self) -> Result<AccountBalance, Self::Error> {
        Err(Self::Error::from(ConnectorError::TradingNotImplemented))
    }
    
    // === 监控和指标 ===
    
    /// 获取连接器指标
    async fn metrics(&self) -> ConnectorMetrics;
    
    /// 执行健康检查
    async fn health_check(&self) -> Result<HealthCheckResult, Self::Error>;
    
    /// 获取连接质量
    async fn connection_quality(&self) -> Result<ConnectionQuality, Self::Error>;
    
    // === 配置管理 ===
    
    /// 获取当前配置
    fn config(&self) -> &Self::Config;
    
    /// 更新配置
    async fn update_config(&mut self, config: Self::Config) -> Result<(), Self::Error>;
    
    /// 重新加载配置
    async fn reload_config(&mut self) -> Result<(), Self::Error> {
        // 默认实现：什么都不做
        Ok(())
    }
}

/// 连接器功能枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorFeature {
    /// 市场数据订阅
    MarketData,
    /// 用户数据流
    UserDataStream,
    /// 现货交易
    SpotTrading,
    /// 期货交易
    FuturesTrading,
    /// 期权交易
    OptionsTrading,
    /// 杠杆交易
    MarginTrading,
    /// 批量订阅
    BatchSubscription,
    /// 自动重连
    AutoReconnect,
    /// 心跳检测
    Heartbeat,
    /// 压缩数据
    Compression,
    /// 增量更新
    IncrementalUpdates,
}

/// 健康检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// 是否健康
    pub healthy: bool,
    /// 检查时间
    pub checked_at: DateTime<Utc>,
    /// 延迟（毫秒）
    pub latency_ms: Option<f64>,
    /// 详细信息
    pub details: HashMap<String, String>,
    /// 警告信息
    pub warnings: Vec<String>,
    /// 错误信息
    pub errors: Vec<String>,
}

/// 现代化的连接器管理器trait
#[async_trait]
pub trait ModernConnectorManager: Send + Sync {
    type Connector: ModernExchangeConnector;
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// 注册连接器
    async fn register_connector(
        &mut self, 
        id: String, 
        connector: Self::Connector
    ) -> Result<(), Self::Error>;
    
    /// 注销连接器
    async fn unregister_connector(&mut self, id: &str) -> Result<(), Self::Error>;
    
    /// 获取连接器
    fn get_connector(&self, id: &str) -> Option<&Self::Connector>;
    
    /// 获取可变连接器
    fn get_connector_mut(&mut self, id: &str) -> Option<&mut Self::Connector>;
    
    /// 列出所有连接器ID
    fn list_connectors(&self) -> Vec<String>;
    
    /// 启动所有连接器
    async fn start_all(&mut self) -> Result<(), Self::Error>;
    
    /// 停止所有连接器
    async fn stop_all(&mut self) -> Result<(), Self::Error>;
    
    /// 获取所有连接器状态
    async fn status_all(&self) -> HashMap<String, ModernConnectionStatus>;
    
    /// 执行所有连接器的健康检查
    async fn health_check_all(&self) -> HashMap<String, HealthCheckResult>;
}

/// 连接器构建器trait
pub trait ConnectorBuilder<C: ModernExchangeConnector> {
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// 设置配置
    fn with_config(self, config: C::Config) -> Self;
    
    /// 设置重连策略
    fn with_reconnect_strategy(self, strategy: ReconnectStrategy) -> Self;
    
    /// 设置指标收集器
    fn with_metrics_collector(self, collector: Arc<dyn MetricsCollector>) -> Self;
    
    /// 构建连接器
    fn build(self) -> Result<C, Self::Error>;
}

/// 重连策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReconnectStrategy {
    /// 固定间隔重连
    FixedInterval {
        interval: Duration,
        max_attempts: Option<u32>,
    },
    /// 指数退避重连
    ExponentialBackoff {
        initial_interval: Duration,
        max_interval: Duration,
        multiplier: f64,
        max_attempts: Option<u32>,
    },
    /// 自定义重连策略
    Custom {
        intervals: Vec<Duration>,
    },
}

/// 指标收集器trait
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// 记录连接事件
    async fn record_connection_event(&self, connector_id: &str, event: ConnectionEvent);
    
    /// 记录消息事件
    async fn record_message_event(&self, connector_id: &str, event: MessageEvent);
    
    /// 记录错误事件
    async fn record_error_event(&self, connector_id: &str, error: &dyn std::error::Error);
    
    /// 更新指标
    async fn update_metrics(&self, connector_id: &str, metrics: &ConnectorMetrics);
}

/// 连接事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionEvent {
    Connected,
    Disconnected,
    Reconnecting,
    Failed(String),
}

/// 消息事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageEvent {
    Sent { size: usize },
    Received { size: usize },
    Heartbeat,
    BatchProcessed { batch_size: usize, processing_time_ms: f64 },
}

/// 批量IO处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIOConfig {
    /// 批量大小
    pub batch_size: usize,
    /// 批量超时时间（毫秒）
    pub batch_timeout_ms: u64,
    /// 最大缓冲区大小
    pub max_buffer_size: usize,
    /// 是否启用压缩
    pub enable_compression: bool,
}

impl Default for BatchIOConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_timeout_ms: 1000,
            max_buffer_size: 10000,
            enable_compression: false,
        }
    }
}

/// 批量IO处理器
#[async_trait]
pub trait BatchIOProcessor<T>: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// 处理单个项目
    async fn process_item(&self, item: T) -> Result<(), Self::Error>;
    
    /// 批量处理项目
    async fn process_batch(&self, items: Vec<T>) -> Result<(), Self::Error>;
    
    /// 刷新缓冲区
    async fn flush(&self) -> Result<(), Self::Error>;
    
    /// 获取缓冲区状态
    fn buffer_status(&self) -> BatchBufferStatus;
}

/// 批量缓冲区状态
#[derive(Debug, Clone)]
pub struct BatchBufferStatus {
    pub current_size: usize,
    pub max_size: usize,
    pub usage_percentage: f64,
    pub pending_items: usize,
    pub last_flush: Option<DateTime<Utc>>,
}

/// 有界通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundedChannelConfig {
    /// 市场数据通道缓冲区大小
    pub market_data_buffer_size: usize,
    /// 用户数据通道缓冲区大小
    pub user_data_buffer_size: usize,
    /// 事件通道缓冲区大小
    pub event_buffer_size: usize,
    /// 批量数据通道缓冲区大小
    pub batch_buffer_size: usize,
    /// 通道满时的处理策略
    pub overflow_strategy: ChannelOverflowStrategy,
}

impl Default for BoundedChannelConfig {
    fn default() -> Self {
        Self {
            market_data_buffer_size: 1000,
            user_data_buffer_size: 500,
            event_buffer_size: 100,
            batch_buffer_size: 200,
            overflow_strategy: ChannelOverflowStrategy::DropOldest,
        }
    }
}

/// 通道溢出策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelOverflowStrategy {
    /// 丢弃最旧的消息
    DropOldest,
    /// 丢弃最新的消息
    DropNewest,
    /// 阻塞直到有空间
    Block,
    /// 返回错误
    Error,
}

/// 现代化连接器的默认实现辅助宏
#[macro_export]
macro_rules! impl_modern_connector_basics {
    ($connector:ty, $exchange:expr, $market:expr, $name:expr) => {
        impl ModernExchangeConnector for $connector {
            fn exchange_type(&self) -> ExchangeType {
                $exchange
            }
            
            fn market_type(&self) -> MarketType {
                $market
            }
            
            fn name(&self) -> &str {
                $name
            }
        }
    };
}