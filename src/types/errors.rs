// src/types/errors.rs - 统一错误处理系统

use serde::{Deserialize, Serialize};
use std::fmt;

/// 统一的应用错误类型 - 整合所有模块的错误
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    // 连接相关错误
    #[error("WebSocket错误: {0}")]
    WebSocketError(String),
    
    #[error("连接错误: {0}")]
    ConnectionError(String),
    
    #[error("连接失败: {0}")]
    ConnectionFailed(String),
    
    #[error("连接丢失: {0}")]
    ConnectionLost(String),
    
    #[error("初始化失败: {0}")]
    InitializationFailed(String),
    
    #[error("断开连接失败: {0}")]
    DisconnectionFailed(String),
    
    #[error("网络错误: {0}")]
    NetworkError(String),
    
    #[error("超时错误: {0}")]
    TimeoutError(String),
    
    #[error("速率限制超出: {0}")]
    RateLimitExceeded(String),
    
    // 认证相关错误
    #[error("认证失败: {0}")]
    AuthenticationFailed(String),
    
    #[error("无效凭据: {0}")]
    InvalidCredentials(String),
    
    #[error("加密错误: {0}")]
    CryptoError(String),
    
    // 交易相关错误
    #[error("交易错误: {0}")]
    TradingError(String),
    
    #[error("下单失败: {0}")]
    OrderPlacementFailed(String),
    
    #[error("撤单失败: {0}")]
    OrderCancellationFailed(String),
    
    #[error("余额不足: {0}")]
    InsufficientBalance(String),
    
    #[error("无效订单参数: {0}")]
    InvalidOrderParameters(String),
    
    // 风险管理错误
    #[error("风险管理错误: {0}")]
    RiskError(String),
    
    #[error("仓位限制超出: {0}")]
    PositionLimitExceeded(String),
    
    #[error("订单大小限制超出: {0}")]
    OrderSizeLimitExceeded(String),
    
    #[error("敞口限制超出: {0}")]
    ExposureLimitExceeded(String),
    
    // 数据相关错误
    #[error("数据解析错误: {0}")]
    DataParsingError(String),
    
    #[error("无效响应: {0}")]
    InvalidResponse(String),
    
    #[error("缺失价格数据: {0}")]
    MissingPriceData(String),
    
    #[error("JSON序列化错误: {0}")]
    SerializationError(String),
    
    #[error("解析错误: {0}")]
    ParseError(String),
    
    // 订阅相关错误
    #[error("订阅错误: {0}")]
    SubscriptionError(String),
    
    #[error("订阅失败: {0}")]
    SubscriptionFailed(String),
    
    #[error("无效交易对: {0}")]
    InvalidSymbol(String),
    
    // 系统相关错误
    #[error("交易所未找到: {0}")]
    ExchangeNotFound(String),
    
    #[error("服务不可用: {0}")]
    ServiceUnavailable(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("I/O错误: {0}")]
    IoError(String),
    
    #[error("HTTP请求错误: {0}")]
    RequestError(String),
    
    #[error("CSV错误: {0}")]
    CsvError(String),
    
    // 仓位管理错误
    #[error("仓位未找到: {0}")]
    PositionNotFound(String),
    
    #[error("无效仓位数据: {0}")]
    InvalidPositionData(String),
    
    #[error("同步错误: {0}")]
    SyncError(String),
    
    #[error("计算错误: {0}")]
    CalculationError(String),
    
    // 路由相关错误
    #[error("路由错误: {0}")]
    RoutingError(String),
    
    // 功能相关错误
    #[error("交易功能未实现")]
    TradingNotImplemented,
    
    #[error("内部错误: {0}")]
    InternalError(String),
    
    #[error("其他错误: {0}")]
    Other(String),
}

/// 应用结果类型别名
pub type Result<T> = std::result::Result<T, AppError>;

