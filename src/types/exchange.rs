// src/types/exchange.rs - 交易所和市场类型定义

use serde::{Deserialize, Serialize};
use std::fmt;

/// 交易所类型枚举（与核心Trait定义保持一致）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExchangeType {
    // 现货交易所
    Phemex,
    LBank,
    XtCom,
    TapBit,
    Hbit,
    Batonex,
    CoinCatch,
    Binance,
    // 期货交易所
    BinanceFutures,
    BybitFutures,
    OkxFutures,
}

impl fmt::Display for ExchangeType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExchangeType::Phemex => write!(f, "PHEMEX"),
            ExchangeType::LBank => write!(f, "LBANK"),
            ExchangeType::XtCom => write!(f, "XTCOM"),
            ExchangeType::TapBit => write!(f, "TAPBIT"),
            ExchangeType::Hbit => write!(f, "HBIT"),
            ExchangeType::Batonex => write!(f, "BATONEX"),
            ExchangeType::CoinCatch => write!(f, "COINCATCH"),
            ExchangeType::Binance => write!(f, "BINANCE"),
            ExchangeType::BinanceFutures => write!(f, "BINANCE_FUTURES"),
            ExchangeType::BybitFutures => write!(f, "BYBIT_FUTURES"),
            ExchangeType::OkxFutures => write!(f, "OKX_FUTURES"),
        }
    }
}

/// 市场类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketType {
    Spot,    // 现货市场
    Futures, // 期货市场
}

impl fmt::Display for MarketType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MarketType::Spot => write!(f, "SPOT"),
            MarketType::Futures => write!(f, "FUTURES"),
        }
    }
}

// 与现有Exchange枚举的转换函数
impl From<crate::exchange_types::Exchange> for ExchangeType {
    fn from(exchange: crate::exchange_types::Exchange) -> Self {
        match exchange {
            crate::exchange_types::Exchange::Phemex => ExchangeType::Phemex,
            crate::exchange_types::Exchange::LBank => ExchangeType::LBank,
            crate::exchange_types::Exchange::XtCom => ExchangeType::XtCom,
            crate::exchange_types::Exchange::TapBit => ExchangeType::TapBit,
            crate::exchange_types::Exchange::Hbit => ExchangeType::Hbit,
            crate::exchange_types::Exchange::Batonex => ExchangeType::Batonex,
            crate::exchange_types::Exchange::CoinCatch => ExchangeType::CoinCatch,
            crate::exchange_types::Exchange::Binance => ExchangeType::Binance,
            crate::exchange_types::Exchange::BinanceFutures => ExchangeType::BinanceFutures,
            crate::exchange_types::Exchange::BybitFutures => ExchangeType::BybitFutures,
            crate::exchange_types::Exchange::OkxFutures => ExchangeType::OkxFutures,
        }
    }
}

impl From<ExchangeType> for crate::exchange_types::Exchange {
    fn from(exchange_type: ExchangeType) -> Self {
        match exchange_type {
            ExchangeType::Phemex => crate::exchange_types::Exchange::Phemex,
            ExchangeType::LBank => crate::exchange_types::Exchange::LBank,
            ExchangeType::XtCom => crate::exchange_types::Exchange::XtCom,
            ExchangeType::TapBit => crate::exchange_types::Exchange::TapBit,
            ExchangeType::Hbit => crate::exchange_types::Exchange::Hbit,
            ExchangeType::Batonex => crate::exchange_types::Exchange::Batonex,
            ExchangeType::CoinCatch => crate::exchange_types::Exchange::CoinCatch,
            ExchangeType::Binance => crate::exchange_types::Exchange::Binance,
            ExchangeType::BinanceFutures => crate::exchange_types::Exchange::BinanceFutures,
            ExchangeType::BybitFutures => crate::exchange_types::Exchange::BybitFutures,
            ExchangeType::OkxFutures => crate::exchange_types::Exchange::OkxFutures,
        }
    }
}

// 根据交易所类型判断市场类型
impl ExchangeType {
    pub fn get_market_type(&self) -> MarketType {
        match self {
            ExchangeType::Phemex | ExchangeType::LBank | ExchangeType::XtCom 
            | ExchangeType::TapBit | ExchangeType::Hbit | ExchangeType::Batonex 
            | ExchangeType::CoinCatch | ExchangeType::Binance => MarketType::Spot,
            ExchangeType::BinanceFutures | ExchangeType::BybitFutures 
            | ExchangeType::OkxFutures => MarketType::Futures,
        }
    }
}