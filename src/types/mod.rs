// src/types/mod.rs - 严格遵循核心Trait定义的类型系统

pub mod exchange;
pub mod market_data;
pub mod orders;
pub mod errors;
pub mod config;
pub mod account;
pub mod common;
pub mod events;

// 重新导出现有类型，确保兼容性
pub use crate::exchange_types::*;

// 核心类型定义（与CrossFury_核心Trait定义.md保持一致）
pub use exchange::{ExchangeType, MarketType};
pub use market_data::{StandardizedMessage, StandardizedOrderBook, StandardizedTrade};
pub use orders::{OrderRequest, OrderResponse, OrderStatus};
pub use errors::ConnectorError;
pub use config::ConnectionStatus;

// 账户相关类型
pub use account::{AccountBalance, Position, PositionSide, CurrencyBalance};

// 通用类型
pub use common::{ExchangeType as CommonExchangeType, MarketType as CommonMarketType, ConnectionStatus as CommonConnectionStatus, DataType, UpdateSpeed};

// 事件和高频数据类型
pub use events::{SystemEvent, HighFrequencyData, DataFlowStats};

// 数据流相关类型（保持向后兼容）
pub use market_data::{HighFrequencyData as MarketDataHighFrequencyData, SystemEvent as MarketDataSystemEvent};