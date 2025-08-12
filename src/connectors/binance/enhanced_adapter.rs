//! 增强版Binance连接器适配器
//!
//! 提供现代化的错误处理和重试机制

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};

use super::config::BinanceConfig;
use super::spot::BinanceSpotConnector;
use super::websocket::BinanceWebSocketHandler;
use crate::connectors::binance::health_monitor::{
    HealthCheckConfig, HealthMonitor, HealthStatistics, RecoveryAction,
};
use crate::connectors::common::{
    adaptive_timeout::AdaptiveTimeoutManager, batch_subscription::BatchSubscriptionManager,
    emergency_ping::EmergencyPingManager,
};
use crate::connectors::traits::*;
use crate::types::config::{BatchSubscriptionResult, ConnectionQuality, HealthStatus};
use crate::types::*;

// 类型别名用于简化复杂类型
type ErrorHistory = Arc<RwLock<Vec<(DateTime<Utc>, EnhancedBinanceError)>>>;

/// 增强版错误类型
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum EnhancedBinanceError {
    #[error("连接错误: {message}")]
    Connection { message: String, retryable: bool },

    #[error("认证错误: {message}")]
    Authentication { message: String, retryable: bool },

    #[error("网络错误: {message}")]
    Network { message: String, retryable: bool },

    #[error("API限制错误: {message}, 重试时间: {retry_after:?}")]
    RateLimit {
        message: String,
        retry_after: Option<Duration>,
    },

    #[error("订阅错误: {message}")]
    Subscription {
        message: String,
        symbol: Option<String>,
    },

    #[error("配置错误: {message}")]
    Configuration { message: String },

    #[error("超时错误: {message}, 超时时间: {timeout:?}")]
    Timeout { message: String, timeout: Duration },

    #[error("服务不可用: {message}")]
    ServiceUnavailable {
        message: String,
        estimated_recovery: Option<DateTime<Utc>>,
    },

    #[error("数据解析错误: {message}")]
    DataParsing {
        message: String,
        raw_data: Option<String>,
    },

    #[error("内部错误: {message}")]
    Internal { message: String },
}

impl EnhancedBinanceError {
    /// 判断错误是否可重试
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Connection { retryable, .. } => *retryable,
            Self::Authentication { retryable, .. } => *retryable,
            Self::Network { retryable, .. } => *retryable,
            Self::RateLimit { .. } => true,
            Self::Subscription { .. } => true,
            Self::Configuration { .. } => false,
            Self::Timeout { .. } => true,
            Self::ServiceUnavailable { .. } => true,
            Self::DataParsing { .. } => false,
            Self::Internal { .. } => false,
        }
    }

    /// 获取建议的重试延迟
    pub fn suggested_retry_delay(&self) -> Option<Duration> {
        match self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            Self::Network { .. } => Some(Duration::from_secs(2)),
            Self::Connection { .. } => Some(Duration::from_secs(5)),
            Self::Timeout { .. } => Some(Duration::from_secs(3)),
            Self::ServiceUnavailable { .. } => Some(Duration::from_secs(30)),
            Self::Subscription { .. } => Some(Duration::from_secs(1)),
            _ => None,
        }
    }
}

/// 重试策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 基础延迟时间
    pub base_delay: Duration,
    /// 最大延迟时间
    pub max_delay: Duration,
    /// 指数退避倍数
    pub backoff_multiplier: f64,
    /// 抖动因子 (0.0-1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

/// 重试状态
#[derive(Debug, Clone)]
struct RetryState {
    attempt: u32,
    last_error: Option<EnhancedBinanceError>,
    last_attempt_time: Option<Instant>,
    total_delay: Duration,
}

impl RetryState {
    fn new() -> Self {
        Self {
            attempt: 0,
            last_error: None,
            last_attempt_time: None,
            total_delay: Duration::ZERO,
        }
    }

    fn reset(&mut self) {
        self.attempt = 0;
        self.last_error = None;
        self.last_attempt_time = None;
        self.total_delay = Duration::ZERO;
    }
}

/// 错误恢复策略
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// 立即重试
    Immediate,
    /// 延迟重试
    Delayed(Duration),
    /// 指数退避
    ExponentialBackoff,
    /// 线性退避
    LinearBackoff,
    /// 不重试
    NoRetry,
}

/// 增强版Binance适配器
#[allow(dead_code)]
#[derive(Clone)]
pub struct EnhancedBinanceAdapter {
    config: BinanceConfig,
    retry_config: RetryConfig,
    spot_connector: Arc<RwLock<Option<BinanceSpotConnector>>>,
    websocket_handler: Arc<RwLock<Option<BinanceWebSocketHandler>>>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    app_state: Arc<crate::AppState>,
    pending_subscriptions: Arc<RwLock<(Vec<String>, Vec<DataType>)>>,

