//! Binance期货连接器主模块
//! 
//! 整合WebSocket、REST API和消息解析功能，实现完整的期货连接器

use crate::connectors::binance::futures::config::{BinanceFuturesConfig, MarginType as ConfigMarginType};
use crate::connectors::binance::futures::websocket::*;
use crate::connectors::binance::futures::rest_api::{BinanceFuturesRestClient, MarginType as RestMarginType};
use crate::connectors::binance::futures::message_parser::*;
use crate::types::market_data::*;
use crate::types::trading::{*, TimeInForce as TradingTimeInForce, PositionSide as TradingPositionSide};
use crate::core::AppError;

// 导入核心Trait和标准化类型
use crate::connectors::traits::ExchangeConnector;
use crate::types::{
    ExchangeType, MarketType, StandardizedMessage, StandardizedOrderBook, 
    StandardizedTrade, OrderRequest, OrderResponse, 
    OrderStatus, AccountBalance, ConnectorError, ConnectionStatus
};
use async_trait::async_trait;

// 定义Result类型别名
pub type Result<T> = std::result::Result<T, AppError>;

use tokio::sync::{mpsc, RwLock};
use futures_util::StreamExt;
use std::sync::Arc;
use std::collections::HashMap;
use log::{info, warn};
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Binance期货连接器
pub struct BinanceFuturesConnector {
    /// 配置信息
    config: BinanceFuturesConfig,
    /// WebSocket处理器
    ws_handler: BinanceFuturesWebSocketHandler,
    /// REST API客户端
    rest_client: BinanceFuturesRestClient,
    /// 消息解析器
    message_parser: BinanceFuturesMessageParser,
    /// 市场数据发送通道
    market_data_sender: Option<mpsc::UnboundedSender<MarketDataEvent>>,
    /// 交易事件发送通道
    trade_event_sender: Option<mpsc::UnboundedSender<TradeEvent>>,
    /// 账户事件发送通道
    account_event_sender: Option<mpsc::UnboundedSender<AccountEvent>>,
    /// 连接状态
    connection_state: Arc<RwLock<ConnectionState>>,
    /// 订阅状态
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionState>>>,
    /// 用户数据流监听密钥
    listen_key: Arc<RwLock<Option<String>>>,
    /// 最后心跳时间
    last_heartbeat: Arc<RwLock<DateTime<Utc>>>,
    /// 重连计数
    reconnect_count: Arc<RwLock<u32>>,
    
    // ExchangeConnector trait 所需的数据流
    /// 标准化市场数据流发送端
    standardized_market_sender: Option<mpsc::UnboundedSender<StandardizedMessage>>,
    /// 标准化市场数据流接收端
    standardized_market_receiver: Option<mpsc::UnboundedReceiver<StandardizedMessage>>,
    /// 标准化用户数据流发送端
    standardized_user_sender: Option<mpsc::UnboundedSender<StandardizedMessage>>,
    /// 标准化用户数据流接收端
    standardized_user_receiver: Option<mpsc::UnboundedReceiver<StandardizedMessage>>,
    /// 本地订单簿缓存
    orderbook_cache: Arc<RwLock<HashMap<String, StandardizedOrderBook>>>,
    /// 本地交易数据缓存
    trades_cache: Arc<RwLock<HashMap<String, Vec<StandardizedTrade>>>>,
}

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 重连中
    Reconnecting,
    /// 连接错误
    Error(String),
}

/// 订阅状态
#[derive(Debug, Clone)]
pub struct SubscriptionState {
    pub stream_name: String,
    pub symbol: String,
    pub is_active: bool,
    pub last_update: DateTime<Utc>,
    pub error_count: u32,
}

/// 连接器统计信息
#[derive(Debug, Clone)]
pub struct ConnectorStats {
    pub connection_state: ConnectionState,
    pub active_subscriptions: u32,
    pub total_messages_received: u64,
    pub total_messages_sent: u64,
    pub last_message_time: Option<DateTime<Utc>>,
    pub reconnect_count: u32,
    pub uptime: chrono::Duration,
}

