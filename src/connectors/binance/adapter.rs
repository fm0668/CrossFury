//! Binance现货连接器适配器
//! 
//! 实现ExchangeConnector trait，提供统一的接口

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use log::{info, warn};

use crate::connectors::traits::*;
use crate::types::*;
use super::config::BinanceConfig;
use super::spot::BinanceSpotConnector;
use super::websocket::BinanceWebSocketHandler;

/// Binance现货连接器适配器
/// 
/// 将Binance特定的实现适配到通用的ExchangeConnector接口
#[derive(Clone)]
pub struct BinanceAdapter {
    config: BinanceConfig,
    spot_connector: Arc<RwLock<Option<BinanceSpotConnector>>>,
    websocket_handler: Arc<RwLock<Option<BinanceWebSocketHandler>>>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    app_state: Arc<crate::AppState>,
    // 存储待订阅的信息
    pending_subscriptions: Arc<RwLock<(Vec<String>, Vec<DataType>)>>,
}

impl BinanceAdapter {
    /// 创建新的Binance适配器实例
    pub async fn new(
        config: BinanceConfig,
        app_state: Arc<crate::AppState>,
    ) -> Result<Self, ConnectorError> {
        Ok(Self {
            config,
            spot_connector: Arc::new(RwLock::new(None)),
            websocket_handler: Arc::new(RwLock::new(None)),
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            app_state,
            pending_subscriptions: Arc::new(RwLock::new((Vec::new(), Vec::new()))),
        })
    }
    
    /// 添加待订阅的市场数据
    pub async fn add_pending_subscription(&self, symbols: Vec<String>, data_types: Vec<DataType>) {
        let mut pending = self.pending_subscriptions.write().await;
        for symbol in symbols {
            if !pending.0.contains(&symbol) {
                pending.0.push(symbol);
            }
        }
        for data_type in data_types {
            if !pending.1.contains(&data_type) {
                pending.1.push(data_type);
            }
        }
    }
}

