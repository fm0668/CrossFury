//! 现代化的Binance连接器实现
//!
//! 基于ModernExchangeConnector trait重新设计的Binance连接器
//! 提供更好的错误处理、重试机制和健康检查功能

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc, RwLock};

use super::super::traits::subscription_manager::SubscriptionConfig as SubMgrConfig;
use super::{
    config::BinanceConfig, spot::BinanceSpotConnector, websocket::BinanceWebSocketHandler,
};
use crate::connectors::traits::modern::*;
use crate::types::{
    common::{ExchangeType, MarketType},
    config::{ConnectionQuality, SubscriptionStatus},
    errors::ConnectorError,
    events::SystemEvent,
    market_data::{StandardizedOrderBook, StandardizedTrade},
};
use crate::types::market_data::MarketDataEvent;
use crate::types::events::UserDataEvent;

/// 现代化的Binance连接器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernBinanceConfig {
    /// 基础Binance配置
    pub base_config: BinanceConfig,
    /// 连接超时设置
    pub connection_timeout: Duration,
    /// 重连间隔
    pub reconnect_interval: Duration,
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 重连策略
    pub reconnect_strategy: ReconnectStrategy,
    /// 有界通道配置
    pub channel_config: BoundedChannelConfig,
    /// 批量IO配置
    pub batch_config: BatchIOConfig,
    /// 健康检查间隔
    pub health_check_interval: Duration,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
}

impl Default for ModernBinanceConfig {
    fn default() -> Self {
        Self {
            base_config: BinanceConfig::default(),
            connection_timeout: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(5),
            max_reconnect_attempts: 5,
            reconnect_strategy: ReconnectStrategy::ExponentialBackoff {
                initial_interval: Duration::from_secs(1),
                max_interval: Duration::from_secs(60),
                multiplier: 2.0,
                max_attempts: Some(10),
            },
            channel_config: BoundedChannelConfig::default(),
            batch_config: BatchIOConfig::default(),
            health_check_interval: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(20),
        }
    }
}

/// 现代化Binance连接器错误类型
#[derive(Debug, thiserror::Error)]
pub enum ModernBinanceError {
    #[error("连接器错误: {0}")]
    Connector(#[from] ConnectorError),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("超时错误: {0}")]
    Timeout(String),

    #[error("重连失败: 已达到最大重连次数 {max_attempts}")]
    MaxReconnectAttemptsReached { max_attempts: u32 },

    #[error("健康检查失败: {reason}")]
    HealthCheckFailed { reason: String },
}

impl ConnectorConfig for ModernBinanceConfig {
    type Error = ModernBinanceError;