impl BinanceFuturesConnector {
    /// 创建新的期货连接器
    pub fn new(config: BinanceFuturesConfig) -> Self {
        let ws_handler = BinanceFuturesWebSocketHandler::new(config.clone());
        let rest_client = BinanceFuturesRestClient::new(config.clone());
        let message_parser = BinanceFuturesMessageParser;
        
        // 创建标准化数据流通道
        let (market_tx, market_rx) = mpsc::unbounded_channel::<StandardizedMessage>();
        let (user_tx, user_rx) = mpsc::unbounded_channel::<StandardizedMessage>();
        
        Self {
            config,
            ws_handler,
            rest_client,
            message_parser,
            market_data_sender: None,
            trade_event_sender: None,
            account_event_sender: None,
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            listen_key: Arc::new(RwLock::new(None)),
            last_heartbeat: Arc::new(RwLock::new(Utc::now())),
            reconnect_count: Arc::new(RwLock::new(0)),
            standardized_market_sender: Some(market_tx),
            standardized_market_receiver: Some(market_rx),
            standardized_user_sender: Some(user_tx),
            standardized_user_receiver: Some(user_rx),
            orderbook_cache: Arc::new(RwLock::new(HashMap::new())),
            trades_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 设置市场数据发送通道
    pub fn set_market_data_sender(&mut self, sender: mpsc::UnboundedSender<MarketDataEvent>) {
        self.market_data_sender = Some(sender.clone());
        self.ws_handler.set_data_sender(sender);
    }
    
    /// 设置交易事件发送通道
    pub fn set_trade_event_sender(&mut self, sender: mpsc::UnboundedSender<TradeEvent>) {
        self.trade_event_sender = Some(sender.clone());
        self.ws_handler.set_trade_sender(sender);
    }
    
    /// 设置账户事件发送通道
    pub fn set_account_event_sender(&mut self, sender: mpsc::UnboundedSender<AccountEvent>) {
        self.account_event_sender = Some(sender.clone());
        self.ws_handler.set_account_sender(sender);
    }
    
    /// 连接到交易所
    pub async fn connect(&mut self) -> Result<()> {
        info!("开始连接Binance期货交易所");
        
        // 更新连接状态
        *self.connection_state.write().await = ConnectionState::Connecting;
        
        // 连接WebSocket
        match self.ws_handler.connect().await {
            Ok(_) => {
                *self.connection_state.write().await = ConnectionState::Connected;
                *self.last_heartbeat.write().await = Utc::now();
                info!("Binance期货WebSocket连接成功");
            }
            Err(e) => {
                let error_msg = format!("WebSocket连接失败: {e}");
                *self.connection_state.write().await = ConnectionState::Error(error_msg.clone());
                return Err(AppError::ConnectionError(error_msg));
            }
        }
        
        // 如果配置了API密钥，启动用户数据流
        if self.config.api_key.is_some() && self.config.secret_key.is_some() {
            match self.start_user_data_stream().await {
                Ok(listen_key) => {
                    *self.listen_key.write().await = Some(listen_key);
                    info!("用户数据流启动成功");
                }
                Err(e) => {
                    warn!("用户数据流启动失败: {e}");
                }
            }
        }
        
        // 订阅配置中指定的交易对
        let symbols = self.config.subscribed_symbols.clone();
        for symbol in &symbols {
            if let Err(e) = self.subscribe_symbol_data(symbol).await {
                warn!("订阅{symbol}数据失败: {e}");
            }
        }
        
        Ok(())
    }
    
    /// 断开连接
    pub async fn disconnect(&mut self) -> Result<()> {
        info!("断开Binance期货连接");
        
        // 关闭用户数据流
        if let Some(listen_key) = self.listen_key.read().await.as_ref() {
            if let Err(e) = self.rest_client.close_user_data_stream(listen_key).await {
                warn!("关闭用户数据流失败: {e}");
            }
        }
        
        // 断开WebSocket连接
        self.ws_handler.disconnect().await?;
        
        // 更新状态
        *self.connection_state.write().await = ConnectionState::Disconnected;
        self.subscriptions.write().await.clear();
        *self.listen_key.write().await = None;
        
        info!("Binance期货连接已断开");
        Ok(())
    }
    
    /// 检查连接状态（异步版本）
    pub async fn is_connected_async(&self) -> bool {
        matches!(*self.connection_state.read().await, ConnectionState::Connected)
    }
    
    /// 获取连接状态
    pub async fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.read().await.clone()
    }
    
    /// 订阅交易对的所有数据
    pub async fn subscribe_symbol_data(&mut self, symbol: &str) -> Result<()> {
        info!("订阅{symbol}的期货数据");
        
        // 订阅深度数据
        self.ws_handler.subscribe_depth(symbol, Some(20)).await?;
        
        // 订阅交易数据
        self.ws_handler.subscribe_trades(symbol).await?;
        
        // 订阅24小时价格统计
        self.ws_handler.subscribe_ticker(symbol).await?;
        
        // 订阅标记价格
        self.ws_handler.subscribe_mark_price(symbol).await?;
        
        // 更新订阅状态
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(symbol.to_string(), SubscriptionState {
            stream_name: format!("{}_all", symbol.to_lowercase()),
            symbol: symbol.to_string(),
            is_active: true,
            last_update: Utc::now(),
            error_count: 0,
        });
        
        info!("{symbol}期货数据订阅成功");
        Ok(())
    }
    
    /// 取消订阅交易对数据
    pub async fn unsubscribe_symbol_data(&mut self, symbol: &str) -> Result<()> {
        info!("取消订阅{symbol}的期货数据");
        
        // 取消各种数据订阅
        let depth_stream = format!("{}@depth20@100ms", symbol.to_lowercase());
        let trade_stream = format!("{}@aggTrade", symbol.to_lowercase());
        let ticker_stream = format!("{}@ticker", symbol.to_lowercase());
        let mark_price_stream = format!("{}@markPrice@1s", symbol.to_lowercase());
        
        self.ws_handler.unsubscribe(&depth_stream).await?;
        self.ws_handler.unsubscribe(&trade_stream).await?;
        self.ws_handler.unsubscribe(&ticker_stream).await?;
        self.ws_handler.unsubscribe(&mark_price_stream).await?;
        
        // 更新订阅状态
        self.subscriptions.write().await.remove(symbol);
        
        info!("{symbol}期货数据取消订阅成功");
        Ok(())
    }
    
    /// 订阅K线数据
    pub async fn subscribe_klines(&mut self, symbol: &str, interval: &str) -> Result<()> {
        self.ws_handler.subscribe_klines(symbol, interval).await
    }
    
    /// 订阅资金费率
    pub async fn subscribe_funding_rates(&mut self) -> Result<()> {
        self.ws_handler.subscribe_funding_rate().await
    }
    
    /// 获取活跃订阅列表
    pub async fn get_active_subscriptions(&self) -> Vec<String> {
        self.subscriptions.read().await
            .iter()
            .filter(|(_, state)| state.is_active)
            .map(|(symbol, _)| symbol.clone())
            .collect()
    }
    
    /// 健康检查
    pub async fn health_check(&self) -> bool {
        // 检查连接状态
        if !self.is_connected_async().await {
            return false;
        }
        
        // 检查心跳时间
        let last_heartbeat = *self.last_heartbeat.read().await;
        let now = Utc::now();
        let heartbeat_timeout = chrono::Duration::seconds(30);
        
        if now - last_heartbeat > heartbeat_timeout {
            warn!("心跳超时，最后心跳时间: {last_heartbeat}");
            return false;
        }
        
        true
    }
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> ConnectorStats {
        let connection_state = self.connection_state.read().await.clone();
        let subscriptions = self.subscriptions.read().await;
        let reconnect_count = *self.reconnect_count.read().await;
        
        ConnectorStats {
            connection_state,
            active_subscriptions: subscriptions.len() as u32,
            total_messages_received: 0, // TODO: 实现消息计数
            total_messages_sent: 0,     // TODO: 实现消息计数
            last_message_time: Some(*self.last_heartbeat.read().await),
            reconnect_count,
            uptime: chrono::Duration::seconds(0), // TODO: 实现运行时间计算
        }
    }
    
    /// 启动用户数据流
    async fn start_user_data_stream(&self) -> Result<String> {
        self.rest_client.start_user_data_stream().await
    }
    
    // REST API 方法代理
    
    /// 获取服务器时间
    pub async fn get_server_time(&self) -> Result<u64> {
        self.rest_client.get_server_time().await
    }
    
    /// 获取交易所信息
    pub async fn get_exchange_info(&self) -> Result<Value> {
        self.rest_client.get_exchange_info().await
    }
    
    /// 获取深度数据
    pub async fn get_depth(&self, symbol: &str, limit: Option<u16>) -> Result<Value> {
        self.rest_client.get_depth(symbol, limit).await
    }
    
    /// 获取24小时价格统计
    pub async fn get_24hr_ticker(&self, symbol: Option<&str>) -> Result<Value> {
        self.rest_client.get_24hr_ticker(symbol).await
    }
    
    /// 获取标记价格
    pub async fn get_mark_price(&self, symbol: Option<&str>) -> Result<Value> {
        self.rest_client.get_mark_price(symbol).await
    }
    
    /// 获取资金费率历史
    pub async fn get_funding_rate(&self, symbol: &str, start_time: Option<u64>, end_time: Option<u64>, limit: Option<u16>) -> Result<Value> {
        self.rest_client.get_funding_rate(symbol, start_time, end_time, limit).await
    }
    
    /// 获取持仓量
    pub async fn get_open_interest(&self, symbol: &str) -> Result<Value> {
        self.rest_client.get_open_interest(symbol).await
    }
    
    /// 获取账户信息
    pub async fn get_account_info(&self) -> Result<Value> {
        self.rest_client.get_account_info().await
    }
    
    /// 获取持仓信息
    pub async fn get_position_info(&self, symbol: Option<&str>) -> Result<Value> {
        self.rest_client.get_position_info(symbol).await
    }
    
    /// 下单
    pub async fn place_order(&self, request: &LocalOrderRequest) -> Result<Value> {
        // 转换为 REST API 所需的类型
         let rest_request = crate::connectors::binance::futures::rest_api::OrderRequest {
             symbol: request.symbol.clone(),
             side: match request.side.as_str() {
                 "BUY" => OrderSide::Buy,
                 "SELL" => OrderSide::Sell,
                 _ => return Err(AppError::ParseError("Invalid order side".to_string())),
             },
             order_type: match request.order_type.as_str() {
                 "MARKET" => OrderType::Market,
                 "LIMIT" => OrderType::Limit,
                 "STOP" => OrderType::Stop,
                 "STOP_MARKET" => OrderType::StopMarket,
                 "TAKE_PROFIT" => OrderType::TakeProfit,
                 "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
                 _ => return Err(AppError::ParseError("Invalid order type".to_string())),
             },
             quantity: request.quantity,
             price: request.price,
             time_in_force: request.time_in_force.as_ref().map(|tif| match tif.as_str() {
                 "GTC" => TradingTimeInForce::GTC,
                 "IOC" => TradingTimeInForce::IOC,
                 "FOK" => TradingTimeInForce::FOK,
                 "GTX" => TradingTimeInForce::GTX,
                 _ => TradingTimeInForce::GTC,
             }),
             position_side: request.position_side.as_ref().map(|ps| match ps.as_str() {
                 "LONG" => TradingPositionSide::Long,
                 "SHORT" => TradingPositionSide::Short,
                 "BOTH" => TradingPositionSide::Both,
                 _ => TradingPositionSide::Both,
             }),
             close_position: request.close_position,
             activation_price: None,
             callback_rate: None,
             working_type: None,
             price_protect: None,
             reduce_only: request.reduce_only,
             client_order_id: request.client_order_id.clone(), // 修正字段名
         };
        self.rest_client.place_order(&rest_request).await
    }
    
    /// 取消订单
    pub async fn cancel_order(&self, symbol: &str, order_id: Option<u64>, orig_client_order_id: Option<&str>) -> Result<Value> {
        self.rest_client.cancel_order(symbol, order_id, orig_client_order_id).await
    }
    
    /// 查询订单
    pub async fn query_order(&self, symbol: &str, order_id: Option<u64>, orig_client_order_id: Option<&str>) -> Result<Value> {
        self.rest_client.query_order(symbol, order_id, orig_client_order_id).await
    }
    
    /// 调整杠杆
    pub async fn change_leverage(&self, symbol: &str, leverage: u8) -> Result<Value> {
        self.rest_client.change_leverage(symbol, leverage).await
    }
    
    /// 调整保证金模式
    pub async fn change_margin_type(&self, symbol: &str, margin_type: ConfigMarginType) -> Result<Value> {
        // 转换为 REST API 所需的类型
        let rest_margin_type = match margin_type {
            ConfigMarginType::Isolated => RestMarginType::Isolated,
            ConfigMarginType::Crossed => RestMarginType::Cross,
        };
        self.rest_client.change_margin_type(symbol, rest_margin_type).await
    }
    
    /// 调整持仓模式
    pub async fn change_position_mode(&self, dual_side_position: bool) -> Result<Value> {
        self.rest_client.change_position_mode(dual_side_position).await
    }
}

/// 本地订单请求结构体
#[derive(Debug, Clone)]
pub struct LocalOrderRequest {
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub time_in_force: Option<String>,
    pub reduce_only: Option<bool>,
    pub close_position: Option<bool>,
    pub position_side: Option<String>,
    pub client_order_id: Option<String>,
}

// 实现 ExchangeConnector trait
#[async_trait]
impl ExchangeConnector for BinanceFuturesConnector {
    // 基础信息
    fn get_exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }
    
