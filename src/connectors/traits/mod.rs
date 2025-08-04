// src/connectors/traits/mod.rs - 严格按照核心Trait定义实现

use async_trait::async_trait;
use tokio::sync::{mpsc, broadcast};
use std::collections::HashMap;
use crate::types::*;

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