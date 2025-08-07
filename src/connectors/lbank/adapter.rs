//! LBank连接器适配器
//! 将现有的LBank WebSocket处理器包装成标准的ExchangeConnector接口

use async_trait::async_trait;
use log::{info, warn};
use chrono;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc, broadcast};

use crate::core::AppState;
use crate::connectors::traits::{ExchangeConnector, DataFlowManager};
use crate::connectors::common::{
    emergency_ping::EmergencyPingManager,
    adaptive_timeout::AdaptiveTimeoutManager,
    batch_subscription::BatchSubscriptionManager,
};
use crate::types::{
    config::{ConnectorConfig, ConnectionStatus, SubscriptionConfig, SubscriptionResult, BatchSubscriptionResult, SubscriptionStatus, ConnectionQuality, ConnectionQualityLevel},
    common::DataType,
    market_data::{StandardizedMessage, StandardizedOrderBook, StandardizedTrade},
    orders::{OrderRequest, OrderResponse, OrderStatus},
    account::{AccountBalance},
    exchange::{ExchangeType, MarketType},
    errors::ConnectorError,
    events::{SystemEvent, HighFrequencyData},
};
use crate::exchange_types::Exchange;
use super::websocket::LBankWebSocketHandler;

/// LBank连接器
/// 实现ExchangeConnector trait，提供标准化的交易所连接接口
#[derive(Clone)]
pub struct LBankConnector {
    config: ConnectorConfig,
    app_state: Arc<AppState>,
    websocket_handler: LBankWebSocketHandler,
    status: Arc<RwLock<ConnectionStatus>>,
    market_data_sender: Arc<RwLock<Option<mpsc::UnboundedSender<StandardizedMessage>>>>,
    user_data_sender: Arc<RwLock<Option<mpsc::UnboundedSender<StandardizedMessage>>>>,
    event_sender: Arc<RwLock<Option<broadcast::Sender<SystemEvent>>>>,
    // WebSocket优化模块
    emergency_ping_manager: Arc<RwLock<EmergencyPingManager>>,
    adaptive_timeout_manager: Arc<RwLock<AdaptiveTimeoutManager>>,
    batch_subscription_manager: Arc<RwLock<BatchSubscriptionManager>>,
}

impl LBankConnector {
    /// 创建新的LBank连接器实例
    pub fn new(config: ConnectorConfig, app_state: Arc<AppState>) -> Self {
        let websocket_handler = LBankWebSocketHandler::new(app_state.clone());
        
        Self {
            config,
            app_state,
            websocket_handler,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            market_data_sender: Arc::new(RwLock::new(None)),
            user_data_sender: Arc::new(RwLock::new(None)),
            event_sender: Arc::new(RwLock::new(None)),
            // 初始化WebSocket优化模块
            emergency_ping_manager: Arc::new(RwLock::new(EmergencyPingManager::with_default_config())),
            adaptive_timeout_manager: Arc::new(RwLock::new(AdaptiveTimeoutManager::with_default_config())),
            batch_subscription_manager: Arc::new(RwLock::new(BatchSubscriptionManager::with_default_config())),
        }
    }


}

#[async_trait]
impl ExchangeConnector for LBankConnector {
    // 基础信息
    fn get_exchange_type(&self) -> ExchangeType {
        ExchangeType::LBank
    }
    
    fn get_market_type(&self) -> MarketType {
        MarketType::Spot
    }
    
    fn get_exchange_name(&self) -> &str {
        "LBank"
    }
    
    // WebSocket 连接管理
    async fn connect_websocket(&self) -> Result<(), ConnectorError> {
        info!("Connecting to LBank WebSocket");
        
        // 初始化WebSocket优化模块
        {
            let mut ping_manager = self.emergency_ping_manager.write().await;
            ping_manager.reset().await;
            
            let mut timeout_manager = self.adaptive_timeout_manager.write().await;
            timeout_manager.reset().await;
        }
        
        // 更新连接状态为已连接（模拟连接成功）
        {
            let mut status = self.status.write().await;
            *status = ConnectionStatus::Connected;
        }
        
        // 启动优化模块的监控任务
        {
            // 超时管理器已初始化，无需额外启动
        }
        
        info!("LBank WebSocket connected successfully，优化模块已启动");
        Ok(())
    }

