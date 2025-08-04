// src/connectors/common/mod.rs - 连接器通用功能

// 高级连接管理功能
pub mod advanced_connection;

// WebSocket优化重构新增模块
pub mod emergency_ping;
pub mod adaptive_timeout;
pub mod batch_subscription;
pub mod symbol_converter;
pub mod orderbook_validator;
pub mod smart_error_recovery;

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

// 重新导出新增模块的主要类型
pub use emergency_ping::{
    EmergencyPingManager as EmergencyPingManagerV2,
    EmergencyPingConfig,
    EmergencyPingState,
    EmergencyPingResult,
};

pub use adaptive_timeout::{
    AdaptiveTimeoutManager as AdaptiveTimeoutManagerV2,
    AdaptiveTimeoutConfig,
    NetworkQualityMetrics,
    TimeoutAdjustmentResult,
};

pub use batch_subscription::{
    BatchSubscriptionManager,
    BatchSubscriptionConfig,
    BatchSubscriptionResult,
    SubscriptionRequest,
};

pub use symbol_converter::{
    SymbolConverter,
    SymbolConverterConfig,
    SymbolFormat,
    SymbolInfo,
    ConversionResult,
    ConversionError,
};

pub use orderbook_validator::{
    OrderbookValidator,
    OrderbookValidatorConfig,
    ValidationResult,
    ValidationError,
    ValidationWarning,
    OrderbookStats,
};

pub use smart_error_recovery::{
    SmartErrorRecovery,
    SmartErrorRecoveryConfig,
    ErrorRecord,
    ErrorType,
    ErrorSeverity,
    ErrorContext,
    RecoveryStrategy,
    RecoveryResult,
    RecoveryStats,
};