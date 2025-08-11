//! 连接器管理器模块
//! 
//! 提供统一的连接器管理功能，包括注册、生命周期管理、监控等

pub mod modern_manager;
pub mod config_manager;

pub use modern_manager::*;
pub use config_manager::*;