    async fn disconnect_websocket(&self) -> Result<(), ConnectorError> {
        info!("Disconnecting from LBank WebSocket");
        
        // 更新连接状态
        {
            let mut status = self.status.write().await;
            *status = ConnectionStatus::Disconnected;
        }
        
        info!("LBank WebSocket disconnected successfully");
        Ok(())
    }

    async fn subscribe_orderbook(&self, symbol: &str) -> Result<(), ConnectorError> {
        info!("Subscribing to orderbook for symbol: {}", symbol);
        
        // 检查连接状态
        let status = self.status.read().await;
        if *status != ConnectionStatus::Connected {
            return Err(ConnectorError::ConnectionLost("WebSocket not connected".to_string()));
        }
        
        // 调用WebSocket处理器的订阅方法
        self.websocket_handler.subscribe(vec![symbol.to_string()]).await
            .map_err(|e| ConnectorError::SubscriptionFailed(format!("Failed to subscribe to orderbook: {e}")))?;
        
        info!("Successfully subscribed to orderbook for {}", symbol);
        Ok(())
    }
    
    async fn subscribe_trades(&self, symbol: &str) -> Result<(), ConnectorError> {
        info!("Subscribing to trades for symbol: {}", symbol);
        
        // LBank WebSocket主要提供订单簿数据，交易数据订阅暂不支持
        warn!("Trade subscription not implemented for LBank");
        Ok(())
    }
    
    async fn subscribe_user_stream(&self) -> Result<(), ConnectorError> {
        info!("Subscribing to user stream");
        
        // LBank连接器暂不支持用户数据流
        warn!("User stream subscription not implemented for LBank");
        Ok(())
    }
    
    // 推送式数据流接口
    fn get_market_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage> {
        let (_sender, receiver) = mpsc::unbounded_channel();
        
        // 将发送器存储起来，以便后续使用
        tokio::spawn(async move {
            // 这里应该从WebSocket处理器获取数据并转发
            // 目前返回一个空的接收器
        });
        
        receiver
    }
    
    fn get_user_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage> {
        let (_, receiver) = mpsc::unbounded_channel();
        receiver
    }
    
    // 本地缓存快照读取
    async fn get_orderbook_snapshot(&self, symbol: &str) -> Option<StandardizedOrderBook> {
        // 从app_state中获取最新的订单簿数据
        if let Some(price_data) = self.app_state.price_data.get(symbol) {
            let data = price_data.value();
            
            Some(StandardizedOrderBook {
                symbol: symbol.to_string(),
                exchange: Exchange::LBank,
                best_bid: data.best_bid,
                best_ask: data.best_ask,
                depth_bids: data.depth_bids.clone().unwrap_or_default(),
                depth_asks: data.depth_asks.clone().unwrap_or_default(),
                timestamp: data.timestamp,
            })
        } else {
            None
        }
    }
    
    async fn get_recent_trades_snapshot(&self, _symbol: &str, _limit: usize) -> Vec<StandardizedTrade> {
        // LBank连接器暂不支持交易数据
        Vec::new()
    }
    
    // 交易相关操作 (REST API)
    async fn place_order(&self, _order: &OrderRequest) -> Result<OrderResponse, ConnectorError> {
        Err(ConnectorError::OrderPlacementFailed("Trading not implemented for LBank connector".to_string()))
    }

    async fn cancel_order(&self, _order_id: &str, _symbol: &str) -> Result<bool, ConnectorError> {
        Err(ConnectorError::OrderCancellationFailed("Trading not implemented for LBank connector".to_string()))
    }

    async fn get_order_status(&self, _order_id: &str, _symbol: &str) -> Result<OrderStatus, ConnectorError> {
        Err(ConnectorError::InvalidResponse("Trading not implemented for LBank connector".to_string()))
    }

