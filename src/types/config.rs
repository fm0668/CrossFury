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
    pub data_types: Vec<DataType>,
    pub depth_levels: Option<u32>, // 订单簿深度
    pub update_speed: Option<UpdateSpeed>,
}

/// 数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    OrderBook,
    Trades,
    Ticker,
    Kline,
    UserData,
}

/// 更新速度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateSpeed {
    Slow,    // 1000ms
    Normal,  // 100ms
    Fast,    // 10ms
    Realtime, // 实时
}