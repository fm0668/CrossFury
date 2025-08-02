//! Binance期货WebSocket处理模块
//! 
//! 实现期货交易所的WebSocket连接、订阅管理和消息处理

use crate::connectors::binance::futures::config::{BinanceFuturesConfig, PositionSide};
use crate::connectors::binance::futures::constants::*;
use crate::types::market_data::*;
use crate::types::trading::*;
use crate::core::AppError;

// 定义Result类型别名
pub type Result<T> = std::result::Result<T, AppError>;

use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, warn, error, debug};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt, stream::{SplitSink}};

/// Binance期货WebSocket处理器
pub struct BinanceFuturesWebSocketHandler {
    /// 配置信息
    config: BinanceFuturesConfig,
    /// WebSocket连接
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    /// WebSocket写入端
    ws_sink: Option<Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>>,
    /// 数据发送通道
    data_sender: Option<mpsc::UnboundedSender<MarketDataEvent>>,
    /// 交易事件发送通道
    trade_sender: Option<mpsc::UnboundedSender<TradeEvent>>,
    /// 账户事件发送通道
    account_sender: Option<mpsc::UnboundedSender<AccountEvent>>,
    /// 订阅管理
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionInfo>>>,
    /// 连接状态
    is_connected: Arc<RwLock<bool>>,
    /// 最后心跳时间
    last_heartbeat: Arc<RwLock<DateTime<Utc>>>,
}

/// 订阅信息
#[derive(Debug, Clone)]
struct SubscriptionInfo {
    stream_name: String,
    symbol: String,
    stream_type: StreamType,
    is_active: bool,
}

/// 流类型
#[derive(Debug, Clone, PartialEq)]
enum StreamType {
    Depth,
    Trade,
    Kline(String), // 时间间隔
    Ticker,
    MarkPrice,
    FundingRate,
    OpenInterest,
    UserData,
}

/// 账户事件类型
#[derive(Debug, Clone)]
pub enum AccountEvent {
    /// 账户更新
    AccountUpdate {
        balances: Vec<FuturesBalance>,
        positions: Vec<FuturesPosition>,
        timestamp: i64,
    },
    /// 订单更新
    OrderUpdate {
        order: FuturesOrder,
        timestamp: i64,
    },
    /// 持仓更新
    PositionUpdate {
        position: FuturesPosition,
        timestamp: i64,
    },
    /// 保证金调用
    MarginCall {
        positions: Vec<FuturesPosition>,
        timestamp: i64,
    },
}

/// 期货余额信息
#[derive(Debug, Clone)]
pub struct FuturesBalance {
    pub asset: String,
    pub wallet_balance: f64,
    pub unrealized_pnl: f64,
    pub margin_balance: f64,
    pub maint_margin: f64,
    pub initial_margin: f64,
    pub position_initial_margin: f64,
    pub open_order_initial_margin: f64,
    pub cross_wallet_balance: f64,
    pub cross_unrealized_pnl: f64,
    pub available_balance: f64,
    pub max_withdraw_amount: f64,
}

/// 期货持仓信息
#[derive(Debug, Clone)]
pub struct FuturesPosition {
    pub symbol: String,
    pub position_amt: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_profit: f64,
    pub liquidation_price: f64,
    pub leverage: u8,
    pub max_notional_value: f64,
    pub margin_type: String,
    pub isolated_margin: f64,
    pub is_auto_add_margin: bool,
    pub position_side: PositionSide,
    pub notional: f64,
    pub isolated_wallet: f64,
    pub update_time: i64,
}

/// 期货订单信息
#[derive(Debug, Clone)]
pub struct FuturesOrder {
    pub symbol: String,
    pub order_id: i64,
    pub client_order_id: String,
    pub price: f64,
    pub orig_qty: f64,
    pub executed_qty: f64,
    pub cumulative_quote_qty: f64,
    pub status: String,
    pub time_in_force: String,
    pub order_type: String,
    pub side: String,
    pub stop_price: f64,
    pub iceberg_qty: f64,
    pub time: i64,
    pub update_time: i64,
    pub is_working: bool,
    pub orig_quote_order_qty: f64,
    pub position_side: PositionSide,
    pub close_position: bool,
    pub activation_price: f64,
    pub callback_rate: f64,
    pub working_type: String,
    pub price_protect: bool,
}

impl BinanceFuturesWebSocketHandler {
    /// 创建新的WebSocket处理器
    pub fn new(config: BinanceFuturesConfig) -> Self {
        Self {
            config,
            ws_stream: None,
            ws_sink: None,
            data_sender: None,
            trade_sender: None,
            account_sender: None,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            is_connected: Arc::new(RwLock::new(false)),
            last_heartbeat: Arc::new(RwLock::new(Utc::now())),
        }
    }
    