    fn get_market_type(&self) -> MarketType {
        MarketType::Futures
    }
    
    fn get_exchange_name(&self) -> &str {
        "Binance Futures"
    }
    
    // WebSocket 连接管理
    async fn connect_websocket(&self) -> std::result::Result<(), ConnectorError> {
        // 由于trait要求&self，但ws_handler需要&mut，这里需要使用内部可变性
        // 暂时返回未实现错误，需要重构ws_handler使用内部可变性
        Err(ConnectorError::ConnectionError("WebSocket连接需要重构以支持内部可变性".to_string()))
    }
    
    async fn disconnect_websocket(&self) -> std::result::Result<(), ConnectorError> {
        // 由于trait要求&self，但ws_handler需要&mut，这里需要使用内部可变性
        // 暂时返回未实现错误，需要重构ws_handler使用内部可变性
        Err(ConnectorError::ConnectionError("WebSocket断开需要重构以支持内部可变性".to_string()))
    }
    
    async fn subscribe_orderbook(&self, symbol: &str) -> std::result::Result<(), ConnectorError> {
        // 由于trait要求&self，但ws_handler需要&mut，这里需要使用内部可变性
        // 暂时返回未实现错误，需要重构ws_handler使用内部可变性
        Err(ConnectorError::SubscriptionError("订阅功能需要重构以支持内部可变性".to_string()))
    }
    