#[async_trait]
impl ExchangeConnector for BinanceAdapter {
    fn get_exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }
    
    fn get_market_type(&self) -> MarketType {
        MarketType::Spot
    }
    
    fn get_exchange_name(&self) -> &str {
        "Binance"
    }
    
    async fn connect_websocket(&self) -> Result<(), ConnectorError> {
        info!("[Binance] 开始连接WebSocket...");
        
        // 获取待订阅的信息
        let (symbols, data_types) = {
            let pending = self.pending_subscriptions.read().await;
            (pending.0.clone(), pending.1.clone())
        };
        
        // 创建WebSocket处理器
        let ws_handler = BinanceWebSocketHandler::new(
            self.config.clone(),
            self.app_state.clone(),
        ).await.map_err(|e| {
            ConnectorError::ConnectionFailed(format!("创建WebSocket处理器失败: {e}"))
        })?;
        
        // 如果有待订阅的数据，先设置订阅信息
        if !symbols.is_empty() && !data_types.is_empty() {
            info!("[Binance] 设置待订阅数据: symbols={symbols:?}, types={data_types:?}");
            ws_handler.subscribe_market_data(symbols, data_types).await.map_err(|e| {
                ConnectorError::SubscriptionFailed(format!("设置订阅失败: {e}"))
            })?;
        }
        
        // 连接WebSocket
        ws_handler.connect().await.map_err(|e| {
            ConnectorError::ConnectionFailed(format!("WebSocket连接失败: {e}"))
        })?;
        
        // 保存处理器实例
        {
            let mut handler_guard = self.websocket_handler.write().await;
            *handler_guard = Some(ws_handler);
        }
        
        // 更新连接状态
        {
            let mut status = self.connection_status.write().await;
            *status = ConnectionStatus::Connected;
        }
        
        info!("[Binance] WebSocket连接成功");
        Ok(())
    }
    
    async fn disconnect_websocket(&self) -> Result<(), ConnectorError> {
        info!("[Binance] 断开WebSocket连接...");
        
        // 断开WebSocket连接
        if let Some(handler) = self.websocket_handler.write().await.take() {
            handler.disconnect().await.map_err(|e| {
                ConnectorError::ConnectionFailed(format!("WebSocket断开失败: {e}"))
            })?;
        }
        
        // 更新连接状态
        {
            let mut status = self.connection_status.write().await;
            *status = ConnectionStatus::Disconnected;
        }
        
        info!("[Binance] WebSocket连接已断开");
        Ok(())
    }
    
    async fn subscribe_orderbook(&self, symbol: &str) -> Result<(), ConnectorError> {
        info!("[Binance] 订阅订单簿: {}", symbol);
        
        let handler_guard = self.websocket_handler.read().await;
        if let Some(handler) = handler_guard.as_ref() {
            handler.subscribe_market_data(
                vec![symbol.to_string()],
                vec![DataType::OrderBook]
            ).await.map_err(|e| {
                ConnectorError::SubscriptionFailed(format!("订阅订单簿失败: {e}"))
            })?;
            info!("[Binance] 订单簿订阅成功: {symbol}");
            Ok(())
        } else {
            Err(ConnectorError::ConnectionFailed("WebSocket未连接".to_string()))
        }
    }
    
    async fn subscribe_trades(&self, symbol: &str) -> Result<(), ConnectorError> {
        info!("[Binance] 订阅交易数据: {}", symbol);
        
        let handler_guard = self.websocket_handler.read().await;
        if let Some(handler) = handler_guard.as_ref() {
            handler.subscribe_market_data(
                vec![symbol.to_string()],
                vec![DataType::Trade]
            ).await.map_err(|e| {
                ConnectorError::SubscriptionFailed(format!("订阅交易数据失败: {e}"))
            })?;
            info!("[Binance] 交易数据订阅成功: {symbol}");
            Ok(())
        } else {
            Err(ConnectorError::ConnectionFailed("WebSocket未连接".to_string()))
        }
    }
    
    async fn subscribe_user_stream(&self) -> Result<(), ConnectorError> {
        warn!("[Binance] 用户数据流订阅暂未实现");
        Err(ConnectorError::TradingNotImplemented)
    }
    
    fn get_market_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage> {
        // 这里需要返回市场数据流接收器
        // 暂时创建一个空的接收器
        let (_tx, rx) = mpsc::unbounded_channel();
        rx
    }
    
    fn get_user_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage> {
        // 这里需要返回用户数据流接收器
        // 暂时创建一个空的接收器
        let (_tx, rx) = mpsc::unbounded_channel();
        rx
    }
    
    fn get_orderbook_snapshot(&self, symbol: &str) -> Option<StandardizedOrderBook> {
        info!("[Binance] 获取订单簿快照: {symbol}");
        // 暂时返回None，实际实现需要从本地缓存获取
        None
    }
    
    fn get_recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Vec<StandardizedTrade> {
        info!("[Binance] 获取最近交易快照: {symbol}, 限制: {limit}");
        // 暂时返回空向量，实际实现需要从本地缓存获取
        Vec::new()
    }
    
    async fn place_order(&self, _order: &OrderRequest) -> Result<OrderResponse, ConnectorError> {
        warn!("[Binance] 交易功能暂未实现");
        Err(ConnectorError::TradingNotImplemented)
    }
    
    async fn cancel_order(&self, order_id: &str, symbol: &str) -> Result<bool, ConnectorError> {
        warn!("[Binance] 取消订单功能暂未实现: order_id={}, symbol={}", order_id, symbol);
        Err(ConnectorError::TradingNotImplemented)
    }
    
    async fn get_order_status(&self, order_id: &str, symbol: &str) -> Result<OrderStatus, ConnectorError> {
        warn!("[Binance] 获取订单状态功能暂未实现: order_id={}, symbol={}", order_id, symbol);
        Err(ConnectorError::TradingNotImplemented)
    }
    
    async fn get_account_balance(&self) -> Result<AccountBalance, ConnectorError> {
        warn!("[Binance] 获取账户余额功能暂未实现");
        Err(ConnectorError::TradingNotImplemented)
    }
    
    fn is_connected(&self) -> bool {
        self.get_connection_status() == ConnectionStatus::Connected
    }
    
    fn is_websocket_connected(&self) -> bool {
        self.is_connected()
    }
    
    fn get_connection_status(&self) -> ConnectionStatus {
        // 这是同步方法，使用try_read避免阻塞
        self.connection_status.try_read()
            .map(|status| *status)
            .unwrap_or(ConnectionStatus::Disconnected)
    }
}

#[async_trait]
impl DataFlowManager for BinanceAdapter {
    fn take_market_data_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<HighFrequencyData>> {
        // 暂时返回None，实际实现需要返回高频数据接收器
        None
    }
    
    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<SystemEvent> {
        // 暂时创建一个空的接收器
        let (_tx, rx) = tokio::sync::broadcast::channel(100);
        rx
    }
    
    fn send_market_data(&self, _data: HighFrequencyData) -> Result<(), mpsc::error::SendError<HighFrequencyData>> {
        // 暂时返回成功，实际实现需要发送数据到队列
        Ok(())
    }
    
    async fn send_event(&self, _event: SystemEvent) {
        // 事件发送的简化实现
        // 实际实现需要将事件发送到事件总线
    }
}

