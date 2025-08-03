// src/types/orders.rs - 订单相关类型定义

use serde::{Deserialize, Serialize};
use super::exchange::ExchangeType;
use std::collections::HashMap;

/// 订单请求（与核心Trait定义保持一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub exchange: ExchangeType,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>, // 市价单时为None
    pub time_in_force: TimeInForce,
    pub client_order_id: Option<String>,
    // 期货交易专用字段
    pub reduce_only: Option<bool>,
    pub close_position: Option<bool>,
    pub position_side: Option<PositionSide>,
}

/// 订单响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub exchange: ExchangeType,
    pub status: OrderStatus,
    pub filled_quantity: f64,
    pub remaining_quantity: f64,
    pub timestamp: i64,
}

/// 订单状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatus {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub exchange: ExchangeType,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
    pub filled_quantity: f64,
    pub remaining_quantity: f64,
    pub status: OrderState,
    pub created_time: i64,
    pub updated_time: i64,
}

/// 订单方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// 订单类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
}

/// 订单状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderState {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

/// 订单有效期
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC, // Good Till Cancel
    IOC, // Immediate Or Cancel
    FOK, // Fill Or Kill
    GTD, // Good Till Date
}

/// 账户余额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub exchange: ExchangeType,
    pub balances: HashMap<String, AssetBalance>,
    pub timestamp: i64,
}

/// 资产余额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBalance {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
    pub total: f64,
}

/// 仓位信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub exchange: ExchangeType,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub margin: f64,
    pub timestamp: i64,
}

/// 仓位方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
    Both, // 双向持仓模式
}

/// 订单修改参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderModification {
    pub new_quantity: Option<f64>,
    pub new_price: Option<f64>,
}

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub exchange: ExchangeType,
    pub side: OrderSide,
    pub executed_quantity: f64,
    pub executed_price: f64,
    pub commission: f64,
    pub commission_asset: String,
    pub timestamp: i64,
}

/// 执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub execution_id: String,
    pub order_id: String,
    pub symbol: String,
    pub exchange: ExchangeType,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub commission: f64,
    pub timestamp: i64,
}