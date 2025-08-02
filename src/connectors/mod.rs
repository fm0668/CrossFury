// src/connectors/mod.rs - 连接器模块入口

pub mod traits;
pub mod common;

// 重新导出核心trait
pub use traits::*;

// 连接器实现模块
pub mod lbank;
pub mod binance;
// pub mod xtcom;
// pub mod tapbit;
// pub mod hbit;
// pub mod batonex;
// pub mod coincatch;
// pub mod binance;
// pub mod bybit;
// pub mod okx;

// 预留管理器和工厂
// pub mod manager;
// pub mod factory;
// pub mod data_flow_manager;