#[async_trait]
impl ConnectorManager for BinanceAdapter {
    async fn add_connector(&mut self, _connector: Box<dyn ExchangeConnector>) -> Result<(), ConnectorError> {
        // 暂时返回未实现错误
        Err(ConnectorError::TradingNotImplemented)
    }
    
    async fn remove_connector(&mut self, _exchange: ExchangeType, _market_type: MarketType) -> Result<(), ConnectorError> {
        // 暂时返回未实现错误
        Err(ConnectorError::TradingNotImplemented)
    }
    
    fn get_connector(&self, exchange: ExchangeType, market_type: MarketType) -> Option<&dyn ExchangeConnector> {
        if exchange == ExchangeType::Binance && market_type == MarketType::Spot {
            Some(self)
        } else {
            None
        }
    }
    
    fn get_all_connectors(&self) -> Vec<&dyn ExchangeConnector> {
        vec![self]
    }
    
    async fn connect_all(&self) -> Result<(), ConnectorError> {
        self.connect_websocket().await
    }
    
    async fn disconnect_all(&self) -> Result<(), ConnectorError> {
        self.disconnect_websocket().await
    }
    
    async fn get_connection_status_all(&self) -> std::collections::HashMap<(ExchangeType, MarketType), ConnectionStatus> {
        let mut status_map = std::collections::HashMap::new();
        status_map.insert((ExchangeType::Binance, MarketType::Spot), self.get_connection_status());
        status_map
    }
}

// 为了向后兼容，保留一些额外的方法
impl BinanceAdapter {
    /// 订阅市场数据（向后兼容方法）
    pub async fn subscribe_market_data(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>,
    ) -> Result<(), ConnectorError> {
        info!("[Binance] 订阅市场数据: symbols={symbols:?}, types={data_types:?}");
        
        // 存储订阅信息
        {
            let mut pending = self.pending_subscriptions.write().await;
            pending.0.extend(symbols.clone());
            pending.1.extend(data_types.clone());
            // 去重
            pending.0.sort();
            pending.0.dedup();
            pending.1.sort();
            pending.1.dedup();
        }
        
        let handler_guard = self.websocket_handler.read().await;
        if let Some(handler) = handler_guard.as_ref() {
            // 如果已连接，立即订阅
            handler.subscribe_market_data(symbols, data_types).await.map_err(|e| {
                ConnectorError::SubscriptionFailed(format!("订阅市场数据失败: {e}"))
            })?;
            info!("[Binance] 市场数据订阅成功");
            Ok(())
        } else {
            // 如果未连接，先建立WebSocket连接
            info!("[Binance] WebSocket未连接，正在建立连接...");
            self.connect_websocket().await?;
            info!("[Binance] WebSocket连接已建立，订阅信息将自动生效");
            Ok(())
        }
    }
    
    /// 取消订阅市场数据（向后兼容方法）
    pub async fn unsubscribe_market_data(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>,
    ) -> Result<(), ConnectorError> {
        info!("[Binance] 取消订阅市场数据: symbols={symbols:?}, types={data_types:?}");
        
        let handler_guard = self.websocket_handler.read().await;
        if let Some(handler) = handler_guard.as_ref() {
            handler.unsubscribe_market_data(symbols, data_types).await.map_err(|e| {
                ConnectorError::SubscriptionFailed(format!("取消订阅失败: {e}"))
            })?;
            info!("[Binance] 取消订阅成功");
            Ok(())
        } else {
            Err(ConnectorError::ConnectionFailed("WebSocket未连接".to_string()))
        }
    }
    
    /// 健康检查（向后兼容方法）
    pub async fn health_check(&self) -> Result<HealthStatus, ConnectorError> {
        let status = self.get_connection_status();
        
        let health_status = match status {
            ConnectionStatus::Connected => HealthStatus::Healthy,
            ConnectionStatus::Connecting => HealthStatus::Degraded,
            ConnectionStatus::Disconnected => HealthStatus::Unhealthy,
            ConnectionStatus::Error => HealthStatus::Unhealthy,
            ConnectionStatus::Reconnecting => HealthStatus::Degraded,
        };
        
        Ok(health_status)
    }
    
    /// 获取连接统计（向后兼容方法）
    pub async fn get_connection_stats(&self) -> Result<ConnectionStats, ConnectorError> {
        // 基础连接统计信息
        let stats = ConnectionStats {
            connected_since: Some(chrono::Utc::now()), // 简化实现
            messages_received: 0, // 需要在实际实现中跟踪
            messages_sent: 0,
            reconnect_count: 0,
            last_ping_time: None,
            latency_ms: None,
        };
        
        Ok(stats)
    }
}