    fn validate(&self) -> Result<(), Self::Error> {
        // 验证基础配置
        if self
            .base_config
            .api_key
            .as_ref()
            .is_none_or(|key| key.is_empty())
        {
            return Err(ModernBinanceError::Config("API密钥不能为空".to_string()));
        }

        // 验证超时设置
        if self.connection_timeout.as_secs() == 0 {
            return Err(ModernBinanceError::Config("连接超时不能为0".to_string()));
        }

        // 验证重连设置
        if self.max_reconnect_attempts == 0 {
            return Err(ModernBinanceError::Config(
                "最大重连次数不能为0".to_string(),
            ));
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

/// 现代化的Binance连接器
pub struct ModernBinanceConnector {
    /// 连接器配置
    config: ModernBinanceConfig,
    /// 连接状态
    status: Arc<RwLock<ModernConnectionStatus>>,
    /// 连接器指标
    metrics_data: Arc<RwLock<ConnectorMetrics>>,
    /// WebSocket处理器
    websocket_handler: Arc<RwLock<Option<BinanceWebSocketHandler>>>,
    /// 现货连接器
    spot_connector: Arc<RwLock<Option<BinanceSpotConnector>>>,
    /// 应用状态
    app_state: Arc<crate::AppState>,
    /// 市场数据发送器
    #[allow(dead_code)]
    market_data_tx: Arc<RwLock<Option<mpsc::Sender<MarketDataEvent>>>>,
    /// 用户数据发送器
    #[allow(dead_code)]
    user_data_tx: Arc<RwLock<Option<mpsc::Sender<UserDataEvent>>>>,
    /// 系统事件发送器
    event_tx: broadcast::Sender<SystemEvent>,
    /// 重连计数器
    reconnect_count: Arc<RwLock<u32>>,
    /// 最后一次健康检查时间
    last_health_check: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl ModernBinanceConnector {
    /// 创建新的现代化Binance连接器
    pub fn new(config: ModernBinanceConfig, app_state: Arc<crate::AppState>) -> Self {
        let (event_tx, _) = broadcast::channel(config.channel_config.event_buffer_size);

        Self {
            config,
            status: Arc::new(RwLock::new(ModernConnectionStatus::Disconnected)),
            metrics_data: Arc::new(RwLock::new(ConnectorMetrics::default())),
            websocket_handler: Arc::new(RwLock::new(None)),
            spot_connector: Arc::new(RwLock::new(None)),
            app_state,
            market_data_tx: Arc::new(RwLock::new(None)),
            user_data_tx: Arc::new(RwLock::new(None)),
            event_tx,
            reconnect_count: Arc::new(RwLock::new(0)),
            last_health_check: Arc::new(RwLock::new(None)),
        }
    }

    /// 内部重连逻辑
    async fn internal_reconnect(&mut self) -> Result<(), ModernBinanceError> {
        let (current_attempt, delay) = {
            let mut reconnect_count = self.reconnect_count.write().await;
            *reconnect_count += 1;

            if *reconnect_count > self.config.max_reconnect_attempts {
                let error = ModernBinanceError::MaxReconnectAttemptsReached {
                    max_attempts: self.config.max_reconnect_attempts,
                };

                // 更新状态为失败
                {
                    let mut status = self.status.write().await;
                    *status = ModernConnectionStatus::Failed {
                        error: error.to_string(),
                        failed_at: Utc::now(),
                        retry_after: None,
                    };
                }

                return Err(error);
            }

            let current_attempt = *reconnect_count;
            let delay = self.calculate_reconnect_delay(current_attempt);
            (current_attempt, delay)
        };

        // 更新状态为重连中
        {
            let mut status = self.status.write().await;
            *status = ModernConnectionStatus::Reconnecting {
                attempt: current_attempt,
                last_error: "连接丢失".to_string(),
                started_at: Utc::now(),
            };
        }

        // 等待重连延迟
        if delay > Duration::ZERO {
            info!(
                "[ModernBinance] 等待 {:?} 后进行第 {} 次重连",
                delay, current_attempt
            );
            tokio::time::sleep(delay).await;
        }

        // 尝试重连
        match self.connect().await {
            Ok(_) => {
                info!("[ModernBinance] 重连成功");
                // 重置重连计数器
                *self.reconnect_count.write().await = 0;
                Ok(())
            }
            Err(e) => {
                warn!("[ModernBinance] 重连失败: {:?}", e);
                Err(e)
            }
        }
    }

    /// 计算重连延迟
    fn calculate_reconnect_delay(&self, attempt: u32) -> Duration {
        match &self.config.reconnect_strategy {
            ReconnectStrategy::FixedInterval { interval, .. } => *interval,
            ReconnectStrategy::Custom { intervals } => {
                if intervals.is_empty() { return self.config.reconnect_interval(); }
                let idx = (attempt as usize).saturating_sub(1).min(intervals.len() - 1);
                intervals[idx]
            }
            ReconnectStrategy::ExponentialBackoff {
                initial_interval,
                max_interval,
                multiplier,
                max_attempts: _,
            } => {
                let delay_ms = initial_interval.as_millis() as f64
                    * multiplier.powi(attempt.saturating_sub(1) as i32);
                let delay = Duration::from_millis(delay_ms as u64);
                delay.min(*max_interval)
            }
        }
    }

    /// 执行健康检查
    async fn perform_health_check(&self) -> Result<HealthCheckResult, ModernBinanceError> {
        let start_time = std::time::Instant::now();
        
        // 检查WebSocket连接状态
        let websocket_healthy = {
            let handler = self.websocket_handler.read().await;
            handler.is_some()
        };

        // 检查现货连接器状态
        let spot_healthy = {
            let connector = self.spot_connector.read().await;
            connector.is_some()
        };

        let latency = start_time.elapsed();
        let overall_healthy = websocket_healthy && spot_healthy;

        // 更新最后健康检查时间
        *self.last_health_check.write().await = Some(Utc::now());

        Ok(HealthCheckResult {
            healthy: overall_healthy,
            latency_ms: Some(latency.as_millis() as f64),
            errors: Vec::new(),
            warnings: Vec::new(),
            details: HashMap::from([
                ("websocket".to_string(), websocket_healthy.to_string()),
                ("spot_connector".to_string(), spot_healthy.to_string()),
            ]),
            checked_at: Utc::now(),
        })
    }

    /// 更新连接器指标
    async fn update_metrics(&self, operation: &str, success: bool, duration: Duration) {
        let mut metrics = self.metrics_data.write().await;
        
        if success {
            metrics.messages_received += 1;
        } else {
            metrics.error_count += 1;
        }
        
        // 更新平均延迟
        metrics.avg_latency_ms = 
            (metrics.avg_latency_ms + duration.as_millis() as f64) / 2.0;
        metrics.last_heartbeat = Some(Utc::now());
        
        debug!(
            "[ModernBinance] 更新指标: {} - 成功: {}, 耗时: {:?}",
            operation, success, duration
        );
    }
}

#[async_trait]
impl ModernExchangeConnector for ModernBinanceConnector {
    type Config = ModernBinanceConfig;
    type Error = ModernBinanceError;
    type MarketData = MarketDataEvent;
    type UserData = UserDataEvent;

    fn market_type(&self) -> MarketType {
        MarketType::Spot
    }
    
    fn name(&self) -> &str {
        "Binance"
    }
    
    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }
    
    fn supported_features(&self) -> Vec<ConnectorFeature> {
        vec![
            ConnectorFeature::MarketData,
            ConnectorFeature::UserData,
            ConnectorFeature::Trading,
            ConnectorFeature::HealthCheck,
        ]
    }
    
    async fn initialize(&mut self, config: Self::Config) -> Result<(), Self::Error> {
        self.config = config;
        Ok(())
    }
    
    async fn start(&mut self) -> Result<(), Self::Error> {
        self.connect().await
    }
    
    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.disconnect().await
    }
    
    async fn connection_status(&self) -> ModernConnectionStatus {
        self.status.read().await.clone()
    }
    
    async fn subscribe(&mut self, _config: SubMgrConfig) -> Result<(), Self::Error> {
        // TODO: 实现订阅逻辑
        Ok(())
    }
    
    async fn unsubscribe(&mut self, _config: SubMgrConfig) -> Result<(), Self::Error> {
        // TODO: 实现取消订阅逻辑
        Ok(())
    }
    
    async fn subscription_status(&self) -> HashMap<String, SubscriptionStatus> {
        // TODO: 实现订阅状态查询
        HashMap::new()
    }

    async fn connect(&mut self) -> Result<(), Self::Error> {
        let start_time = std::time::Instant::now();
        
        info!("[ModernBinance] 开始连接...");
        
        // 更新状态为连接中
        {
            let mut status = self.status.write().await;
            *status = ModernConnectionStatus::Connecting {
                attempt: 0,
                started_at: Utc::now(),
            };
        }

        // 创建WebSocket处理器
        let ws_handler = BinanceWebSocketHandler::new(
            self.config.base_config.clone(),
            Arc::clone(&self.app_state),
        )
        .await
        .map_err(|e| ModernBinanceError::Network(format!("创建WebSocket处理器失败: {}", e)))?;

        // 连接WebSocket
        ws_handler
            .connect()
            .await
            .map_err(|e| ModernBinanceError::Network(format!("WebSocket连接失败: {}", e)))?;

        // 创建现货连接器
        let spot_connector = BinanceSpotConnector::new(
            self.config.base_config.clone(),
            Arc::clone(&self.app_state),
        )
        .await
        .map_err(|e| ModernBinanceError::Network(format!("创建现货连接器失败: {}", e)))?;

        // 保存连接器实例
        {
            let mut ws_guard = self.websocket_handler.write().await;
            *ws_guard = Some(ws_handler);
        }
        {
            let mut spot_guard = self.spot_connector.write().await;
            *spot_guard = Some(spot_connector);
        }

        // 更新状态为已连接
        {
            let mut status = self.status.write().await;
            *status = ModernConnectionStatus::Connected {
                connected_at: Utc::now(),
                last_heartbeat: Utc::now(),
                attempt: 0,
            };
        }

        let duration = start_time.elapsed();
        self.update_metrics("connect", true, duration).await;
        
        info!("[ModernBinance] 连接成功，耗时: {:?}", duration);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        let start_time = std::time::Instant::now();
        
        info!("[ModernBinance] 开始断开连接...");

        // 断开WebSocket连接
        if let Some(handler) = self.websocket_handler.write().await.take() {
            handler
                .disconnect()
                .await
                .map_err(|e| ModernBinanceError::Network(format!("WebSocket断开失败: {}", e)))?;
        }

        // 清理现货连接器
        {
            let mut spot_guard = self.spot_connector.write().await;
            *spot_guard = None;
        }

        // 更新状态为已断开
        {
            let mut status = self.status.write().await;
            *status = ModernConnectionStatus::Disconnected;
        }

        // 重置重连计数器
        *self.reconnect_count.write().await = 0;

        let duration = start_time.elapsed();
        self.update_metrics("disconnect", true, duration).await;
        
        info!("[ModernBinance] 断开连接成功，耗时: {:?}", duration);
        Ok(())
    }

    async fn reconnect(&mut self) -> Result<(), Self::Error> {
        info!("[ModernBinance] 开始重连...");
        
        // 先断开现有连接
        if let Err(e) = self.disconnect().await {
            warn!("[ModernBinance] 断开连接时出错: {:?}", e);
        }

        // 执行内部重连逻辑
        self.internal_reconnect().await
    }

    async fn health_check(&self) -> Result<HealthCheckResult, Self::Error> {
        self.perform_health_check().await
    }

    // 添加缺少的trait方法
     fn market_data_stream(&self) -> Option<mpsc::Receiver<Self::MarketData>> {
         None
     }
     
     fn user_data_stream(&self) -> Option<mpsc::Receiver<Self::UserData>> {
         None
     }
     
     fn event_stream(&self) -> broadcast::Receiver<SystemEvent> {
         self.event_tx.subscribe()
     }
     
     async fn orderbook_snapshot(&self, symbol: &str) -> Result<Option<StandardizedOrderBook>, Self::Error> {
         warn!("[ModernBinance] 订单簿快照功能待实现: {}", symbol);
         Ok(None)
     }
     
     async fn recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Result<Vec<StandardizedTrade>, Self::Error> {
         warn!("[ModernBinance] 最近交易快照功能待实现: {} (limit: {})", symbol, limit);
         Ok(Vec::new())
     }
     
     async fn metrics(&self) -> ConnectorMetrics {
         self.metrics_data.read().await.clone()
     }
     
     async fn connection_quality(&self) -> Result<ConnectionQuality, Self::Error> {
         // 修正为克隆而非移动
         let metrics = self.metrics_data.read().await.clone();
         let status = self.status.read().await.clone();
         
         let (latency_ms, stability_score) = match status {
             ModernConnectionStatus::Connected { .. } => {
                 let total = metrics.messages_received + metrics.error_count;
                 let success_rate = if total > 0 {
                     metrics.messages_received as f64 / total as f64
                 } else {
                     1.0
                 };
                 (metrics.avg_latency_ms, success_rate.clamp(0.0, 1.0))
             }
             ModernConnectionStatus::Connecting { .. } | ModernConnectionStatus::Reconnecting { .. } => {
                 // 连接/重连中，稳定性适中
                 (metrics.avg_latency_ms, 0.6)
             }
             ModernConnectionStatus::Disconnected | ModernConnectionStatus::Failed { .. } => {
                 // 断开或失败，稳定性较差
                 (metrics.avg_latency_ms, 0.2)
             }
         };
     
         let quality = ConnectionQuality {
             latency_ms,
             packet_loss_rate: {
                 let v: f64 = 1.0 - stability_score;
                 if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v }
             },
             stability_score,
             last_updated: chrono::Utc::now(),
         };
         
         Ok(quality)
     }
     
     fn config(&self) -> &Self::Config {
         &self.config
     }
     
     async fn update_config(&mut self, config: Self::Config) -> Result<(), Self::Error> {
         self.config = config;
         Ok(())
     }
}

impl BoundedChannelProvider for ModernBinanceConnector {
    fn get_channel_stats(&self) -> HashMap<String, ChannelStats> {
        HashMap::new()
    }
    
