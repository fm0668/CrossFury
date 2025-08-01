// src/types/market_data.rs - 市场数据类型定义

use serde::{Deserialize, Serialize};
use super::exchange::ExchangeType;

/// 标准化消息类型（与核心Trait定义保持一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StandardizedMessage {
    OrderBookUpdate(StandardizedOrderBook),
    TradeUpdate(StandardizedTrade),
    TickerUpdate(Ticker),
    KlineUpdate(Kline),
    UserDataUpdate(UserData),
}

/// 标准化订单簿（重用现有类型，确保兼容性）
pub type StandardizedOrderBook = crate::exchange_types::StandardOrderBook;

/// 标准化交易数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardizedTrade {
    pub symbol: String,
    pub exchange: ExchangeType,
    pub price: f64,
    pub quantity: f64,
    pub side: TradeSide,
    pub timestamp: i64,
    pub trade_id: String,
}

/// 交易方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Ticker数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub exchange: ExchangeType,
    pub last_price: f64,
    pub bid_price: f64,
    pub ask_price: f64,
    pub volume_24h: f64,
    pub change_24h: f64,
    pub timestamp: i64,
}

/// K线数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    pub symbol: String,
    pub exchange: ExchangeType,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: i64,
    pub interval: String,
}

/// 用户数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserData {
    OrderUpdate(OrderUpdate),
    PositionUpdate(PositionUpdate),
    BalanceUpdate(BalanceUpdate),
}

/// 订单更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub order_id: String,
    pub symbol: String,
    pub exchange: ExchangeType,
    pub status: String,
    pub filled_quantity: f64,
    pub remaining_quantity: f64,
    pub timestamp: i64,
}

/// 仓位更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    pub symbol: String,
    pub exchange: ExchangeType,
    pub size: f64,
    pub entry_price: f64,
    pub unrealized_pnl: f64,
    pub timestamp: i64,
}

/// 余额更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceUpdate {
    pub asset: String,
    pub exchange: ExchangeType,
    pub free: f64,
    pub locked: f64,
    pub timestamp: i64,
}

/// 高频数据流类型（与重构方案保持一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HighFrequencyData {
    OrderBookUpdate(StandardizedOrderBook),
    TradeUpdate(StandardizedTrade),
    TickerUpdate(Ticker),
    KlineUpdate(Kline),
}

/// 系统事件类型（与重构方案保持一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    // 连接状态事件
    ConnectorConnected { exchange: String },
    ConnectorDisconnected { exchange: String, reason: String },
    ConnectorError { exchange: String, error: String },
    
    // 订阅状态事件
    SubscriptionAdded { exchange: String, symbol: String, data_type: String },
    SubscriptionRemoved { exchange: String, symbol: String, data_type: String },
    
    // 系统事件
    SystemStarted,
    SystemStopped,
    HealthCheckFailed { exchange: String, error: String },
}