// src/types/config.rs - 配置相关类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 连接状态（与核心Trait定义保持一致）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConnectionStatus::Disconnected => write!(f, "DISCONNECTED"),
            ConnectionStatus::Connecting => write!(f, "CONNECTING"),
            ConnectionStatus::Connected => write!(f, "CONNECTED"),
            ConnectionStatus::Reconnecting => write!(f, "RECONNECTING"),
            ConnectionStatus::Error => write!(f, "ERROR"),
        }
    }
}

/// 连接器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub passphrase: Option<String>,
    pub testnet: bool,
    pub websocket_url: Option<String>,
    pub rest_api_url: Option<String>,
    pub reconnect_interval: u64, // 重连间隔（毫秒）
    pub max_reconnect_attempts: u32,
    pub ping_interval: u64, // 心跳间隔（毫秒）
    pub request_timeout: u64, // 请求超时（毫秒）
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            secret_key: None,
            passphrase: None,
            testnet: false,
            websocket_url: None,
            rest_api_url: None,
            reconnect_interval: 5000,
            max_reconnect_attempts: 10,
            ping_interval: 30000,
            request_timeout: 10000,
        }
    }
}

/// 数据流管理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowConfig {
    pub market_data_buffer_size: usize,
    pub event_channel_capacity: usize,
    pub enable_metrics: bool,
    pub metrics_interval: u64, // 指标收集间隔（毫秒）
}

impl Default for DataFlowConfig {
    fn default() -> Self {
        Self {
            market_data_buffer_size: 10000,
            event_channel_capacity: 1000,
            enable_metrics: true,
            metrics_interval: 1000,
        }
    }
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub check_interval: u64, // 检查间隔（毫秒）
    pub timeout: u64, // 超时时间（毫秒）
    pub max_failures: u32, // 最大失败次数
    pub enable_auto_recovery: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: 30000,
            timeout: 5000,
            max_failures: 3,
            enable_auto_recovery: true,
        }
    }
}

/// 系统配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub connectors: HashMap<String, ConnectorConfig>,
    pub data_flow: DataFlowConfig,
    pub health_check: HealthCheckConfig,
    pub log_level: String,
    pub enable_telemetry: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            connectors: HashMap::new(),
            data_flow: DataFlowConfig::default(),
            health_check: HealthCheckConfig::default(),
            log_level: "info".to_string(),
            enable_telemetry: false,
        }
    }
}

/// 订阅配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionConfig {
    pub symbols: Vec<String>,
    pub data_types: Vec<crate::types::common::DataType>,
    pub depth_levels: Option<u32>, // 订单簿深度
    pub update_speed: Option<UpdateSpeed>,
}

/// 更新速度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateSpeed {
    Slow,    // 1000ms
    Normal,  // 100ms
    Fast,    // 10ms
    Realtime, // 实时
}

/// 健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "HEALTHY"),
            HealthStatus::Degraded => write!(f, "DEGRADED"),
            HealthStatus::Unhealthy => write!(f, "UNHEALTHY"),
        }
    }
}

/// 连接统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ConnectionStats {
    pub connected_since: Option<chrono::DateTime<chrono::Utc>>,
    pub messages_received: u64,
    pub messages_sent: u64,
    pub reconnect_count: u32,
    pub last_ping_time: Option<chrono::DateTime<chrono::Utc>>,
    pub latency_ms: Option<f64>,
}

/// 连接质量信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionQuality {
    pub latency_ms: f64,
    pub packet_loss_rate: f64,
    pub stability_score: f64, // 0.0 - 1.0
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for ConnectionQuality {
    fn default() -> Self {
        Self {
            latency_ms: 0.0,
            packet_loss_rate: 0.0,
            stability_score: 1.0,
            last_updated: chrono::Utc::now(),
        }
    }
}

/// 连接质量等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionQualityLevel {
    Excellent,
    Good,
    Fair,
    Poor,
}

impl ConnectionQuality {
    /// 评估连接质量等级
    pub fn assess_quality(&self) -> ConnectionQualityLevel {
        if self.latency_ms > 1000.0 || self.packet_loss_rate > 0.1 || self.stability_score < 0.3 {
            ConnectionQualityLevel::Poor
        } else if self.latency_ms > 500.0 || self.packet_loss_rate > 0.05 || self.stability_score < 0.7 {
            ConnectionQualityLevel::Fair
        } else if self.latency_ms > 200.0 || self.packet_loss_rate > 0.01 || self.stability_score < 0.9 {
            ConnectionQualityLevel::Good
        } else {
            ConnectionQualityLevel::Excellent
        }
    }
    
    /// 检查连接质量是否较差
    pub fn is_poor(&self) -> bool {
        matches!(self.assess_quality(), ConnectionQualityLevel::Poor)
    }
}

/// 订阅状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    Pending,
    Subscribing,
    Active,
    Failed(String),
    Retrying(u32),
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SubscriptionStatus::Pending => write!(f, "PENDING"),
            SubscriptionStatus::Subscribing => write!(f, "SUBSCRIBING"),
            SubscriptionStatus::Active => write!(f, "ACTIVE"),
            SubscriptionStatus::Failed(reason) => write!(f, "FAILED: {}", reason),
            SubscriptionStatus::Retrying(count) => write!(f, "RETRYING({})", count),
        }
    }
}

/// 单个订阅结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionResult {
    pub symbol: String,
    pub data_type: crate::types::common::DataType,
    pub status: SubscriptionStatus,
    pub error: Option<String>,
}

/// 批量订阅结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSubscriptionResult {
    pub total_requested: usize,
    pub successful: usize,
    pub failed: usize,
    pub pending: usize,
    pub failed_symbols: Vec<(String, String)>, // (symbol, reason)
    pub results: Vec<SubscriptionResult>,
}

impl Default for BatchSubscriptionResult {
    fn default() -> Self {
        Self {
            total_requested: 0,
            successful: 0,
            failed: 0,
            pending: 0,
            failed_symbols: Vec::new(),
            results: Vec::new(),
        }
    }
}

/// 高级连接器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConnectorConfig {
    pub base: ConnectorConfig,
    
    // 高级连接配置
    pub emergency_ping_threshold: u32,  // 触发紧急ping的超时次数
    pub connection_quality_check_interval: u64,  // 连接质量检查间隔
    pub adaptive_timeout_enabled: bool,  // 是否启用自适应超时
    
    // 订阅配置
    pub subscription_batch_size: usize,  // 批量订阅大小
    pub subscription_retry_count: u32,   // 订阅重试次数
    pub symbol_validation_enabled: bool, // 是否启用符号验证
    
    // 数据处理配置
    pub depth_data_validation: bool,     // 深度数据验证
    pub price_precision_auto_detect: bool, // 自动检测价格精度
    pub orderbook_sort_validation: bool, // 订单簿排序验证
}

impl Default for AdvancedConnectorConfig {
    fn default() -> Self {
        Self {
            base: ConnectorConfig::default(),
            emergency_ping_threshold: 2,
            connection_quality_check_interval: 30000,
            adaptive_timeout_enabled: true,
            subscription_batch_size: 10,
            subscription_retry_count: 3,
            symbol_validation_enabled: true,
            depth_data_validation: true,
            price_precision_auto_detect: true,
            orderbook_sort_validation: true,
        }
    }
}