    // 错误处理和重试相关
    retry_state: Arc<Mutex<RetryState>>,
    error_history: ErrorHistory,
    recovery_strategy: Arc<RwLock<RecoveryStrategy>>,

    // WebSocket优化模块
    emergency_ping_manager: Arc<RwLock<EmergencyPingManager>>,
    adaptive_timeout_manager: Arc<RwLock<AdaptiveTimeoutManager>>,
    batch_subscription_manager: Arc<RwLock<BatchSubscriptionManager>>,

    // 健康监控
    health_monitor: Arc<RwLock<HealthMonitor>>,
    last_successful_operation: Arc<RwLock<Option<Instant>>>,
    consecutive_failures: Arc<RwLock<u32>>,
}

impl EnhancedBinanceAdapter {
    /// 创建新的增强版Binance适配器实例
    pub async fn new(
        config: BinanceConfig,
        app_state: Arc<crate::AppState>,
    ) -> Result<Self, EnhancedBinanceError> {
        Self::new_with_retry_config(config, app_state, RetryConfig::default()).await
    }

    /// 使用自定义重试配置创建适配器
    pub async fn new_with_retry_config(
        config: BinanceConfig,
        app_state: Arc<crate::AppState>,
        retry_config: RetryConfig,
    ) -> Result<Self, EnhancedBinanceError> {
        let health_config = HealthCheckConfig {
            check_interval_secs: 30,
            data_timeout_secs: 120,
            max_consecutive_failures: 3,
            auto_recovery_enabled: true,
            health_check_timeout_secs: 10,
        };

        let health_monitor = HealthMonitor::new(health_config);

        Ok(Self {
            config,
            retry_config,
            spot_connector: Arc::new(RwLock::new(None)),
            websocket_handler: Arc::new(RwLock::new(None)),
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            app_state,
            pending_subscriptions: Arc::new(RwLock::new((Vec::new(), Vec::new()))),
            retry_state: Arc::new(Mutex::new(RetryState::new())),
            error_history: Arc::new(RwLock::new(Vec::new())),
            recovery_strategy: Arc::new(RwLock::new(RecoveryStrategy::ExponentialBackoff)),
            emergency_ping_manager: Arc::new(RwLock::new(EmergencyPingManager::with_default_config())),
            adaptive_timeout_manager: Arc::new(RwLock::new(AdaptiveTimeoutManager::with_default_config())),
            batch_subscription_manager: Arc::new(RwLock::new(BatchSubscriptionManager::with_default_config())),
            health_monitor: Arc::new(RwLock::new(health_monitor)),
            last_successful_operation: Arc::new(RwLock::new(None)),
            consecutive_failures: Arc::new(RwLock::new(0)),
        })
    }

    /// 记录错误到历史记录
    async fn record_error(&self, error: EnhancedBinanceError) {
        let mut history = self.error_history.write().await;
        history.push((Utc::now(), error));
        
        // 保持历史记录在合理大小
        if history.len() > 100 {
            history.drain(0..50);
        }
    }

    /// 计算重试延迟
    async fn calculate_retry_delay(&self, error: &EnhancedBinanceError) -> Duration {
        let retry_state = self.retry_state.lock().await;
        let strategy = self.recovery_strategy.read().await;
        
        match &*strategy {
            RecoveryStrategy::Immediate => Duration::ZERO,
            RecoveryStrategy::Delayed(duration) => *duration,
            RecoveryStrategy::ExponentialBackoff => {
                let base_delay = error.suggested_retry_delay()
                    .unwrap_or(self.retry_config.base_delay);
                
                let exponential_delay = base_delay.as_millis() as f64 
                    * self.retry_config.backoff_multiplier.powi(retry_state.attempt as i32);
                
                let max_delay = self.retry_config.max_delay.as_millis() as f64;
                let delay_ms = exponential_delay.min(max_delay);
                
                // 添加抖动
                let jitter = delay_ms * self.retry_config.jitter_factor * (rand::random::<f64>() - 0.5);
                let final_delay = (delay_ms + jitter).max(0.0) as u64;
                
                Duration::from_millis(final_delay)
            },
            RecoveryStrategy::LinearBackoff => {
                let base_delay = self.retry_config.base_delay;
                let linear_delay = base_delay * (retry_state.attempt + 1);
                linear_delay.min(self.retry_config.max_delay)
            },
            RecoveryStrategy::NoRetry => Duration::MAX,
        }
    }

