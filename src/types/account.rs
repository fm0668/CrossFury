//! 账户相关类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 账户余额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    /// 总余额
    pub total: f64,
    /// 可用余额
    pub available: f64,
    /// 冻结余额
    pub frozen: f64,
    /// 币种余额详情
    pub balances: HashMap<String, CurrencyBalance>,
}

/// 单个币种余额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyBalance {
    /// 币种名称
    pub currency: String,
    /// 总余额
    pub total: f64,
    /// 可用余额
    pub available: f64,
    /// 冻结余额
    pub frozen: f64,
}

/// 持仓信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// 交易对
    pub symbol: String,
    /// 持仓方向（多头/空头）
    pub side: PositionSide,
    /// 持仓数量
    pub size: f64,
    /// 平均开仓价格
    pub avg_price: f64,
    /// 当前价格
    pub mark_price: f64,
    /// 未实现盈亏
    pub unrealized_pnl: f64,
    /// 已实现盈亏
    pub realized_pnl: f64,
    /// 保证金
    pub margin: f64,
    /// 杠杆倍数
    pub leverage: f64,
}

/// 持仓方向
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PositionSide {
    /// 多头
    Long,
    /// 空头
    Short,
}