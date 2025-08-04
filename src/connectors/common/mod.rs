// src/connectors/common/mod.rs - 连接器通用功能

// 高级连接管理功能
pub mod advanced_connection;

// 预留通用功能模块
// pub mod health_checker;
// pub mod message_parser;
// pub mod rate_limiter;
// pub mod reconnect_handler;

// 重新导出主要类型
pub use advanced_connection::{
    EmergencyPingManager,
    ConnectionQualityMonitor,
    AdaptiveTimeoutManager,
};