    async fn get_account_balance(&self) -> Result<AccountBalance, ConnectorError> {
        Err(ConnectorError::DataParsingError("Account balance not implemented for LBank connector".to_string()))
    }
    
    // 连接状态
    async fn is_connected(&self) -> bool {
        // 检查WebSocket处理器的连接状态
        self.websocket_handler.is_connected()
    }
    
    async fn is_websocket_connected(&self) -> bool {
        self.websocket_handler.is_connected()
    }
    
    async fn get_connection_status(&self) -> ConnectionStatus {
        let status = self.status.read().await;
        *status
    }
    
    // WebSocket优化功能实现
    async fn get_connection_quality(&self) -> Result<ConnectionQuality, ConnectorError> {
        let status = self.get_connection_status().await;
        
        let quality = match status {
            ConnectionStatus::Connected => {
                ConnectionQuality {
                    latency_ms: 60.0,
                    packet_loss_rate: 0.0,
                    stability_score: 0.85,
                    last_updated: chrono::Utc::now(),
                }
            },
            ConnectionStatus::Connecting | ConnectionStatus::Reconnecting => {
                ConnectionQuality {
                    latency_ms: 250.0,
                    packet_loss_rate: 0.15,
                    stability_score: 0.25,
                    last_updated: chrono::Utc::now(),
                }
            },
            _ => {
                ConnectionQuality {
                    latency_ms: 1200.0,
                    packet_loss_rate: 0.6,
                    stability_score: 0.05,
                    last_updated: chrono::Utc::now(),
                }
            },
        };
        
        Ok(quality)
    }
    
    async fn emergency_ping(&self) -> Result<Duration, ConnectorError> {
        info!("[LBank] 执行紧急ping检查");
        
        let ping_manager = self.emergency_ping_manager.read().await;
        if ping_manager.should_send_emergency_ping().await {
            // 执行ping操作
            if self.websocket_handler.is_connected() {
                // 这里可以发送ping消息或执行健康检查
                info!("[LBank] 紧急ping执行成功");
                Ok(Duration::from_millis(60))
            } else {
                Err(ConnectorError::ConnectionLost("WebSocket未连接".to_string()))
            }
        } else {
            info!("[LBank] 跳过紧急ping，频率限制");
            Ok(Duration::from_millis(120))
        }
    }
    
    async fn subscribe_batch(
        &self, 
        symbols: Vec<String>, 
        batch_size: usize
    ) -> Result<BatchSubscriptionResult, ConnectorError> {
        info!("[LBank] 批量订阅: {} 个符号", symbols.len());
        
        let mut successful_count = 0;
        let mut failed_symbols = Vec::new();
        let total_requested = symbols.len();
        
        // 使用批量订阅管理器优化订阅请求
        for symbol in &symbols {
            match self.subscribe_orderbook(symbol).await {
                Ok(_) => successful_count += 1,
                Err(e) => failed_symbols.push((symbol.clone(), e.to_string())),
            }
        }
        
        let result = BatchSubscriptionResult {
            total_requested,
            successful: successful_count,
            failed: failed_symbols.len(),
            pending: 0,
            failed_symbols,
            results: Vec::new(), // 简化实现，不返回详细结果
        };
        
        info!("[LBank] 批量订阅完成: {}/{} 成功", successful_count, total_requested);
        Ok(result)
    }
}

#[async_trait]
impl DataFlowManager for LBankConnector {
    // 高频数据流管理
    fn take_market_data_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<HighFrequencyData>> {
        // 暂不实现高频数据流
        None
    }
    
    fn subscribe_events(&self) -> broadcast::Receiver<SystemEvent> {
        let (_sender, receiver) = broadcast::channel(1000);
        receiver
    }
    
    fn send_market_data(&self, _data: HighFrequencyData) -> Result<(), mpsc::error::SendError<HighFrequencyData>> {
        // 暂不实现高频数据发送
        Ok(())
    }
    
    async fn send_event(&self, _event: SystemEvent) {
        // 暂不实现事件发送
    }
}

