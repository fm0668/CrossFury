//! 通用类型定义

use serde::{Deserialize, Serialize};

/// 交易所类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExchangeType {
    /// Binance
    Binance,
    /// Bybit
    Bybit,
    /// OKX
    OKX,
    /// LBank
    LBank,
    /// XTcom
    XTcom,
    /// Tapbit
    Tapbit,
    /// HBit
    HBit,
    /// Batonex
    Batonex,
    /// Coincatch
    Coincatch,
}

/// 市场类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketType {
    /// 现货市场
    Spot,
    /// 期货市场
    Futures,
    /// 期权市场
    Options,
    /// 杠杆市场
    Margin,
}

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// 已断开连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 断开连接中
    Disconnecting,
    /// 重连中
    Reconnecting,
    /// 连接错误
    Error,
}

/// 数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// 订单簿数据
    OrderBook,
    /// 交易数据
    Trade,
    /// K线数据
    Kline,
    /// 24小时统计数据
    Ticker24hr,
    /// 用户数据流
    UserData,
}

/// 更新速度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateSpeed {
    /// 慢速更新（1秒）
    Slow,
    /// 正常更新（100ms）
    Normal,
    /// 快速更新（实时）
    Fast,
}