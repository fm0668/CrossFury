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

/// 价格档位数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub quantity: f64,
}

/// 深度更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthUpdate {
    pub symbol: String,
    pub first_update_id: i64,
    pub final_update_id: i64,
    pub event_time: i64,
    pub best_bid_price: f64,
    pub best_ask_price: f64,
    /// 完整的买单深度数据（按价格从高到低排序）
    pub depth_bids: Vec<PriceLevel>,
    /// 完整的卖单深度数据（按价格从低到高排序）
    pub depth_asks: Vec<PriceLevel>,
}

/// 高频数据流类型（与重构方案保持一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HighFrequencyData {
    OrderBookUpdate(StandardizedOrderBook),
    TradeUpdate(StandardizedTrade),
    TickerUpdate(Ticker),
    KlineUpdate(Kline),
}

/// 交易更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeUpdate {
    pub symbol: String,
    pub trade_id: i64,
    pub price: f64,
    pub quantity: f64,
    pub timestamp: i64,
    pub is_buyer_maker: bool,
}

/// K线更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineUpdate {
    pub symbol: String,
    pub open_time: i64,
    pub close_time: i64,
    pub interval: String,
    pub open_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub close_price: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub is_closed: bool,
}

/// Ticker更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerUpdate {
    pub symbol: String,
    pub price_change: f64,
    pub price_change_percent: f64,
    pub last_price: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub open_price: f64,
}

/// 标记价格更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkPriceUpdate {
    pub symbol: String,
    pub mark_price: f64,
    pub index_price: f64,
    pub funding_rate: f64,
    pub next_funding_time: i64,
}

/// 持仓量更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenInterestUpdate {
    pub symbol: String,
    pub open_interest: f64,
    pub timestamp: i64,
}

/// 资金费率更新数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateUpdate {
    pub symbol: String,
    pub funding_rate: f64,
    pub funding_time: i64,
}

/// 市场数据事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketDataEvent {
    DepthUpdate(DepthUpdate),
    TradeUpdate(TradeUpdate),
    KlineUpdate(KlineUpdate),
    TickerUpdate(TickerUpdate),
    MarkPriceUpdate(MarkPriceUpdate),
    OpenInterestUpdate(OpenInterestUpdate),
    FundingRateUpdate(FundingRateUpdate),
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