    /// 设置数据发送通道
    pub fn set_data_sender(&mut self, sender: mpsc::UnboundedSender<MarketDataEvent>) {
        self.data_sender = Some(sender);
    }
    
    /// 设置交易事件发送通道
    pub fn set_trade_sender(&mut self, sender: mpsc::UnboundedSender<TradeEvent>) {
        self.trade_sender = Some(sender);
    }
    
    /// 设置账户事件发送通道
    pub fn set_account_sender(&mut self, sender: mpsc::UnboundedSender<AccountEvent>) {
        self.account_sender = Some(sender);
    }
    
    /// 连接WebSocket
    pub async fn connect(&mut self) -> Result<()> {
        let ws_url = if self.config.testnet {
            BINANCE_FUTURES_TESTNET_WS_URL
        } else {
            BINANCE_FUTURES_WS_URL
        };
        
        info!("正在连接Binance期货WebSocket: {}", ws_url);
        
        let (ws_stream, _) = connect_async(ws_url).await
            .map_err(|e| AppError::WebSocketError(format!("连接失败: {}", e)))?;
        
        self.ws_stream = Some(ws_stream);
        *self.is_connected.write().await = true;
        *self.last_heartbeat.write().await = Utc::now();
        
        info!("Binance期货WebSocket连接成功");
        Ok(())
    }
    
    /// 订阅深度数据
    pub async fn subscribe_depth(&mut self, symbol: &str, levels: Option<u8>) -> Result<()> {
        let levels_str = levels.unwrap_or(20).to_string();
        let stream_name = format!("{}@depth{}@100ms", symbol.to_lowercase(), levels_str);
        
        self.subscribe_stream(&stream_name, symbol, StreamType::Depth).await
    }
    
    /// 订阅交易数据
    pub async fn subscribe_trades(&mut self, symbol: &str) -> Result<()> {
        let stream_name = format!("{}@aggTrade", symbol.to_lowercase());
        
        self.subscribe_stream(&stream_name, symbol, StreamType::Trade).await
    }
    
    /// 订阅K线数据
    pub async fn subscribe_klines(&mut self, symbol: &str, interval: &str) -> Result<()> {
        let stream_name = format!("{}@kline_{}", symbol.to_lowercase(), interval);
        
        self.subscribe_stream(&stream_name, symbol, StreamType::Kline(interval.to_string())).await
    }
    
    /// 订阅24小时价格统计
    pub async fn subscribe_ticker(&mut self, symbol: &str) -> Result<()> {
        let stream_name = format!("{}@ticker", symbol.to_lowercase());
        
        self.subscribe_stream(&stream_name, symbol, StreamType::Ticker).await
    }
    
    /// 订阅标记价格
    pub async fn subscribe_mark_price(&mut self, symbol: &str) -> Result<()> {
        let stream_name = format!("{}@markPrice", symbol.to_lowercase());
        
        self.subscribe_stream(&stream_name, symbol, StreamType::MarkPrice).await
    }
    
    /// 订阅资金费率
    pub async fn subscribe_funding_rate(&mut self) -> Result<()> {
        let stream_name = "!markPrice@arr".to_string();
        
        self.subscribe_stream(&stream_name, "ALL", StreamType::FundingRate).await
    }
    
    /// 订阅持仓量
    pub async fn subscribe_open_interest(&mut self, symbol: &str) -> Result<()> {
        let stream_name = format!("{}@openInterest", symbol.to_lowercase());
        
        self.subscribe_stream(&stream_name, symbol, StreamType::OpenInterest).await
    }
    
    /// 通用订阅方法
    async fn subscribe_stream(&mut self, stream_name: &str, symbol: &str, stream_type: StreamType) -> Result<()> {
        if self.ws_sink.is_none() {
            return Err(AppError::WebSocketError("WebSocket未连接".to_string()));
        }
        
        let subscribe_msg = json!({
            "method": "SUBSCRIBE",
            "params": [stream_name],
            "id": chrono::Utc::now().timestamp_millis()
        });
        
        if let Some(ref ws_sink) = self.ws_sink {
            let mut sink = ws_sink.lock().await;
            sink.send(Message::Text(subscribe_msg.to_string())).await
                .map_err(|e| AppError::WebSocketError(format!("发送订阅消息失败: {}", e)))?;
        }
        
        // 记录订阅信息
        let subscription_info = SubscriptionInfo {
            stream_name: stream_name.to_string(),
            symbol: symbol.to_string(),
            stream_type,
            is_active: true,
        };
        
        self.subscriptions.write().await.insert(stream_name.to_string(), subscription_info);
        
        info!("已订阅期货流: {} ({})", stream_name, symbol);
        Ok(())
    }
    
