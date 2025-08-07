// src/connectors/traits/mod.rs - 严格按照核心Trait定义实现

use async_trait::async_trait;
use tokio::sync::{mpsc, broadcast};
use std::collections::HashMap;
use std::time::Duration;
use chrono;
use crate::types::*;
use crate::types::config::BatchSubscriptionResult;

/// ExchangeConnector trait - 完全按照CrossFury_核心Trait定义.md实现
#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    // 基础信息
    fn get_exchange_type(&self) -> ExchangeType;
    fn get_market_type(&self) -> MarketType;
    fn get_exchange_name(&self) -> &str;
    
    // WebSocket 连接管理
    async fn connect_websocket(&self) -> Result<(), ConnectorError>;
    async fn disconnect_websocket(&self) -> Result<(), ConnectorError>;
    async fn subscribe_orderbook(&self, symbol: &str) -> Result<(), ConnectorError>;
    async fn subscribe_trades(&self, symbol: &str) -> Result<(), ConnectorError>;
    async fn subscribe_user_stream(&self) -> Result<(), ConnectorError>;
    
    // 推送式数据流接口
    fn get_market_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage>;
    fn get_user_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage>;
    
    // 本地缓存快照读取
    async fn get_orderbook_snapshot(&self, symbol: &str) -> Option<StandardizedOrderBook>;
    async fn get_recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Vec<StandardizedTrade>;
    
    // 交易相关操作 (REST API)
    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse, ConnectorError>;
    async fn cancel_order(&self, order_id: &str, symbol: &str) -> Result<bool, ConnectorError>;
    async fn get_order_status(&self, order_id: &str, symbol: &str) -> Result<OrderStatus, ConnectorError>;
    async fn get_account_balance(&self) -> Result<AccountBalance, ConnectorError>;
    
    // 连接状态
    async fn is_connected(&self) -> bool;
    async fn is_websocket_connected(&self) -> bool;
    async fn get_connection_status(&self) -> ConnectionStatus;
    
    // 高级连接管理功能 - 提供默认实现以保持向后兼容
    async fn get_connection_quality(&self) -> Result<ConnectionQuality, ConnectorError> {
        // 默认实现：返回基础连接质量信息
        Ok(ConnectionQuality {
            latency_ms: 100.0,
            packet_loss_rate: 0.0,
            stability_score: 0.8,
            last_updated: chrono::Utc::now(),
        })
    }
    
    async fn emergency_ping(&self) -> Result<Duration, ConnectorError> {
        // 默认实现：返回模拟的ping时间
        Ok(Duration::from_millis(50))
    }
    
    async fn subscribe_batch(
        &self, 
        symbols: Vec<String>, 
        batch_size: usize
    ) -> Result<BatchSubscriptionResult, ConnectorError> {
        // 默认实现：逐个订阅（回退到基础实现）
        let mut successful_count = 0;
        let mut failed_symbols = Vec::new();
        let total_requested = symbols.len();
        
        for symbol in symbols {
            match self.subscribe_orderbook(&symbol).await {
                Ok(_) => successful_count += 1,
                Err(e) => failed_symbols.push((symbol, e.to_string())),
            }
        }
        
        Ok(BatchSubscriptionResult {
            total_requested,
            successful: successful_count,
            failed: failed_symbols.len(),
            pending: 0,
            failed_symbols,
            results: Vec::new(),
        })
    }
    
    async fn get_subscription_status(&self) -> Result<HashMap<String, SubscriptionStatus>, ConnectorError> {
        // 默认实现：返回空的订阅状态
        Ok(HashMap::new())
    }
}

/// DataFlowManager trait - 完全按照核心Trait定义实现
#[async_trait]
pub trait DataFlowManager: Send + Sync {
    // 高频数据流管理
    fn take_market_data_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<HighFrequencyData>>;
    fn subscribe_events(&self) -> broadcast::Receiver<SystemEvent>;
    fn send_market_data(&self, data: HighFrequencyData) -> Result<(), mpsc::error::SendError<HighFrequencyData>>;
    async fn send_event(&self, event: SystemEvent);
}

/// ConnectorManager trait - 完全按照核心Trait定义实现
#[async_trait]
pub trait ConnectorManager: Send + Sync {
    // 连接器管理
    async fn add_connector(&mut self, connector: Box<dyn ExchangeConnector>) -> Result<(), ConnectorError>;
    async fn remove_connector(&mut self, exchange: ExchangeType, market_type: MarketType) -> Result<(), ConnectorError>;
    fn get_connector(&self, exchange: ExchangeType, market_type: MarketType) -> Option<&dyn ExchangeConnector>;
    fn get_all_connectors(&self) -> Vec<&dyn ExchangeConnector>;
    
    // 批量操作
    async fn connect_all(&self) -> Result<(), ConnectorError>;
    async fn disconnect_all(&self) -> Result<(), ConnectorError>;
    async fn get_connection_status_all(&self) -> HashMap<(ExchangeType, MarketType), ConnectionStatus>;
}