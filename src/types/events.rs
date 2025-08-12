//! 事件和高频数据类型定义

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use super::common::{ExchangeType, MarketType};
use super::market_data::{StandardizedOrderBook, StandardizedTrade};
use super::trading::{OrderSide, OrderStatus, OrderType, PositionSide};

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
    /// 服务暂停事件
    ServicePaused {
        exchange: ExchangeType,
        market_type: MarketType,
        reason: String,
        timestamp: SystemTime,
    },
    /// 服务降级事件
    ServiceDegraded {
        exchange: ExchangeType,
        market_type: MarketType,
        reason: String,
        timestamp: SystemTime,
    },
    /// 连接器初始化事件
    ConnectorInitialized {
        connector_id: String,
        exchange: ExchangeType,
        market_type: MarketType,
    },
    /// 连接器连接事件
    ConnectorConnected {
        connector_id: String,
        exchange: ExchangeType,
        market_type: MarketType,
    },
    /// 连接器断开事件
    ConnectorDisconnected {
        connector_id: String,
        exchange: ExchangeType,
        market_type: MarketType,
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

/// 余额变化原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BalanceChangeReason {
    /// 交易
    Trade,
    /// 资金费率
    FundingFee,
    /// 转账
    Transfer,
    /// 其他原因
    Other(String),
}

/// 标准化订单更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardizedOrderUpdate {
    /// 订单ID
    pub order_id: String,
    /// 客户端订单ID
    pub client_order_id: Option<String>,
    /// 交易对
    pub symbol: String,
    /// 订单方向
    pub side: OrderSide,
    /// 订单类型
    pub order_type: OrderType,
    /// 订单状态
    pub status: OrderStatus,
    /// 订单数量
    pub quantity: f64,
    /// 订单价格
    pub price: Option<f64>,
    /// 已成交数量
    pub filled_quantity: f64,
    /// 剩余数量
    pub remaining_quantity: f64,
    /// 平均成交价格
    pub average_price: Option<f64>,
    /// 持仓方向（期货）
    pub position_side: Option<PositionSide>,
    /// 只减仓（期货）
    pub reduce_only: Option<bool>,
    /// 创建时间
    pub created_time: SystemTime,
    /// 更新时间
    pub updated_time: SystemTime,
}

/// 标准化余额更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardizedBalanceUpdate {
    /// 资产
    pub asset: String,
    /// 总余额
    pub total_balance: f64,
    /// 可用余额
    pub available_balance: f64,
    /// 冻结余额
    pub frozen_balance: f64,
    /// 余额变化
    pub balance_change: f64,
    /// 变化原因
    pub change_reason: BalanceChangeReason,
}

/// 标准化持仓更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardizedPositionUpdate {
    /// 交易对
    pub symbol: String,
    /// 持仓方向
    pub position_side: PositionSide,
    /// 持仓数量
    pub position_amount: f64,
    /// 开仓价格
    pub entry_price: f64,
    /// 标记价格
    pub mark_price: f64,
    /// 未实现盈亏
    pub unrealized_pnl: f64,
    /// 已实现盈亏
    pub realized_pnl: f64,
    /// 保证金
    pub margin: f64,
    /// 杠杆倍数
    pub leverage: f64,
    /// 盈亏百分比
    pub pnl_percentage: f64,
}

/// 标准化交易执行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardizedTradeExecution {
    /// 成交ID
    pub trade_id: String,
    /// 订单ID
    pub order_id: String,
    /// 交易对
    pub symbol: String,
    /// 交易方向
    pub side: OrderSide,
    /// 成交数量
    pub quantity: f64,
    /// 成交价格
    pub price: f64,
    /// 手续费
    pub commission: f64,
    /// 手续费资产
    pub commission_asset: String,
    /// 是否为挂单方
    pub is_maker: bool,
    /// 执行时间
    pub execution_time: SystemTime,
}

/// 用户数据事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserDataEvent {
    /// 订单更新
    OrderUpdate {
        exchange: ExchangeType,
        market_type: MarketType,
        order: StandardizedOrderUpdate,
        timestamp: SystemTime,
    },
    /// 余额更新
    BalanceUpdate {
        exchange: ExchangeType,
        market_type: MarketType,
        balance: StandardizedBalanceUpdate,
        timestamp: SystemTime,
    },
    /// 持仓更新
    PositionUpdate {
        exchange: ExchangeType,
        market_type: MarketType,
        position: StandardizedPositionUpdate,
        timestamp: SystemTime,
    },
    /// 交易执行
    TradeExecution {
        exchange: ExchangeType,
        market_type: MarketType,
        execution: StandardizedTradeExecution,
        timestamp: SystemTime,
    },
}