    async fn subscribe_trades(&self, symbol: &str) -> std::result::Result<(), ConnectorError> {
        // 由于trait要求&self，但ws_handler需要&mut，这里需要使用内部可变性
        // 暂时返回未实现错误，需要重构ws_handler使用内部可变性
        Err(ConnectorError::SubscriptionError("订阅功能需要重构以支持内部可变性".to_string()))
    }
    
    async fn subscribe_user_stream(&self) -> std::result::Result<(), ConnectorError> {
        // 启动用户数据流
        match self.start_user_data_stream().await {
            Ok(listen_key) => {
                *self.listen_key.write().await = Some(listen_key);
                Ok(())
            }
            Err(e) => Err(ConnectorError::SubscriptionError(format!("用户数据流订阅失败: {}", e)))
        }
    }
    
    // 推送式数据流接口 - 暂时使用空实现，避免panic
    fn get_market_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage> {
        // 创建一个新的通道并返回接收端
        let (_tx, rx) = mpsc::unbounded_channel();
        rx
    }
    
    fn get_user_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage> {
        // 创建一个新的通道并返回接收端
        let (_tx, rx) = mpsc::unbounded_channel();
        rx
    }
    
    // 本地缓存快照读取
    async fn get_orderbook_snapshot(&self, symbol: &str) -> Option<StandardizedOrderBook> {
        let cache = self.orderbook_cache.read().await;
        cache.get(symbol).cloned()
    }
    
