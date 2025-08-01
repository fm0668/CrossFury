// src/types/errors.rs - 错误类型定义

use serde::{Deserialize, Serialize};
use std::fmt;

/// 连接器错误类型（与核心Trait定义保持一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectorError {
    // 连接相关错误
    ConnectionFailed(String),
    ConnectionLost(String),
    WebSocketError(String),
    
    // 认证相关错误
    AuthenticationFailed(String),
    InvalidCredentials(String),
    
    // 订阅相关错误
    SubscriptionFailed(String),
    InvalidSymbol(String),
    
    // 交易相关错误
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
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnectorError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ConnectorError::ConnectionLost(msg) => write!(f, "Connection lost: {}", msg),
            ConnectorError::WebSocketError(msg) => write!(f, "WebSocket error: {}", msg),
            ConnectorError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            ConnectorError::InvalidCredentials(msg) => write!(f, "Invalid credentials: {}", msg),
            ConnectorError::SubscriptionFailed(msg) => write!(f, "Subscription failed: {}", msg),
            ConnectorError::InvalidSymbol(msg) => write!(f, "Invalid symbol: {}", msg),
            ConnectorError::OrderPlacementFailed(msg) => write!(f, "Order placement failed: {}", msg),
            ConnectorError::OrderCancellationFailed(msg) => write!(f, "Order cancellation failed: {}", msg),
            ConnectorError::InsufficientBalance(msg) => write!(f, "Insufficient balance: {}", msg),
            ConnectorError::InvalidOrderParameters(msg) => write!(f, "Invalid order parameters: {}", msg),
            ConnectorError::DataParsingError(msg) => write!(f, "Data parsing error: {}", msg),
            ConnectorError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            ConnectorError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ConnectorError::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
            ConnectorError::RateLimitExceeded(msg) => write!(f, "Rate limit exceeded: {}", msg),
            ConnectorError::ExchangeNotFound(msg) => write!(f, "Exchange not found: {}", msg),
            ConnectorError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
            ConnectorError::InternalError(msg) => write!(f, "Internal error: {}", msg),
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
            ExecutionError::ConnectorError(err) => write!(f, "Connector error: {}", err),
            ExecutionError::RoutingError(msg) => write!(f, "Routing error: {}", msg),
            ExecutionError::RiskCheckFailed(msg) => write!(f, "Risk check failed: {}", msg),
            ExecutionError::PositionError(msg) => write!(f, "Position error: {}", msg),
            ExecutionError::InternalError(msg) => write!(f, "Internal error: {}", msg),
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
            RiskError::PositionLimitExceeded(msg) => write!(f, "Position limit exceeded: {}", msg),
            RiskError::OrderSizeLimitExceeded(msg) => write!(f, "Order size limit exceeded: {}", msg),
            RiskError::ExposureLimitExceeded(msg) => write!(f, "Exposure limit exceeded: {}", msg),
            RiskError::InvalidRiskParameters(msg) => write!(f, "Invalid risk parameters: {}", msg),
            RiskError::InternalError(msg) => write!(f, "Internal error: {}", msg),
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
            PositionError::PositionNotFound(msg) => write!(f, "Position not found: {}", msg),
            PositionError::InvalidPositionData(msg) => write!(f, "Invalid position data: {}", msg),
            PositionError::SyncError(msg) => write!(f, "Sync error: {}", msg),
            PositionError::CalculationError(msg) => write!(f, "Calculation error: {}", msg),
            PositionError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for PositionError {}