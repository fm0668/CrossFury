//! 事件和高频数据类型定义

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use super::common::{ExchangeType, MarketType};
use super::market_data::{StandardizedOrderBook, StandardizedTrade};

/// 系统事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// 连接事件
    Connection {
        exchange: ExchangeType,
        market_type: MarketType,
        connected: bool,
        timestamp: SystemTime,
    },
    /// 订阅事件
    Subscription {
        exchange: ExchangeType,
        market_type: MarketType,
        symbol: String,
        subscribed: bool,
        timestamp: SystemTime,
    },
    /// 错误事件
    Error {
        exchange: ExchangeType,
        market_type: MarketType,
        error: String,
        timestamp: SystemTime,
    },
    /// 数据质量事件
    DataQuality {
        exchange: ExchangeType,
        market_type: MarketType,
        symbol: String,
        latency_ms: u64,
        timestamp: SystemTime,
    },
    /// 套利机会事件
    ArbitrageOpportunity {
        symbol: String,
        buy_exchange: ExchangeType,
        sell_exchange: ExchangeType,
        profit_percentage: f64,
        timestamp: SystemTime,
    },
}

/// 高频数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HighFrequencyData {
    /// 订单簿更新
    OrderBookUpdate {
        exchange: ExchangeType,
        market_type: MarketType,
        orderbook: StandardizedOrderBook,
        timestamp: SystemTime,
    },
    /// 交易更新
    TradeUpdate {
        exchange: ExchangeType,
        market_type: MarketType,
        trade: StandardizedTrade,
        timestamp: SystemTime,
    },
    /// 价格变动
    PriceChange {
        exchange: ExchangeType,
        market_type: MarketType,
        symbol: String,
        old_price: f64,
        new_price: f64,
        change_percentage: f64,
        timestamp: SystemTime,
    },
    /// 深度变化
    DepthChange {
        exchange: ExchangeType,
        market_type: MarketType,
        symbol: String,
        bid_depth: f64,
        ask_depth: f64,
        timestamp: SystemTime,
    },
}

/// 数据流统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowStats {
    /// 消息总数
    pub total_messages: u64,
    /// 每秒消息数
    pub messages_per_second: f64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// 最大延迟（毫秒）
    pub max_latency_ms: u64,
    /// 错误计数
    pub error_count: u64,
    /// 连接时长（秒）
    pub uptime_seconds: u64,
}