    /// 取消订阅
    pub async fn unsubscribe(&mut self, stream_name: &str) -> Result<()> {
        if self.ws_sink.is_none() {
            return Err(AppError::WebSocketError("WebSocket未连接".to_string()));
        }
        
        let unsubscribe_msg = json!({
            "method": "UNSUBSCRIBE",
            "params": [stream_name],
            "id": chrono::Utc::now().timestamp_millis()
        });
        
        if let Some(ref ws_sink) = self.ws_sink {
            let mut sink = ws_sink.lock().await;
            sink.send(Message::Text(unsubscribe_msg.to_string())).await
                .map_err(|e| AppError::WebSocketError(format!("发送取消订阅消息失败: {}", e)))?;
        }
        
        // 移除订阅信息
        self.subscriptions.write().await.remove(stream_name);
        
        info!("已取消订阅期货流: {}", stream_name);
        Ok(())
    }
    
    /// 检查连接状态
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }
    
    /// 断开连接
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(ws_sink) = self.ws_sink.take() {
            let mut sink = ws_sink.lock().await;
            let _ = sink.close().await;
        }
        
        self.ws_stream = None;
        
        *self.is_connected.write().await = false;
        self.subscriptions.write().await.clear();
        
        info!("Binance期货WebSocket已断开连接");
        Ok(())
    }
    
    /// 获取活跃订阅列表
    pub async fn get_active_subscriptions(&self) -> Vec<String> {
        self.subscriptions.read().await
            .values()
            .filter(|sub| sub.is_active)
            .map(|sub| sub.stream_name.clone())
            .collect()
    }
    
    /// 启动消息处理循环
    pub async fn start_message_loop(&mut self) -> Result<()> {
        if self.ws_stream.is_none() {
            return Err(AppError::WebSocketError("WebSocket未连接".to_string()));
        }
        
        let ws_stream = self.ws_stream.take().unwrap();
        let data_sender = self.data_sender.clone();
        let trade_sender = self.trade_sender.clone();
        let account_sender = self.account_sender.clone();
        let subscriptions = self.subscriptions.clone();
        let is_connected = self.is_connected.clone();
        
        let (ws_sink, mut ws_stream) = ws_stream.split();
        let ws_sink = Arc::new(Mutex::new(ws_sink));
        
        // 将写入端保存到结构体中，以便订阅操作使用
        self.ws_sink = Some(ws_sink.clone());
        
        tokio::spawn(async move {
            use futures_util::StreamExt;
            
            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = Self::process_message(
                            &text,
                            &data_sender,
                            &trade_sender,
                            &account_sender,
                            &subscriptions,
                        ).await {
                            error!("处理WebSocket消息失败: {:?}", e);
                        }
                    }
                    Ok(Message::Ping(payload)) => {
                        use futures_util::SinkExt;
                        let mut sink = ws_sink.lock().await;
                        if let Err(e) = sink.send(Message::Pong(payload)).await {
                            error!("发送Pong消息失败: {:?}", e);
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket连接已关闭");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket错误: {:?}", e);
                        break;
                    }
                    _ => {}
                }
            }
            
            *is_connected.write().await = false;
            info!("WebSocket消息处理循环已退出");
        });
        
        Ok(())
    }
    
    /// 处理WebSocket消息
    async fn process_message(
        text: &str,
        data_sender: &Option<mpsc::UnboundedSender<MarketDataEvent>>,
        _trade_sender: &Option<mpsc::UnboundedSender<TradeEvent>>,
        _account_sender: &Option<mpsc::UnboundedSender<AccountEvent>>,
        _subscriptions: &Arc<RwLock<HashMap<String, SubscriptionInfo>>>,
    ) -> Result<()> {
        debug!("收到WebSocket消息: {}", text);
        
        let msg: Value = serde_json::from_str(text)
            .map_err(|e| AppError::ParseError(format!("解析JSON失败: {}", e)))?;
        
        // 检查是否是订阅确认消息
        if msg.get("result").is_some() {
            debug!("收到订阅确认: {:?}", msg);
            return Ok(());
        }
        
        // 处理流数据
        if let Some(stream) = msg.get("stream").and_then(|s| s.as_str()) {
            debug!("处理流数据: {}", stream);
            if let Some(data) = msg.get("data") {
                if stream.contains("@depth") {
                    debug!("处理深度数据: {:?}", data);
                    Self::process_depth_data(stream, data, data_sender).await?;
                } else if stream.contains("@aggTrade") {
                    // TODO: 处理交易数据
                } else if stream.contains("@kline") {
                    // TODO: 处理K线数据
                } else if stream.contains("@ticker") {
                    // TODO: 处理24小时价格统计
                } else if stream.contains("@markPrice") {
                    // TODO: 处理标记价格
                }
            }
        } else {
            // 可能是直接的深度数据格式
            debug!("检查是否为直接深度数据格式: {:?}", msg);
            if msg.get("e").and_then(|e| e.as_str()) == Some("depthUpdate") {
                debug!("处理直接深度更新数据");
                Self::process_direct_depth_data(&msg, data_sender).await?;
            }
        }
        
        Ok(())
    }
    
    /// 处理深度数据
    async fn process_depth_data(
        stream: &str,
        data: &Value,
        data_sender: &Option<mpsc::UnboundedSender<MarketDataEvent>>,
    ) -> Result<()> {
        if let Some(sender) = data_sender {
            // 解析交易对名称
            let symbol = stream.split('@').next().unwrap_or("").to_uppercase();
            
            // 解析买单和卖单
            let empty_vec = vec![];
            let bids = data.get("b").and_then(|b| b.as_array()).unwrap_or(&empty_vec);
            let asks = data.get("a").and_then(|a| a.as_array()).unwrap_or(&empty_vec);
            
            let mut depth_bids = Vec::new();
            let mut depth_asks = Vec::new();
            
            // 解析买单
            for bid in bids {
                if let Some(bid_array) = bid.as_array() {
                    if bid_array.len() >= 2 {
                        let price = bid_array[0].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        let quantity = bid_array[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        depth_bids.push(PriceLevel { price, quantity });
                    }
                }
            }
            
            // 解析卖单
            for ask in asks {
                if let Some(ask_array) = ask.as_array() {
                    if ask_array.len() >= 2 {
                        let price = ask_array[0].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        let quantity = ask_array[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        depth_asks.push(PriceLevel { price, quantity });
                    }
                }
            }
            
            // 获取最佳买卖价
            let best_bid_price = depth_bids.first().map(|b| b.price).unwrap_or(0.0);
            let best_ask_price = depth_asks.first().map(|a| a.price).unwrap_or(0.0);
            
            let depth_update = DepthUpdate {
                symbol,
                first_update_id: 0, // TODO: 从实际数据中获取
                final_update_id: 0, // TODO: 从实际数据中获取
                event_time: Utc::now().timestamp_millis(),
                best_bid_price,
                best_ask_price,
                depth_bids,
                depth_asks,
            };
            
            let market_event = MarketDataEvent::DepthUpdate(depth_update);
            
            if let Err(e) = sender.send(market_event) {
                error!("发送深度数据失败: {:?}", e);
            }
        }
        
        Ok(())
    }
    
    /// 处理直接深度数据格式
    async fn process_direct_depth_data(
        data: &Value,
        data_sender: &Option<mpsc::UnboundedSender<MarketDataEvent>>,
    ) -> Result<()> {
        if let Some(sender) = data_sender {
            // 解析交易对名称
            let symbol = data.get("s").and_then(|s| s.as_str()).unwrap_or("").to_string();
            
            // 解析买单和卖单
            let empty_vec = vec![];
            let bids = data.get("b").and_then(|b| b.as_array()).unwrap_or(&empty_vec);
            let asks = data.get("a").and_then(|a| a.as_array()).unwrap_or(&empty_vec);
            
            let mut depth_bids = Vec::new();
            let mut depth_asks = Vec::new();
            
            // 解析买单
            for bid in bids {
                if let Some(bid_array) = bid.as_array() {
                    if bid_array.len() >= 2 {
                        let price = bid_array[0].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        let quantity = bid_array[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        depth_bids.push(PriceLevel { price, quantity });
                    }
                }
            }
            
            // 解析卖单
            for ask in asks {
                if let Some(ask_array) = ask.as_array() {
                    if ask_array.len() >= 2 {
                        let price = ask_array[0].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        let quantity = ask_array[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                        depth_asks.push(PriceLevel { price, quantity });
                    }
                }
            }
            
            // 获取最佳买卖价
            let best_bid_price = depth_bids.first().map(|b| b.price).unwrap_or(0.0);
            let best_ask_price = depth_asks.first().map(|a| a.price).unwrap_or(0.0);
            
            let first_update_id = data.get("U").and_then(|u| u.as_i64()).unwrap_or(0);
            let final_update_id = data.get("u").and_then(|u| u.as_i64()).unwrap_or(0);
            let event_time = data.get("E").and_then(|e| e.as_i64()).unwrap_or(Utc::now().timestamp_millis());
            
            // 在移动之前保存长度信息
            let bids_len = depth_bids.len();
            let asks_len = depth_asks.len();
            
            let depth_update = DepthUpdate {
                symbol,
                first_update_id,
                final_update_id,
                event_time,
                best_bid_price,
                best_ask_price,
                depth_bids,
                depth_asks,
            };
            
            let market_event = MarketDataEvent::DepthUpdate(depth_update);
            
            if let Err(e) = sender.send(market_event) {
                error!("发送深度数据失败: {:?}", e);
            } else {
                debug!("成功发送深度数据: {} bids, {} asks", bids_len, asks_len);
            }
        }
        
        Ok(())
    }
}