    async fn get_recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Vec<StandardizedTrade> {
        let cache = self.trades_cache.read().await;
        if let Some(trades) = cache.get(symbol) {
            trades.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    // 交易相关操作 (REST API)
    async fn place_order(&self, order: &OrderRequest) -> std::result::Result<OrderResponse, ConnectorError> {
        // 转换订单请求格式
        let local_order = LocalOrderRequest {
            symbol: order.symbol.clone(),
            side: order.side.as_str().to_string(),
            order_type: order.order_type.as_str().to_string(),
            quantity: order.quantity,
            price: order.price,
            time_in_force: order.time_in_force.as_ref().map(|t| t.as_str().to_string()),
            reduce_only: order.reduce_only,
            close_position: order.close_position,
            position_side: order.position_side.as_ref().map(|p| p.as_str().to_string()),
            client_order_id: order.client_order_id.clone(),
        };
        
        // 调用REST客户端下单方法
        match self.rest_client.new_order(
            &local_order.symbol,
            &local_order.side,
            &local_order.order_type,
            local_order.quantity,
            local_order.price,
            local_order.time_in_force.as_deref(),
            local_order.reduce_only,
            local_order.close_position,
            local_order.position_side.as_deref(),
            local_order.client_order_id.as_deref(),
        ).await {
            Ok(response) => {
                // 解析响应并转换为标准格式
                let order_id = response.get("orderId")
                    .and_then(|v| v.as_u64())
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                
                let status = response.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("NEW")
                    .to_string();
                
                let filled_qty = response.get("executedQty")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                Ok(OrderResponse {
                    order_id,
                    client_order_id: order.client_order_id.clone(),
                    symbol: order.symbol.clone(),
                    status,
                    filled_quantity: filled_qty,
                    remaining_quantity: order.quantity - filled_qty,
                    average_price: response.get("avgPrice")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok()),
                    timestamp: response.get("updateTime")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as u64),
                })
            }
            Err(e) => Err(ConnectorError::TradingError(format!("下单失败: {}", e)))
        }
    }
    