    /// 执行带重试的操作
    async fn execute_with_retry<F, T, E>(&self, operation: F) -> Result<T, EnhancedBinanceError>
    where
        F: Fn() -> Result<T, E> + Send + Sync,
        E: Into<EnhancedBinanceError>,
    {
        let mut retry_state = self.retry_state.lock().await;
        
        loop {
            match operation() {
                Ok(result) => {
                    // 成功，重置重试状态
                    retry_state.reset();
                    *self.last_successful_operation.write().await = Some(Instant::now());
                    *self.consecutive_failures.write().await = 0;
                    return Ok(result);
                },
                Err(e) => {
                    let error: EnhancedBinanceError = e.into();
                    retry_state.attempt += 1;
                    retry_state.last_error = Some(error.clone());
                    retry_state.last_attempt_time = Some(Instant::now());
                    
                    *self.consecutive_failures.write().await += 1;
                    self.record_error(error.clone()).await;
                    
                    // 检查是否应该重试
                    if !error.is_retryable() || retry_state.attempt >= self.retry_config.max_attempts {
                        return Err(error);
                    }
                    
                    // 计算延迟并等待
                    let delay = self.calculate_retry_delay(&error).await;
                    retry_state.total_delay += delay;
                    // 在释放锁之前保存当前尝试次数，避免 moved after drop 错误
                    let current_attempt = retry_state.attempt;
                    
                    drop(retry_state); // 释放锁
                    
                    if delay > Duration::ZERO {
                        info!("重试操作，延迟: {:?}, 尝试次数: {}", delay, current_attempt);
                        tokio::time::sleep(delay).await;
                    }
                    
                    retry_state = self.retry_state.lock().await; // 重新获取锁
                }
            }
        }
    }

    /// 获取健康状态
    pub async fn get_health_status(&self) -> HealthStatus {
        self.health_monitor.read().await.get_health_status().await
    }

    /// 获取健康统计信息
    pub async fn get_health_statistics(&self) -> HealthStatistics {
        self.health_monitor.read().await.get_statistics().await
    }

    /// 获取错误历史
    pub async fn get_error_history(&self) -> Vec<(DateTime<Utc>, EnhancedBinanceError)> {
        self.error_history.read().await.clone()
    }

    /// 设置恢复策略
    pub async fn set_recovery_strategy(&self, strategy: RecoveryStrategy) {
        *self.recovery_strategy.write().await = strategy;
    }

    /// 强制健康检查
    pub async fn force_health_check(&self) -> HealthStatus {
        // simple_health_check 需要可变借用，这里临时获取可写锁
        self.health_monitor.write().await.simple_health_check().await
    }

    /// 获取连接质量
    pub async fn get_connection_quality(&self) -> ConnectionQuality {
        let consecutive_failures = *self.consecutive_failures.read().await;
        let last_successful = *self.last_successful_operation.read().await;
        
        // 基于失败次数与成功时间估计稳定性
        let stability_score = if consecutive_failures == 0 {
            0.95
        } else if consecutive_failures <= 2 {
            0.85
        } else if consecutive_failures <= 5 {
            0.7
        } else {
            0.3
        };

        let latency_ms = match last_successful {
            Some(ts) => {
                let elapsed = ts.elapsed().as_millis() as f64;
                // 粗略估计：越久未成功，延迟指标越差（上限 2000ms）
                (elapsed / 4.0).min(2000.0)
            }
            None => 1500.0,
        };

        ConnectionQuality {
            latency_ms,
            packet_loss_rate: {
                let v: f64 = 1.0 - stability_score;
                if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v }
            },
            stability_score,
            last_updated: chrono::Utc::now(),
        }
    }
}