    fn get_bounded_channel_config(&self) -> &BoundedChannelConfig {
        &self.config.channel_config
    }
    
    fn update_bounded_channel_config(&mut self, config: BoundedChannelConfig) {
        self.config.channel_config = config;
    }
}

#[async_trait]
impl BatchIOProvider for ModernBinanceConnector {
    type Error = ModernBinanceError;
    
    async fn get_batch_stats(&self) -> BatchIOStats {
        BatchIOStats {
            total_batches_processed: 0, // TODO: 实现实际统计
            average_batch_size: self.config.batch_config.max_batch_size as f64,
            total_processing_time_ms: 0,
            average_processing_time_ms: 0.0,
            average_processing_time: Duration::from_millis(100),
            last_batch_processed_at: None,
        }
    }

    fn get_batch_buffer_status(&self) -> BatchBufferStatus {
        BatchBufferStatus {
            current_size: 0,
            max_size: self.config.batch_config.max_buffer_size,
            usage_percentage: 0.0,
            pending_items: 0,
            last_flush: None,
            pending_writes: 0,
            pending_reads: 0,
            buffer_utilization: 0.0,
            buffer_usage_percent: 0.0,
            last_flush_time: None,
        }
    }

    fn get_batch_io_config(&self) -> &BatchIOConfig {
        &self.config.batch_config
    }

    fn update_batch_io_config(&mut self, config: BatchIOConfig) {
        self.config.batch_config = config;
    }

    async fn flush_all_batches(&self) -> Result<(), Self::Error> {
        // TODO: 执行缓冲区flush逻辑
        Ok(())
    }
}