    async fn cancel_order(&self, order_id: &str, symbol: &str) -> std::result::Result<bool, ConnectorError> {
        match self.rest_client.cancel_order(symbol, Some(order_id.parse().unwrap_or(0)), None).await {
            Ok(_) => Ok(true),
            Err(e) => Err(ConnectorError::TradingError(format!("取消订单失败: {}", e)))
        }
    }
    
    async fn get_order_status(&self, order_id: &str, symbol: &str) -> std::result::Result<OrderStatus, ConnectorError> {
        match self.rest_client.query_order(symbol, Some(order_id.parse().unwrap_or(0)), None).await {
            Ok(response) => {
                // 解析响应并转换为标准格式
                let status = response.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string();
                
                let filled_qty = response.get("executedQty")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                let orig_qty = response.get("origQty")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                
                Ok(OrderStatus {
                    order_id: order_id.to_string(),
                    symbol: symbol.to_string(),
                    status,
                    filled_quantity: filled_qty,
                    remaining_quantity: orig_qty - filled_qty,
                    average_price: response.get("avgPrice")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok()),
                    timestamp: response.get("updateTime")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as u64),
                })
            }
            Err(e) => Err(ConnectorError::TradingError(format!("查询订单状态失败: {}", e)))
        }
    }
    
    async fn get_account_balance(&self) -> std::result::Result<AccountBalance, ConnectorError> {
        match self.rest_client.get_account_info().await {
            Ok(response) => {
                // 解析响应并转换为标准格式
                let mut total_balance = 0.0;
                let mut available_balance = 0.0;
                
                // 查找USDT资产
                if let Some(assets) = response.get("assets").and_then(|v| v.as_array()) {
                    for asset in assets {
                        if let Some(asset_name) = asset.get("asset").and_then(|v| v.as_str()) {
                            if asset_name == "USDT" {
                                total_balance = asset.get("walletBalance")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                
                                available_balance = asset.get("availableBalance")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                break;
                            }
                        }
                    }
                }
                
                Ok(AccountBalance {
                    total: total_balance,
                    available: available_balance,
                    frozen: total_balance - available_balance,
                    balances: std::collections::HashMap::new(), // 空的详细余额映射
                })
            }
            Err(e) => Err(ConnectorError::TradingError(format!("获取账户余额失败: {}", e)))
        }
    }
    
    // 连接状态
    async fn is_connected(&self) -> bool {
        let state = self.connection_state.read().await;
        matches!(*state, ConnectionState::Connected)
    }
    
    async fn is_websocket_connected(&self) -> bool {
        let state = self.connection_state.read().await;
        matches!(*state, ConnectionState::Connected)
    }
    
    async fn get_connection_status(&self) -> ConnectionStatus {
        let state = self.connection_state.read().await;
        match &*state {
            ConnectionState::Connected => ConnectionStatus::Connected,
            ConnectionState::Connecting => ConnectionStatus::Connecting,
            ConnectionState::Reconnecting => ConnectionStatus::Reconnecting,
            ConnectionState::Disconnected => ConnectionStatus::Disconnected,
            ConnectionState::Error(_) => ConnectionStatus::Error,
        }
    }
}
