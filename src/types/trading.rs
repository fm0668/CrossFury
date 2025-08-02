//! 交易相关类型定义
//! 
//! 定义期货交易中使用的各种类型和结构体

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::types::common::*;

/// 交易事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeEvent {
    /// 订单更新事件
    OrderUpdate(OrderUpdate),
    /// 成交事件
    TradeExecution(TradeExecution),
    /// 持仓更新事件
    PositionUpdate(PositionUpdate),
    /// 账户余额更新事件
    BalanceUpdate(BalanceUpdate),
}

/// 订单更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub symbol: String,
    pub order_id: String,
    pub client_order_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub quantity: f64,
    pub price: f64,
    pub executed_quantity: f64,
    pub executed_price: f64,
    pub timestamp: u64,
    pub time_in_force: TimeInForce,
    pub reduce_only: bool,
    pub close_position: bool,
}

/// 成交执行信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecution {
    pub symbol: String,
    pub trade_id: String,
    pub order_id: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub commission: f64,
    pub commission_asset: String,
    pub timestamp: u64,
    pub is_maker: bool,
}

/// 持仓更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    pub symbol: String,
    pub position_side: PositionSide,
    pub position_amount: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub percentage: f64,
    pub timestamp: u64,
}

/// 余额更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceUpdate {
    pub asset: String,
    pub wallet_balance: f64,
    pub cross_wallet_balance: f64,
    pub balance_change: f64,
    pub timestamp: u64,
}

/// 订单方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn to_api_string(&self) -> &'static str {
        match self {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
    }
    
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BUY" => Some(OrderSide::Buy),
            "SELL" => Some(OrderSide::Sell),
            _ => None,
        }
    }
}

/// 订单类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopMarket,
    TakeProfit,
    TakeProfitMarket,
    TrailingStopMarket,
}

impl OrderType {
    pub fn to_api_string(&self) -> &'static str {
        match self {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::Stop => "STOP",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            OrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
        }
    }
    
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "MARKET" => Some(OrderType::Market),
            "LIMIT" => Some(OrderType::Limit),
            "STOP" => Some(OrderType::Stop),
            "STOP_MARKET" => Some(OrderType::StopMarket),
            "TAKE_PROFIT" => Some(OrderType::TakeProfit),
            "TAKE_PROFIT_MARKET" => Some(OrderType::TakeProfitMarket),
            "TRAILING_STOP_MARKET" => Some(OrderType::TrailingStopMarket),
            _ => None,
        }
    }
}

/// 订单状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

impl OrderStatus {
    pub fn to_api_string(&self) -> &'static str {
        match self {
            OrderStatus::New => "NEW",
            OrderStatus::PartiallyFilled => "PARTIALLY_FILLED",
            OrderStatus::Filled => "FILLED",
            OrderStatus::Canceled => "CANCELED",
            OrderStatus::Rejected => "REJECTED",
            OrderStatus::Expired => "EXPIRED",
        }
    }
    
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NEW" => Some(OrderStatus::New),
            "PARTIALLY_FILLED" => Some(OrderStatus::PartiallyFilled),
            "FILLED" => Some(OrderStatus::Filled),
            "CANCELED" => Some(OrderStatus::Canceled),
            "REJECTED" => Some(OrderStatus::Rejected),
            "EXPIRED" => Some(OrderStatus::Expired),
            _ => None,
        }
    }
}

/// 持仓方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Both,
    Long,
    Short,
}

impl PositionSide {
    pub fn to_api_string(&self) -> &'static str {
        match self {
            PositionSide::Both => "BOTH",
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
        }
    }
    
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BOTH" => Some(PositionSide::Both),
            "LONG" => Some(PositionSide::Long),
            "SHORT" => Some(PositionSide::Short),
            _ => None,
        }
    }
}

/// 订单有效期类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good Till Cancel
    GTC,
    /// Immediate Or Cancel
    IOC,
    /// Fill Or Kill
    FOK,
    /// Good Till Crossing
    GTX,
}

impl TimeInForce {
    pub fn to_api_string(&self) -> &'static str {
        match self {
            TimeInForce::GTC => "GTC",
            TimeInForce::IOC => "IOC",
            TimeInForce::FOK => "FOK",
            TimeInForce::GTX => "GTX",
        }
    }
    
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GTC" => Some(TimeInForce::GTC),
            "IOC" => Some(TimeInForce::IOC),
            "FOK" => Some(TimeInForce::FOK),
            "GTX" => Some(TimeInForce::GTX),
            _ => None,
        }
    }
}

/// 下单请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
    pub stop_price: Option<f64>,
    pub time_in_force: TimeInForce,
    pub reduce_only: bool,
    pub close_position: bool,
    pub position_side: PositionSide,
    pub client_order_id: Option<String>,
}

/// 取消订单请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderRequest {
    pub symbol: String,
    pub order_id: Option<String>,
    pub client_order_id: Option<String>,
}

/// 订单查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOrderRequest {
    pub symbol: String,
    pub order_id: Option<String>,
    pub client_order_id: Option<String>,
}

/// 订单响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub symbol: String,
    pub order_id: String,
    pub client_order_id: String,
    pub status: OrderStatus,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: f64,
    pub executed_quantity: f64,
    pub executed_price: f64,
    pub time_in_force: TimeInForce,
    pub reduce_only: bool,
    pub close_position: bool,
    pub position_side: PositionSide,
    pub timestamp: u64,
}

/// 持仓信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub position_side: PositionSide,
    pub position_amount: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub percentage: f64,
    pub isolated_wallet: f64,
    pub isolated_margin: f64,
    pub maintenance_margin: f64,
    pub initial_margin: f64,
    pub open_order_initial_margin: f64,
    pub max_notional: f64,
    pub bid_notional: f64,
    pub ask_notional: f64,
    pub timestamp: u64,
}

/// 账户余额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
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
    pub margin_available: bool,
    pub timestamp: u64,
}

/// 账户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub fee_tier: i32,
    pub can_trade: bool,
    pub can_deposit: bool,
    pub can_withdraw: bool,
    pub update_time: u64,
    pub total_initial_margin: f64,
    pub total_maint_margin: f64,
    pub total_wallet_balance: f64,
    pub total_unrealized_pnl: f64,
    pub total_margin_balance: f64,
    pub total_position_initial_margin: f64,
    pub total_open_order_initial_margin: f64,
    pub total_cross_wallet_balance: f64,
    pub total_cross_unrealized_pnl: f64,
    pub available_balance: f64,
    pub max_withdraw_amount: f64,
    pub assets: Vec<Balance>,
    pub positions: Vec<Position>,
}