/// 连接器错误类型（保持向后兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectorError {
    // 连接相关错误
    ConnectionError(String),
    ConnectionFailed(String),
    ConnectionLost(String),
    WebSocketError(String),
    InitializationFailed(String),
    DisconnectionFailed(String),
    
    // 认证相关错误
    AuthenticationFailed(String),
    InvalidCredentials(String),
    
    // 订阅相关错误
    SubscriptionError(String),
    SubscriptionFailed(String),
    InvalidSymbol(String),
    
    // 交易相关错误
    TradingError(String),
    OrderPlacementFailed(String),
    OrderCancellationFailed(String),
    InsufficientBalance(String),
    InvalidOrderParameters(String),
    
    // 数据相关错误
    DataParsingError(String),
    InvalidResponse(String),
    
    // 网络相关错误
    NetworkError(String),
    TimeoutError(String),
    RateLimitExceeded(String),
    
    // 系统相关错误
    ExchangeNotFound(String),
    ServiceUnavailable(String),
    InternalError(String),
    
    // 功能相关错误
    TradingNotImplemented,
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnectorError::ConnectionError(msg) => write!(f, "Connection error: {msg}"),
            ConnectorError::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            ConnectorError::ConnectionLost(msg) => write!(f, "Connection lost: {msg}"),
            ConnectorError::WebSocketError(msg) => write!(f, "WebSocket error: {msg}"),
            ConnectorError::InitializationFailed(msg) => write!(f, "Initialization failed: {msg}"),
            ConnectorError::DisconnectionFailed(msg) => write!(f, "Disconnection failed: {msg}"),
            ConnectorError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {msg}"),
            ConnectorError::InvalidCredentials(msg) => write!(f, "Invalid credentials: {msg}"),
            ConnectorError::SubscriptionError(msg) => write!(f, "Subscription error: {msg}"),
            ConnectorError::SubscriptionFailed(msg) => write!(f, "Subscription failed: {msg}"),
            ConnectorError::InvalidSymbol(msg) => write!(f, "Invalid symbol: {msg}"),
            ConnectorError::TradingError(msg) => write!(f, "Trading error: {msg}"),
            ConnectorError::OrderPlacementFailed(msg) => write!(f, "Order placement failed: {msg}"),
            ConnectorError::OrderCancellationFailed(msg) => write!(f, "Order cancellation failed: {msg}"),
            ConnectorError::InsufficientBalance(msg) => write!(f, "Insufficient balance: {msg}"),
            ConnectorError::InvalidOrderParameters(msg) => write!(f, "Invalid order parameters: {msg}"),
            ConnectorError::DataParsingError(msg) => write!(f, "Data parsing error: {msg}"),
            ConnectorError::InvalidResponse(msg) => write!(f, "Invalid response: {msg}"),
            ConnectorError::NetworkError(msg) => write!(f, "Network error: {msg}"),
            ConnectorError::TimeoutError(msg) => write!(f, "Timeout error: {msg}"),
            ConnectorError::RateLimitExceeded(msg) => write!(f, "Rate limit exceeded: {msg}"),
            ConnectorError::ExchangeNotFound(msg) => write!(f, "Exchange not found: {msg}"),
            ConnectorError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {msg}"),
            ConnectorError::InternalError(msg) => write!(f, "Internal error: {msg}"),
            ConnectorError::TradingNotImplemented => write!(f, "Trading functionality not implemented"),
        }
    }
}

impl std::error::Error for ConnectorError {}

/// 执行错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionError {
    ConnectorError(ConnectorError),
    RoutingError(String),
    RiskCheckFailed(String),
    PositionError(String),
    InternalError(String),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecutionError::ConnectorError(err) => write!(f, "Connector error: {err}"),
            ExecutionError::RoutingError(msg) => write!(f, "Routing error: {msg}"),
            ExecutionError::RiskCheckFailed(msg) => write!(f, "Risk check failed: {msg}"),
            ExecutionError::PositionError(msg) => write!(f, "Position error: {msg}"),
            ExecutionError::InternalError(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// 风险管理错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskError {
    PositionLimitExceeded(String),
    OrderSizeLimitExceeded(String),
    ExposureLimitExceeded(String),
    InvalidRiskParameters(String),
    InternalError(String),
}

impl fmt::Display for RiskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RiskError::PositionLimitExceeded(msg) => write!(f, "Position limit exceeded: {msg}"),
            RiskError::OrderSizeLimitExceeded(msg) => write!(f, "Order size limit exceeded: {msg}"),
            RiskError::ExposureLimitExceeded(msg) => write!(f, "Exposure limit exceeded: {msg}"),
            RiskError::InvalidRiskParameters(msg) => write!(f, "Invalid risk parameters: {msg}"),
            RiskError::InternalError(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for RiskError {}

/// 仓位管理错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionError {
    PositionNotFound(String),
    InvalidPositionData(String),
    SyncError(String),
    CalculationError(String),
    InternalError(String),
}

