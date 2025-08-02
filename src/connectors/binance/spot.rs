//! Binance现货连接器核心实现
//! 
//! 提供Binance现货市场的WebSocket连接和数据处理功能

use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, warn, error, debug};
use serde_json::Value;

use crate::types::*;
use super::config::BinanceConfig;

/// Binance现货连接器
/// 
/// 负责处理Binance现货市场的实时数据连接和处理
pub struct BinanceSpotConnector {
    config: BinanceConfig,
    app_state: Arc<crate::AppState>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    subscribed_symbols: Arc<RwLock<Vec<String>>>,
    subscribed_data_types: Arc<RwLock<Vec<DataType>>>,
}

impl BinanceSpotConnector {
    /// 创建新的Binance现货连接器实例
    pub async fn new(
        config: BinanceConfig,
        app_state: Arc<crate::AppState>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            config,
            app_state,
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            subscribed_data_types: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    /// 获取WebSocket URL
    pub fn get_websocket_url(&self) -> &'static str {
        if self.config.testnet {
            super::config::BINANCE_SPOT_TESTNET_WS_URL
        } else {
            super::config::BINANCE_SPOT_WS_URL
        }
    }
    
    /// 获取组合流WebSocket URL
    pub fn get_combined_stream_url(&self, streams: &[String]) -> String {
        let base_url = if self.config.testnet {
            "wss://testnet.binance.vision:9443"
        } else {
            "wss://stream.binance.com:9443"
        };
        
        if streams.is_empty() {
            return format!("{}/ws/btcusdt@ticker", base_url); // 默认流
        }
        
        let streams_param = streams.join("/");
        format!("{}/stream?streams={}", base_url, streams_param)
    }
    
    /// 获取REST API URL
    pub fn get_api_url(&self) -> &'static str {
        if self.config.testnet {
            super::config::BINANCE_SPOT_TESTNET_API_URL
        } else {
            super::config::BINANCE_SPOT_API_URL
        }
    }
    
    /// 处理订单簿数据
    pub async fn process_orderbook_data(&self, data: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or("Missing symbol in orderbook data")?;
            
        let bids = data.get("b")
            .and_then(|b| b.as_array())
            .ok_or("Missing bids in orderbook data")?;
            
        let asks = data.get("a")
            .and_then(|a| a.as_array())
            .ok_or("Missing asks in orderbook data")?;
        
        // 解析最佳买卖价
        let best_bid = bids.first()
            .and_then(|b| b.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|p| p.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
            
        let best_ask = asks.first()
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|p| p.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        // 解析深度数据
        let depth_bids = self.parse_depth_levels(bids);
        let depth_asks = self.parse_depth_levels(asks);
        
        // 创建订单簿更新
        let orderbook_update = crate::OrderbookUpdate {
            symbol: format!("BINANCE_{}", symbol),
            best_bid,
            best_ask,
            timestamp: chrono::Utc::now().timestamp_millis(),
            scale: 8,
            is_synthetic: false,
            leg1: None,
            leg2: None,
            depth_bids: Some(depth_bids),
            depth_asks: Some(depth_asks),
        };
        
        // 发送到订单簿队列
        if let Some(tx) = &self.app_state.orderbook_queue {
            if let Err(e) = tx.send(orderbook_update) {
                error!("[Binance] 发送订单簿更新失败: {}", e);
            } else {
                debug!("[Binance] 订单簿更新已发送: {}", symbol);
            }
        }
        
        Ok(())
    }
    
    /// 处理交易数据
    pub async fn process_trade_data(&self, data: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or("Missing symbol in trade data")?;
            
        let price = data.get("p")
            .and_then(|p| p.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("Missing or invalid price in trade data")?;
            
        let quantity = data.get("q")
            .and_then(|q| q.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or("Missing or invalid quantity in trade data")?;
            
        let timestamp = data.get("T")
            .and_then(|t| t.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            
        let is_buyer_maker = data.get("m")
            .and_then(|m| m.as_bool())
            .unwrap_or(false);
        
        info!("[Binance] 交易数据: {} - 价格: {}, 数量: {}, 时间: {}", 
              symbol, price, quantity, timestamp);
        
        // 这里可以添加交易数据的进一步处理逻辑
        // 例如发送到交易数据队列或更新统计信息
        
        Ok(())
    }
    
    /// 解析深度级别数据
    fn parse_depth_levels(&self, levels: &Vec<Value>) -> Vec<(f64, f64)> {
        levels.iter()
            .filter_map(|level| {
                if let Some(arr) = level.as_array() {
                    if arr.len() >= 2 {
                        let price = arr[0].as_str()?.parse::<f64>().ok()?;
                        let quantity = arr[1].as_str()?.parse::<f64>().ok()?;
                        Some((price, quantity))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// 生成订阅消息
    pub fn create_subscription_message(
        &self,
        symbols: &[String],
        data_types: &[DataType],
        subscribe: bool,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut streams = Vec::new();
        
        for symbol in symbols {
            let symbol_lower = symbol.to_lowercase();
            
            for data_type in data_types {
                match data_type {
                    DataType::OrderBook => {
                        // 订阅深度数据，20档，100ms更新
                        streams.push(format!("{}@depth20@100ms", symbol_lower));
                    }
                    DataType::Trade => {
                        // 订阅交易数据
                        streams.push(format!("{}@trade", symbol_lower));
                    }
                    DataType::Ticker24hr => {
                        // 订阅24小时价格变动统计
                        streams.push(format!("{}@ticker", symbol_lower));
                    }
                    _ => {
                        warn!("[Binance] 不支持的数据类型: {:?}", data_type);
                    }
                }
            }
        }
        
        if streams.is_empty() {
            return Err("没有有效的订阅流".into());
        }
        
        let method = if subscribe { "SUBSCRIBE" } else { "UNSUBSCRIBE" };
        let message = serde_json::json!({
            "method": method,
            "params": streams,
            "id": chrono::Utc::now().timestamp_millis()
        });
        
        Ok(message.to_string())
    }
    
    /// 更新连接状态
    pub async fn update_connection_status(&self, status: ConnectionStatus) {
        let mut current_status = self.connection_status.write().await;
        *current_status = status;
    }
    
    /// 获取连接状态
    pub async fn get_connection_status(&self) -> ConnectionStatus {
        *self.connection_status.read().await
    }
    
    /// 添加订阅的交易对
    pub async fn add_subscribed_symbols(&self, symbols: Vec<String>) {
        let mut subscribed = self.subscribed_symbols.write().await;
        for symbol in symbols {
            if !subscribed.contains(&symbol) {
                subscribed.push(symbol);
            }
        }
    }
    
    /// 添加订阅的数据类型
    pub async fn add_subscribed_data_types(&self, data_types: Vec<DataType>) {
        let mut subscribed = self.subscribed_data_types.write().await;
        for data_type in data_types {
            if !subscribed.contains(&data_type) {
                subscribed.push(data_type);
            }
        }
    }
    
    /// 获取已订阅的交易对
    pub async fn get_subscribed_symbols(&self) -> Vec<String> {
        self.subscribed_symbols.read().await.clone()
    }
    
    /// 获取已订阅的数据类型
    pub async fn get_subscribed_data_types(&self) -> Vec<DataType> {
        self.subscribed_data_types.read().await.clone()
    }
}