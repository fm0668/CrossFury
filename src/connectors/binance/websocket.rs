//! Binance WebSocket处理器
//! 
//! 负责WebSocket连接管理和消息处理

use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};
use log::{info, warn, error, debug};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use chrono;

use crate::types::*;
use crate::types::market_data::PriceLevel;
use super::config::BinanceConfig;
use super::spot::BinanceSpotConnector;

/// Binance WebSocket处理器
/// 
/// 管理WebSocket连接生命周期和消息处理
pub struct BinanceWebSocketHandler {
    config: BinanceConfig,
    spot_connector: Arc<BinanceSpotConnector>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    should_reconnect: Arc<RwLock<bool>>,
    app_state: Arc<crate::AppState>,
    connection_id: String,
}

impl BinanceWebSocketHandler {
    /// 创建新的WebSocket处理器
    pub async fn new(
        config: BinanceConfig,
        app_state: Arc<crate::AppState>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let spot_connector = Arc::new(
            BinanceSpotConnector::new(config.clone(), app_state.clone()).await?
        );
        
        let connection_id = format!("binance-ws-{}", chrono::Utc::now().timestamp_millis());
        
        Ok(Self {
            config,
            spot_connector,
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            should_reconnect: Arc::new(RwLock::new(true)),
            app_state,
            connection_id,
        })
    }
    
    /// 连接WebSocket
    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[Binance] {} 开始连接WebSocket...", self.connection_id);
        
        // 设置重连标志
        {
            let mut should_reconnect = self.should_reconnect.write().await;
            *should_reconnect = true;
        }
        
        // 更新连接状态
        {
            let mut status = self.connection_status.write().await;
            *status = ConnectionStatus::Connecting;
        }
        
        // 获取已订阅的符号和数据类型
        let symbols = self.spot_connector.get_subscribed_symbols().await;
        let data_types = self.spot_connector.get_subscribed_data_types().await;
        
        info!("[Binance] {} 订阅信息: symbols={:?}, types={:?}", self.connection_id, symbols, data_types);
        
        // 生成流列表
        let mut streams = Vec::new();
        for symbol in &symbols {
            let symbol_lower = symbol.to_lowercase();
            for data_type in &data_types {
                match data_type {
                    crate::types::DataType::OrderBook => {
                        streams.push(format!("{symbol_lower}@depth20@100ms"));
                        info!("[Binance] {} 添加深度流: {}@depth20@100ms", self.connection_id, symbol_lower);
                    }
                    crate::types::DataType::Trade => {
                        streams.push(format!("{symbol_lower}@trade"));
                        info!("[Binance] {} 添加交易流: {}@trade", self.connection_id, symbol_lower);
                    }
                    crate::types::DataType::Ticker24hr => {
                        streams.push(format!("{symbol_lower}@ticker"));
                        info!("[Binance] {} 添加价格流: {}@ticker", self.connection_id, symbol_lower);
                    }
                    _ => {
                        warn!("[Binance] {} 不支持的数据类型: {:?}", self.connection_id, data_type);
                    }
                }
            }
        }
        
        info!("[Binance] {} 生成的流列表: {:?}", self.connection_id, streams);
        
        let ws_url = self.spot_connector.get_combined_stream_url(&streams);
        info!("[Binance] {} 连接URL: {}", self.connection_id, ws_url);
        
        // 启动连接循环
        let spot_connector = self.spot_connector.clone();
        let connection_status = self.connection_status.clone();
        let should_reconnect = self.should_reconnect.clone();
        let app_state = self.app_state.clone();
        let connection_id = self.connection_id.clone();
        
        tokio::spawn(async move {
            Self::connection_loop(
                &ws_url,
                spot_connector,
                connection_status,
                should_reconnect,
                app_state,
                connection_id,
            ).await;
        });
        
