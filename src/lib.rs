// Define modules
pub mod core;
pub mod utils;
pub mod terminal_log;  // Terminal logging module
pub mod exchange_types;  // Exchange-specific data types
pub mod cross_exchange;  // Cross-exchange arbitrage logic
pub mod token_lists;
pub mod symbol_mapper;  // Symbol mapper for cross-exchange arbitrage
pub mod config;  // Added missing config module export
pub mod error_handling;  // Added missing error_handling module export
pub mod json_parser;

// 新增重构模块
pub mod types;  // 新的类型系统
pub mod connectors;  // 新的连接器系统


// Re-export key components for easier usage
pub use core::*;

// Re-export utils functions
pub use utils::{
    parse_perpetual_symbol,
    ensure_exchange_prefix,
    analyze_exchange_token_distribution,
};

// 注意：原 network 模块的 WebSocket 处理器已移除
// 请使用重构后的 connectors 模块中的相应功能

// Re-export futures-related types
pub use exchange_types::FuturesContract;

// Re-export cross-exchange and symbol mapper functions
pub use cross_exchange::process_mapped_cross_exchange_arbitrage;
pub use symbol_mapper::process_mapped_cross_exchange_arbitrage as symbol_mapped_cross_exchange;

// Re-export token list functions
pub use token_lists::{
    TARGET_TOKENS,
    is_target_token,
    normalize_symbol,
    extract_exchange
};

// Re-export new cross-exchange functions
pub use cross_exchange::{
    get_normalized_cross_exchange_symbols,
    get_cross_exchange_symbols,
    buffer_cross_exchange_opportunity,
    process_cross_exchange_arbitrage,
    flush_cross_ex_buffer,
    build_exchange_fees,
    MIN_PROFIT_THRESHOLD,
    get_target_cross_exchange_symbols,
};

// In lib.rs, add this to your re-exports
pub use cross_exchange::process_mapped_cross_exchange_arbitrage_subset;

// 注意：原 network 模块的功能已移除
// 请使用重构后的 connectors 模块中的相应功能

// Re-export from terminal_log module
pub use terminal_log::run_clean_metrics_display;

// Re-export exchange types
pub use exchange_types::*;

// 注意：原 network 模块的功能已移除
// 请使用重构后的 connectors 模块中的相应功能