impl fmt::Display for PositionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PositionError::PositionNotFound(msg) => write!(f, "Position not found: {msg}"),
            PositionError::InvalidPositionData(msg) => write!(f, "Invalid position data: {msg}"),
            PositionError::SyncError(msg) => write!(f, "Sync error: {msg}"),
            PositionError::CalculationError(msg) => write!(f, "Calculation error: {msg}"),
            PositionError::InternalError(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for PositionError {}

// 为向后兼容性提供错误类型转换
impl From<ConnectorError> for AppError {
    fn from(err: ConnectorError) -> Self {
        match err {
            ConnectorError::ConnectionError(msg) => AppError::ConnectionError(msg),
            ConnectorError::ConnectionFailed(msg) => AppError::ConnectionFailed(msg),
            ConnectorError::ConnectionLost(msg) => AppError::ConnectionLost(msg),
            ConnectorError::WebSocketError(msg) => AppError::WebSocketError(msg),
            ConnectorError::InitializationFailed(msg) => AppError::InitializationFailed(msg),
            ConnectorError::DisconnectionFailed(msg) => AppError::DisconnectionFailed(msg),
            ConnectorError::AuthenticationFailed(msg) => AppError::AuthenticationFailed(msg),
            ConnectorError::InvalidCredentials(msg) => AppError::InvalidCredentials(msg),
            ConnectorError::SubscriptionError(msg) => AppError::SubscriptionError(msg),
            ConnectorError::SubscriptionFailed(msg) => AppError::SubscriptionFailed(msg),
            ConnectorError::InvalidSymbol(msg) => AppError::InvalidSymbol(msg),
            ConnectorError::TradingError(msg) => AppError::TradingError(msg),
            ConnectorError::OrderPlacementFailed(msg) => AppError::OrderPlacementFailed(msg),
            ConnectorError::OrderCancellationFailed(msg) => AppError::OrderCancellationFailed(msg),
            ConnectorError::InsufficientBalance(msg) => AppError::InsufficientBalance(msg),
            ConnectorError::InvalidOrderParameters(msg) => AppError::InvalidOrderParameters(msg),
            ConnectorError::DataParsingError(msg) => AppError::DataParsingError(msg),
            ConnectorError::InvalidResponse(msg) => AppError::InvalidResponse(msg),
            ConnectorError::NetworkError(msg) => AppError::NetworkError(msg),
            ConnectorError::TimeoutError(msg) => AppError::TimeoutError(msg),
            ConnectorError::RateLimitExceeded(msg) => AppError::RateLimitExceeded(msg),
            ConnectorError::ExchangeNotFound(msg) => AppError::ExchangeNotFound(msg),
            ConnectorError::ServiceUnavailable(msg) => AppError::ServiceUnavailable(msg),
            ConnectorError::InternalError(msg) => AppError::InternalError(msg),
            ConnectorError::TradingNotImplemented => AppError::TradingNotImplemented,
        }
    }
}

impl From<ExecutionError> for AppError {
    fn from(err: ExecutionError) -> Self {
        match err {
            ExecutionError::ConnectorError(connector_err) => connector_err.into(),
            ExecutionError::RoutingError(msg) => AppError::RoutingError(msg),
            ExecutionError::RiskCheckFailed(msg) => AppError::RiskError(msg),
            ExecutionError::PositionError(msg) => AppError::InvalidPositionData(msg),
            ExecutionError::InternalError(msg) => AppError::InternalError(msg),
        }
    }
}

impl From<RiskError> for AppError {
    fn from(err: RiskError) -> Self {
        match err {
            RiskError::PositionLimitExceeded(msg) => AppError::PositionLimitExceeded(msg),
            RiskError::OrderSizeLimitExceeded(msg) => AppError::OrderSizeLimitExceeded(msg),
            RiskError::ExposureLimitExceeded(msg) => AppError::ExposureLimitExceeded(msg),
            RiskError::InvalidRiskParameters(msg) => AppError::RiskError(msg),
            RiskError::InternalError(msg) => AppError::InternalError(msg),
        }
    }
}

impl From<PositionError> for AppError {
    fn from(err: PositionError) -> Self {
        match err {
            PositionError::PositionNotFound(msg) => AppError::PositionNotFound(msg),
            PositionError::InvalidPositionData(msg) => AppError::InvalidPositionData(msg),
            PositionError::SyncError(msg) => AppError::SyncError(msg),
            PositionError::CalculationError(msg) => AppError::CalculationError(msg),
            PositionError::InternalError(msg) => AppError::InternalError(msg),
        }
    }
}

// 为标准错误类型提供转换
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::RequestError(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err.to_string())
    }
}

impl From<csv::Error> for AppError {
    fn from(err: csv::Error) -> Self {
        AppError::CsvError(err.to_string())
    }
}

// 为core.rs中的旧CoreAppError提供转换
impl From<crate::core::CoreAppError> for AppError {
    fn from(err: crate::core::CoreAppError) -> Self {
        match err {
            crate::core::CoreAppError::WebSocketError(msg) => AppError::WebSocketError(msg),
            crate::core::CoreAppError::RequestError(err) => AppError::RequestError(err.to_string()),
            crate::core::CoreAppError::SerializationError(err) => AppError::SerializationError(err.to_string()),
            crate::core::CoreAppError::IoError(err) => AppError::IoError(err.to_string()),
            crate::core::CoreAppError::CsvError(err) => AppError::CsvError(err.to_string()),
            crate::core::CoreAppError::ParseError(msg) => AppError::ParseError(msg),
            crate::core::CoreAppError::MissingPriceData(symbol) => AppError::MissingPriceData(symbol),
            crate::core::CoreAppError::TimeoutError => AppError::TimeoutError("操作超时".to_string()),
            crate::core::CoreAppError::ConnectionError(msg) => AppError::ConnectionError(msg),
            crate::core::CoreAppError::ConfigError(msg) => AppError::ConfigError(msg),
            crate::core::CoreAppError::CryptoError(msg) => AppError::CryptoError(msg),
            crate::core::CoreAppError::RiskError(msg) => AppError::RiskError(msg),
            crate::core::CoreAppError::Other(msg) => AppError::Other(msg),
        }
    }
}