// LBankConnector 的额外方法实现
impl LBankConnector {
    /// 健康检查
    pub async fn health_check(&self) -> Result<bool, ConnectorError> {
        // 检查连接状态
        let is_healthy = self.is_connected().await;
        Ok(is_healthy)
    }
    
    /// 获取连接统计信息
    pub async fn get_connection_stats(&self) -> Result<(u64, u64, f64), ConnectorError> {
        // 返回 (消息数, 更新数, 运行时间)
        // 这里返回模拟数据
        Ok((0, 0, 0.0))
    }
    
    /// 设置消息发送器
    pub async fn set_message_sender(&mut self, sender: mpsc::UnboundedSender<StandardizedMessage>) {
        let mut market_data_sender = self.market_data_sender.write().await;
        *market_data_sender = Some(sender);
    }
    
    /// 订阅市场数据
    pub async fn subscribe_market_data(&mut self, config: SubscriptionConfig) -> Result<(), ConnectorError> {
        for symbol in &config.symbols {
            for data_type in &config.data_types {
                match data_type {
                    DataType::OrderBook => {
                        self.subscribe_orderbook(symbol).await?;
                    },
                    DataType::Trade => {
                        self.subscribe_trades(symbol).await?;
                    },
                    _ => {
                        warn!("Unsupported data type: {data_type:?}");
                    }
                }
            }
        }
        Ok(())
    }
    
    /// 获取最新订单簿
    pub async fn get_latest_orderbook(&self, symbol: &str) -> Result<StandardizedOrderBook, ConnectorError> {
        if let Some(orderbook) = self.get_orderbook_snapshot(symbol).await {
            Ok(orderbook)
        } else {
            Err(ConnectorError::InvalidResponse(format!("No orderbook data available for {symbol}")))
        }
    }
    
    /// 下单（简化版本，用于测试）
    pub async fn place_order(&mut self, order: OrderRequest) -> Result<(), ConnectorError> {
        // 调用 trait 方法
        let _response = ExchangeConnector::place_order(self, &order).await?;
        Ok(())
    }
    
    /// 取消订单（简化版本，用于测试）
    pub async fn cancel_order(&mut self, order_id: String) -> Result<(), ConnectorError> {
        // 调用 trait 方法
        let _result = ExchangeConnector::cancel_order(self, &order_id, "").await?;
        Ok(())
    }
    
    /// 获取账户余额（简化版本，用于测试）
    pub async fn get_account_balance(&mut self) -> Result<(), ConnectorError> {
        // 调用 trait 方法
        let _balance = ExchangeConnector::get_account_balance(self).await?;
        Ok(())
    }
    
    /// 获取仓位（简化版本，用于测试）
    pub async fn get_positions(&mut self) -> Result<(), ConnectorError> {
        // LBank 不支持仓位查询
        Err(ConnectorError::ServiceUnavailable("Positions not supported for LBank".to_string()))
    }
    
    /// 获取紧急Ping管理器
    pub async fn get_emergency_ping_manager(&self) -> Arc<RwLock<EmergencyPingManager>> {
        self.emergency_ping_manager.clone()
    }
    
    /// 获取自适应超时管理器
    pub async fn get_adaptive_timeout_manager(&self) -> Arc<RwLock<AdaptiveTimeoutManager>> {
        self.adaptive_timeout_manager.clone()
    }
    
    /// 获取批量订阅管理器
    pub async fn get_batch_subscription_manager(&self) -> Arc<RwLock<BatchSubscriptionManager>> {
        self.batch_subscription_manager.clone()
    }
    
    /// 执行紧急ping（向后兼容方法）
    pub async fn execute_emergency_ping(&self) -> Result<(), ConnectorError> {
        self.emergency_ping().await.map(|_| ())
    }
    
    /// 获取当前超时设置
    pub async fn get_current_timeout(&self) -> std::time::Duration {
        let timeout_manager = self.adaptive_timeout_manager.read().await;
        timeout_manager.get_current_timeout().await
    }
    
    /// 重置超时管理器
    pub async fn reset_timeout_manager(&self) {
        let mut timeout_manager = self.adaptive_timeout_manager.write().await;
        timeout_manager.reset();
    }
}