        // 等待连接建立（减少等待时间避免阻塞）
        let mut attempts = 0;
        while attempts < 30 { // 最多等待3秒
            tokio::time::sleep(Duration::from_millis(100)).await;
            let status = *self.connection_status.read().await;
            match status {
                ConnectionStatus::Connected => {
                    info!("[Binance] WebSocket连接成功");
                    return Ok(());
                }
                ConnectionStatus::Error => {
                    warn!("[Binance] WebSocket连接失败，但连接将在后台继续尝试");
                    return Ok(()); // 不返回错误，让连接在后台继续尝试
                }
                _ => {}
            }
            attempts += 1;
        }
        
        // 连接超时，但不返回错误，让连接在后台继续尝试
        info!("[Binance] WebSocket连接正在后台建立中...");
        Ok(())
    }
    
    /// 断开WebSocket连接
    pub async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[Binance] 断开WebSocket连接...");
        
        // 设置不再重连
        {
            let mut should_reconnect = self.should_reconnect.write().await;
            *should_reconnect = false;
        }
        
        // 更新连接状态
        {
            let mut status = self.connection_status.write().await;
            *status = ConnectionStatus::Disconnected;
        }
        
        info!("[Binance] WebSocket连接已断开");
        Ok(())
    }
    
    /// 订阅市场数据
    pub async fn subscribe_market_data(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[Binance] {} 订阅市场数据: symbols={:?}, types={:?}", self.connection_id, symbols, data_types);
        
        // 记录订阅信息
        self.spot_connector.add_subscribed_symbols(symbols.clone()).await;
        self.spot_connector.add_subscribed_data_types(data_types.clone()).await;
        
        // 如果当前已连接，需要重新连接以使用新的订阅URL
        let current_status = self.get_connection_status().await;
        if current_status == ConnectionStatus::Connected {
            info!("[Binance] {} 检测到新订阅，重新连接WebSocket", self.connection_id);
            
            // 停止当前连接
            {
                let mut should_reconnect = self.should_reconnect.write().await;
                *should_reconnect = false;
            }
            
            // 等待连接断开
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            // 重新启动连接
            {
                let mut should_reconnect = self.should_reconnect.write().await;
                *should_reconnect = true;
            }
            
            self.connect().await?;
        }
        
        Ok(())
    }
    
    /// 取消订阅市场数据
    pub async fn unsubscribe_market_data(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[Binance] 取消订阅市场数据: symbols={symbols:?}, types={data_types:?}");
        
        // 生成取消订阅消息
        let unsubscription_message = self.spot_connector
            .create_subscription_message(&symbols, &data_types, false)?;
        
        info!("[Binance] 取消订阅消息: {unsubscription_message}");
        
        // 注意：实际的取消订阅消息发送需要在WebSocket连接循环中处理
        
        Ok(())
    }
    
    /// 创建WebSocket连接
    async fn connect_with_headers(ws_url: &str) -> Result<(tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tokio_tungstenite::tungstenite::handshake::client::Response), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, response) = connect_async(ws_url).await?;
        Ok((ws_stream, response))
    }

    /// WebSocket连接循环
    async fn connection_loop(
        ws_url: &str,
        spot_connector: Arc<BinanceSpotConnector>,
        connection_status: Arc<RwLock<ConnectionStatus>>,
        should_reconnect: Arc<RwLock<bool>>,
        app_state: Arc<crate::AppState>,
        connection_id: String,
    ) {
        let mut retry_count = 0;
        let max_retries = 15; // 增加最大重试次数，提高实盘环境的连接稳定性
        
        while retry_count < max_retries {
            // 检查是否应该继续重连
            {
                let should_reconnect_guard = should_reconnect.read().await;
                if !*should_reconnect_guard {
                    info!("[Binance] {connection_id} 停止重连");
                    break;
                }
            }
            
            info!("[Binance] {} 连接到 {} (尝试 {}/{})", connection_id, ws_url, retry_count + 1, max_retries);
            
            match timeout(Duration::from_secs(15), Self::connect_with_headers(ws_url)).await {
                Ok(Ok((ws_stream, _response))) => {
                    info!("[Binance] {connection_id} WebSocket连接成功");
                    retry_count = 0; // 重置重试计数
                    
                    // 更新连接状态
                    {
                        let mut status = connection_status.write().await;
                        *status = ConnectionStatus::Connected;
                    }
                    
                    let (write, mut read) = ws_stream.split();
                    let write = Arc::new(Mutex::new(write));
                    
                    // 启动心跳任务，参考xtcom的实现
                    let ping_write = write.clone();
                    let ping_connection_id = connection_id.clone();
                    let ping_should_reconnect = should_reconnect.clone();
                    let ping_task = tokio::spawn(async move {
                        let mut interval = tokio::time::interval(Duration::from_secs(10)); // 缩短心跳间隔到10秒
                        
                        // 等待连接稳定后再开始发送心跳
                        sleep(Duration::from_secs(1)).await;
                        
                        loop {
                            interval.tick().await;
                            
                            // 检查是否应该停止
                            {
                                let should_reconnect_guard = ping_should_reconnect.read().await;
                                if !*should_reconnect_guard {
                                    debug!("[Binance] {ping_connection_id} 心跳任务收到停止信号");
                                    break;
                                }
                            }
                            
                            debug!("[Binance] {ping_connection_id} 发送心跳");
                            let mut writer = ping_write.lock().await;
                            if let Err(e) = writer.send(Message::Ping(vec![])).await {
                                error!("[Binance] {ping_connection_id} 发送心跳失败: {e}");
                                break;
                            }
                        }
                        
                        debug!("[Binance] {ping_connection_id} 心跳任务结束");
                    });
                    
                    info!("[Binance] {connection_id} 开始接收数据流...");
                    
                    // 消息处理循环
                    let mut consecutive_errors = 0;
                    let mut consecutive_timeouts = 0;
                    
                    loop {
                        match timeout(Duration::from_secs(10), read.next()).await {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                consecutive_errors = 0;
                                consecutive_timeouts = 0;
                                
                                info!("[Binance] {} 收到消息: {}", connection_id, 
                                    if text.len() > 200 { format!("{}...({}字符)", &text[..200], text.len()) } else { text.clone() });
                                
                                if let Err(e) = Self::process_message(&spot_connector, &text, &app_state, &connection_id).await {
                                    error!("[Binance] {connection_id} 处理消息失败: {e}");
                                }
                            }
                            Ok(Some(Ok(Message::Ping(payload)))) => {
                                consecutive_errors = 0;
                                consecutive_timeouts = 0;
                                
                                let mut writer = write.lock().await;
                                if let Err(e) = writer.send(Message::Pong(payload)).await {
                                    error!("[Binance] {connection_id} 发送Pong失败: {e}");
                                    break;
                                }
                            }
                            Ok(Some(Ok(Message::Pong(_)))) => {
                                consecutive_errors = 0;
                                consecutive_timeouts = 0;
                                debug!("[Binance] {connection_id} 收到Pong");
                            }
                            Ok(Some(Ok(Message::Close(_)))) => {
                                info!("[Binance] {connection_id} WebSocket连接被服务器关闭");
                                break;
                            }
                            Ok(Some(Ok(Message::Binary(_)))) => {
                                consecutive_errors = 0;
                                consecutive_timeouts = 0;
                                debug!("[Binance] {connection_id} 收到二进制消息，忽略");
                            }
                            Ok(Some(Err(e))) => {
                                consecutive_errors += 1;
                                
                                // 根据错误类型进行不同处理
                                match &e {
                                    tokio_tungstenite::tungstenite::Error::ConnectionClosed => {
                                        error!("[Binance] {connection_id} 连接已关闭");
                                        break;
                                    }
                                    tokio_tungstenite::tungstenite::Error::AlreadyClosed => {
                                        error!("[Binance] {connection_id} 连接已经关闭");
                                        break;
                                    }
                                    tokio_tungstenite::tungstenite::Error::Protocol(_) => {
                                        error!("[Binance] {connection_id} 协议错误: {e}");
                                        if consecutive_errors >= 2 {
                                            error!("[Binance] {connection_id} 多次协议错误，重连");
                                            break;
                                        }
                                    }
                                    _ => {
                                        error!("[Binance] {connection_id} WebSocket错误: {e}");
                                        if consecutive_errors >= 3 {
                                            error!("[Binance] {connection_id} 连续错误过多，重连");
                                            break;
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                info!("[Binance] {connection_id} WebSocket流结束");
                                break;
                            }
                            Err(_) => {
                                consecutive_timeouts += 1;
                                warn!("[Binance] {connection_id} 读取超时 ({consecutive_timeouts})");
                                
                                // 参考xtcom的超时处理策略
                                if consecutive_timeouts == 1 {
                                    // 第一次超时时发送紧急ping
                                    warn!("[Binance] {connection_id} 发送紧急ping以保持连接");
                                    let mut writer = write.lock().await;
                                    if let Err(e) = writer.send(Message::Ping(vec![])).await {
                                        error!("[Binance] {connection_id} 发送紧急ping失败: {e}");
                                        break;
                                    }
                                } else if consecutive_timeouts >= 2 {
                                    error!("[Binance] {connection_id} 连续超时过多，重连");
                                    break;
                                }
                            }
                            _ => {
                                // 处理其他未知的消息类型
                                debug!("[Binance] {connection_id} 收到未知消息类型");
                            }
                        }
                    }
                    
                    // 停止心跳任务
                    ping_task.abort();
                    
                    // 连接断开，更新状态
                    {
                        let mut status = connection_status.write().await;
                        *status = ConnectionStatus::Disconnected;
                    }
                    
                    error!("[Binance] {connection_id} 会话结束，准备重连...");
                }
                Ok(Err(e)) => {
                    error!("[Binance] {connection_id} 连接失败: {e}");
                    retry_count += 1;
                    
                    // 更新连接状态
                    {
                        let mut status = connection_status.write().await;
                        *status = ConnectionStatus::Error;
                    }
                }
                Err(_) => {
                    error!("[Binance] {connection_id} 连接超时");
                    retry_count += 1;
                    
                    // 更新连接状态
                    {
                        let mut status = connection_status.write().await;
                        *status = ConnectionStatus::Error;
                    }
                }
            }
            
            // 指数退避重连，参考xtcom的实现
            let delay = f64::min(0.5 * 1.5f64.powi(retry_count), 30.0); // 使用更保守的重连策略
            info!("[Binance] {connection_id} {delay:.2}秒后重连... (尝试 {}/{})", retry_count + 1, max_retries);
            sleep(Duration::from_secs_f64(delay)).await;
        }
        
        error!("[Binance] {connection_id} 达到最大重试次数，停止连接");
    }
    
    /// 处理WebSocket消息
    async fn process_message(
        spot_connector: &Arc<BinanceSpotConnector>,
        text: &str,
        app_state: &Arc<crate::AppState>,
        connection_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 解析JSON消息
        let json_value: serde_json::Value = serde_json::from_str(text)?;
        
        // 检查是否是组合流格式（包含stream字段）
        if let Some(stream) = json_value.get("stream").and_then(|s| s.as_str()) {
            if stream.contains("@depth") {
                // 解析深度数据
                if let Some(data) = json_value.get("data") {
                    Self::process_depth_data(data, app_state, connection_id).await?;
                }
            }
        }
        // 检查是否是直接深度数据格式（包含lastUpdateId字段）
        else if json_value.get("lastUpdateId").is_some() && 
                json_value.get("bids").is_some() && 
                json_value.get("asks").is_some() {
            // 直接处理深度数据
            Self::process_depth_data(&json_value, app_state, connection_id).await?;
        }
        else {
            debug!("[Binance] {} 收到未知格式消息: {}", connection_id, 
                if text.len() > 100 { &text[..100] } else { text });
        }
        
        Ok(())
    }
    
    /// 处理深度数据
    async fn process_depth_data(
        data: &serde_json::Value,
        app_state: &Arc<crate::AppState>,
        connection_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 提取交易对符号
        // 对于组合流格式，符号在's'字段中
        // 对于直接流格式，我们需要从连接信息中推断或使用默认值
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .unwrap_or("BTCUSDT"); // 默认使用BTCUSDT，实际应用中应该从连接上下文获取
            
        // 提取买单和卖单数据
        // 尝试从'bids'字段获取买单数据（深度快照格式）
        let bids = data.get("bids")
            .and_then(|b| b.as_array())
            // 如果没有'bids'字段，尝试从'b'字段获取（增量更新格式）
            .or_else(|| data.get("b").and_then(|b| b.as_array()))
            .ok_or("缺少买单数据")?;
            
        // 尝试从'asks'字段获取卖单数据（深度快照格式）
        let asks = data.get("asks")
            .and_then(|a| a.as_array())
            // 如果没有'asks'字段，尝试从'a'字段获取（增量更新格式）
            .or_else(|| data.get("a").and_then(|a| a.as_array()))
            .ok_or("缺少卖单数据")?;
        
        // 解析完整的买单深度数据
        let mut depth_bids = Vec::new();
        let mut best_bid_price = 0.0;
        let mut best_bid_qty = 0.0;
        
        for bid in bids {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price = bid_array[0].as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let quantity = bid_array[1].as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    
                    // 只添加有效的价格档位（数量大于0）
                    if quantity > 0.0 {
                        depth_bids.push(PriceLevel { price, quantity });
                        
                        // 设置最佳买价（第一个有效档位）
                        if best_bid_price == 0.0 {
                            best_bid_price = price;
                            best_bid_qty = quantity;
                        }
                    }
                }
            }
        }
        
        // 解析完整的卖单深度数据
        let mut depth_asks = Vec::new();
        let mut best_ask_price = 0.0;
        let mut best_ask_qty = 0.0;
        
        for ask in asks {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price = ask_array[0].as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let quantity = ask_array[1].as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    
                    // 只添加有效的价格档位（数量大于0）
                    if quantity > 0.0 {
                        depth_asks.push(PriceLevel { price, quantity });
                        
                        // 设置最佳卖价（第一个有效档位）
                        if best_ask_price == 0.0 {
                            best_ask_price = price;
                            best_ask_qty = quantity;
                        }
                    }
                }
            }
        }
        
        // 确保买单按价格从高到低排序
        depth_bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal));
        
        // 确保卖单按价格从低到高排序
        depth_asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));
        
        // 创建深度更新数据
        let depth_update = crate::types::DepthUpdate {
            symbol: symbol.to_string(),
            first_update_id: data.get("U").and_then(|u| u.as_i64()).unwrap_or(0),
            final_update_id: data.get("u").and_then(|u| u.as_i64()).unwrap_or(0),
            event_time: data.get("E").and_then(|e| e.as_i64()).unwrap_or(chrono::Utc::now().timestamp_millis()),
            best_bid_price,
            best_ask_price,
            depth_bids,
            depth_asks,
        };
        
        // 发送到队列
        if let Some(ref sender) = app_state.depth_queue {
            if let Err(e) = sender.send(depth_update) {
                error!("[Binance] {connection_id} 发送深度数据到队列失败: {e}");
            } else {
                debug!("[Binance] {connection_id} 已发送深度数据: {symbol} 买价:{best_bid_price} 卖价:{best_ask_price}");
            }
        } else {
            warn!("[Binance] {connection_id} 深度队列未初始化，跳过发送");
        }
        
        Ok(())
    }
    
    /// 获取连接状态
    pub async fn get_connection_status(&self) -> ConnectionStatus {
        *self.connection_status.read().await
    }
}