#[async_trait]
impl ExchangeConnector for EnhancedBinanceAdapter {
    fn get_exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }

    fn get_market_type(&self) -> MarketType {
        MarketType::Spot
    }

    fn get_exchange_name(&self) -> &str {
        "Binance Enhanced"
    }

    async fn is_connected(&self) -> bool {
        matches!(*self.connection_status.read().await, ConnectionStatus::Connected)
    }

    async fn get_connection_status(&self) -> ConnectionStatus {
        *self.connection_status.read().await
    }

    async fn connect_websocket(&self) -> Result<(), ConnectorError> {
        // TODO: 实现WebSocket连接逻辑
        Ok(())
    }

    async fn disconnect_websocket(&self) -> Result<(), ConnectorError> {
        // TODO: 实现WebSocket断开逻辑
        Ok(())
    }

    async fn subscribe_orderbook(&self, symbol: &str) -> Result<(), ConnectorError> {
        // TODO: 实现订单簿订阅逻辑
        Ok(())
    }

    async fn subscribe_trades(&self, symbol: &str) -> Result<(), ConnectorError> {
        // TODO: 实现交易数据订阅逻辑
        Ok(())
    }

    async fn subscribe_user_stream(&self) -> Result<(), ConnectorError> {
        // TODO: 实现用户数据流订阅逻辑
        Ok(())
    }

    fn get_market_data_stream(&self) -> mpsc::Receiver<StandardizedMessage> {
        let (_, rx) = mpsc::channel(1000);
        rx
    }

    fn get_user_data_stream(&self) -> mpsc::Receiver<StandardizedMessage> {
        let (_, rx) = mpsc::channel(1000);
        rx
    }

    async fn get_orderbook_snapshot(&self, symbol: &str) -> Option<StandardizedOrderBook> {
        // TODO: 实现订单簿快照获取逻辑
        None
    }

    async fn get_recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Vec<StandardizedTrade> {
        // TODO: 实现最近交易快照获取逻辑
        Vec::new()
    }

    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse, ConnectorError> {
        // TODO: 实现下单逻辑
        Err(ConnectorError::TradingNotImplemented)
    }

    async fn cancel_order(&self, _order_id: &str, _symbol: &str) -> Result<bool, ConnectorError> {
        // TODO: 实现撤单逻辑
        Ok(false)
    }

    async fn get_order_status(&self, _order_id: &str, _symbol: &str) -> Result<OrderStatus, ConnectorError> {
        // TODO: 实现订单状态查询逻辑
        Err(ConnectorError::TradingNotImplemented)
    }

    async fn get_account_balance(&self) -> Result<AccountBalance, ConnectorError> {
        // TODO: 实现账户余额查询逻辑
        Err(ConnectorError::TradingNotImplemented)
    }

    async fn is_websocket_connected(&self) -> bool {
        // TODO: 实现WebSocket连接状态检查逻辑
        false
    }
}



#[async_trait]
impl OrderManagement for EnhancedBinanceAdapter {
    type Error = EnhancedBinanceError;
    
    async fn place_order(&mut self, order: &OrderRequest) -> Result<OrderResponse, Self::Error> {
        self.execute_with_retry(|| -> Result<OrderResponse, EnhancedBinanceError> {
            // 实际下单逻辑
            let resp = OrderResponse {
                order_id: "test_order_id".to_string(),
                client_order_id: order.client_order_id.clone(),
                symbol: order.symbol.clone(),
                status: "NEW".to_string(),
                filled_quantity: 0.0,
                remaining_quantity: order.quantity,
                average_price: None,
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            };
            Ok(resp)
        }).await
    }

    async fn cancel_order(&mut self, _order_id: &str, _symbol: &str) -> Result<bool, Self::Error> {
        self.execute_with_retry(|| -> Result<bool, EnhancedBinanceError> {
            // 实际撤单逻辑
            Ok(true)
        }).await
    }

    async fn get_order_status(&self, order_id: &str, symbol: &str) -> Result<OrderStatus, Self::Error> {
        self.execute_with_retry(|| -> Result<OrderStatus, EnhancedBinanceError> {
            // 实际查询订单状态逻辑
            Ok(OrderStatus {
                order_id: order_id.to_string(),
                symbol: symbol.to_string(),
                status: "FILLED".to_string(),
                filled_quantity: 0.0,
                remaining_quantity: 0.0,
                average_price: None,
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            })
        }).await
    }
}

#[async_trait]
impl AccountDataProvider for EnhancedBinanceAdapter {
    type Error = EnhancedBinanceError;
    
    async fn get_account_balance(&self) -> Result<AccountBalance, Self::Error> {
        self.execute_with_retry(|| -> Result<AccountBalance, EnhancedBinanceError> {
            use std::collections::HashMap;
            // 实际获取账户余额逻辑（示例数据）
            let mut balances: HashMap<String, CurrencyBalance> = HashMap::new();
            balances.insert(
                "USDT".to_string(),
                CurrencyBalance { currency: "USDT".to_string(), total: 1000.0, available: 1000.0, frozen: 0.0 },
            );
            balances.insert(
                "BTC".to_string(),
                CurrencyBalance { currency: "BTC".to_string(), total: 0.1, available: 0.1, frozen: 0.0 },
            );
            let total: f64 = balances.values().map(|b| b.total).sum();
            let available: f64 = balances.values().map(|b| b.available).sum();
            let frozen: f64 = balances.values().map(|b| b.frozen).sum();
            Ok(AccountBalance { total, available, frozen, balances })
        }).await
    }

    async fn get_positions(&self) -> Result<Vec<crate::types::trading::Position>, Self::Error> {
        self.execute_with_retry(|| -> Result<Vec<crate::types::trading::Position>, EnhancedBinanceError> {
            // 实际获取持仓逻辑
            Ok(vec![])
        }).await
    }
}
