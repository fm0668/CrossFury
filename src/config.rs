// config.rs - Centralized configuration system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use std::path::Path;
use std::sync::OnceLock;
use crate::exchange_types::Exchange;
use once_cell::sync::Lazy;
use crate::types::config::AdvancedConnectorConfig;

/// Global configuration singleton
pub static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns a reference to the global configuration.
/// If not yet initialized, uses the default configuration.
pub fn get_config() -> &'static Config {
    CONFIG.get().unwrap_or_else(|| {
        // Use a static reference to the default configuration.
        static DEFAULT_CONFIG: once_cell::sync::OnceCell<Config> = once_cell::sync::OnceCell::new();
        DEFAULT_CONFIG.get_or_init(Config::default)
    })
}

/// Initializes configuration from the given file path.
pub async fn init_config<P: AsRef<Path>>(path: P) -> Result<(), String> {
    let file_format = path.as_ref().extension().and_then(|os| os.to_str());
    
    let mut file = File::open(path.as_ref()).await
        .map_err(|e| format!("Failed to open config file: {e}"))?;
    
    let mut contents = String::new();
    file.read_to_string(&mut contents).await
        .map_err(|e| format!("Failed to read config file: {e}"))?;
    
    let config = match file_format {
        Some("toml") => toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse TOML config: {e:?}")),
        Some("json") => serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse JSON config: {e:?}")),
        Some("yaml") | Some("yml") => serde_yaml::from_str(&contents)
            .map_err(|e| format!("Failed to parse YAML config: {e:?}")),
        _ => Err("Unsupported config file format".to_string()),
    }?;
    
    CONFIG.set(config)
        .map_err(|_| "Configuration already initialized".to_string())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub connection: ConnectionConfig,
    pub arbitrage: ArbitrageConfig,
    pub exchanges: HashMap<String, ExchangeConfig>,
    pub token_configs: HashMap<String, TokenConfig>,
    pub features: FeatureFlags,
    pub websocket_optimization: WebSocketOptimizationConfig,
    pub advanced_connectors: HashMap<String, AdvancedConnectorConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeneralConfig {
    pub log_level: String,
    pub metrics_interval_secs: u64,
    pub csv_flush_interval_secs: u64,
    pub worker_threads: usize,
    pub scanner_threads: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionConfig {
    pub max_ws_connections: usize,
    pub max_subscriptions_per_connection: usize,
    pub ping_interval_ms: u64,
    pub reconnect_delay_base_secs: f64,
    pub max_reconnect_delay_secs: f64,
    pub stale_connection_timeout_ms: i64,
    pub force_reconnect_timeout_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArbitrageConfig {
    pub min_profit_threshold_pct: f64,
    pub max_reasonable_profit_pct: f64,
    pub default_trade_size_usd: f64,
    pub default_slippage_pct: f64,
    pub large_order_slippage_pct: f64,
    pub max_path_length: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExchangeConfig {
    pub websocket_url: String,
    pub api_url: Option<String>,  // Added for futures exchanges REST API
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub maker_fee_pct: f64,
    pub taker_fee_pct: f64,
    pub max_retries: usize,
    pub connection_timeout_secs: u64,
    pub ping_interval_secs: u64,
    pub batch_size: usize,
    pub supported_symbols: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenConfig {
    pub price_scale: i32,
    pub max_reasonable_profit_pct: f64,
    pub max_price_variation_pct: f64,
    pub slippage_factor: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureFlags {
    pub enable_multi_hop_arbitrage: bool,
    pub enable_adaptive_slippage: bool,
    pub enable_circuit_breakers: bool,
    pub enable_simd_json: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSocketOptimizationConfig {
    pub enable_emergency_ping: bool,
    pub emergency_ping_threshold: u32,
    pub emergency_ping_interval_ms: u64,
    pub enable_adaptive_timeout: bool,
    pub initial_timeout_ms: u64,
    pub max_timeout_ms: u64,
    pub timeout_adjustment_factor: f64,
    pub enable_batch_subscription: bool,
    pub batch_size: usize,
    pub batch_delay_ms: u64,
    pub connection_quality_check_interval_ms: u64,
    pub latency_threshold_ms: f64,
    pub packet_loss_threshold: f64,
}

/// Default configuration used when no config file is provided.
/// Note: We use the name DEFAULT_CONFIG here.
pub static DEFAULT_CONFIG: Lazy<Config> = Lazy::new(|| Config {
    general: GeneralConfig {
        log_level: String::from("info"),
        metrics_interval_secs: 10,
        csv_flush_interval_secs: 10,
        worker_threads: 7,
        scanner_threads: 3,
    },
    connection: ConnectionConfig {
        max_ws_connections: 25,
        max_subscriptions_per_connection: 50,
        ping_interval_ms: 500,
        reconnect_delay_base_secs: 0.5,
        max_reconnect_delay_secs: 5.0,
        stale_connection_timeout_ms: 30000,
        force_reconnect_timeout_ms: 45000,
    },
    arbitrage: ArbitrageConfig {
        min_profit_threshold_pct: 0.10,
        max_reasonable_profit_pct: 5.0,
        default_trade_size_usd: 2500.0,
        default_slippage_pct: 0.001,
        large_order_slippage_pct: 0.003,
        max_path_length: 3,
    },
    exchanges: HashMap::new(),
    token_configs: HashMap::new(),
    features: FeatureFlags {
        enable_multi_hop_arbitrage: true,
        enable_adaptive_slippage: true,
        enable_circuit_breakers: true,
        enable_simd_json: true,
    },
    websocket_optimization: WebSocketOptimizationConfig {
        enable_emergency_ping: true,
        emergency_ping_threshold: 2,
        emergency_ping_interval_ms: 5000,
        enable_adaptive_timeout: true,
        initial_timeout_ms: 10000,
        max_timeout_ms: 60000,
        timeout_adjustment_factor: 1.5,
        enable_batch_subscription: true,
        batch_size: 10,
        batch_delay_ms: 100,
        connection_quality_check_interval_ms: 30000,
        latency_threshold_ms: 500.0,
        packet_loss_threshold: 0.05,
    },
    advanced_connectors: HashMap::new(),
});

impl Config {
    /// Returns the default configuration.
    pub fn default() -> Self {
        DEFAULT_CONFIG.clone()
    }
    
    /// Returns a reference to the global configuration singleton.
    pub fn global() -> &'static OnceLock<Config> {
        &CONFIG
    }
    
    /// Load configuration from a file.
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file_format = path.as_ref().extension().and_then(|os| os.to_str());
        let mut file = File::open(path.as_ref()).await
            .map_err(|e| format!("Failed to open config file: {e}"))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await
            .map_err(|e| format!("Failed to read config file: {e}"))?;
        match file_format {
            Some("toml") => toml::from_str(&contents)
                .map_err(|e| format!("Failed to parse TOML config: {e:?}")),
            Some("json") => serde_json::from_str(&contents)
                .map_err(|e| format!("Failed to parse JSON config: {e:?}")),
            Some("yaml") | Some("yml") => serde_yaml::from_str(&contents)
                .map_err(|e| format!("Failed to parse YAML config: {e:?}")),
            _ => Err("Unsupported config file format".to_string()),
        }
    }
    
    /// Get exchange configuration by exchange enum.
    pub fn get_exchange_config(&self, exchange: &Exchange) -> Option<&ExchangeConfig> {
        self.exchanges.get(&exchange.to_string())
    }
    
    /// Get token configuration by symbol.
    pub fn get_token_config(&self, symbol: &str) -> Option<&TokenConfig> {
        // Try an exact match first.
        if let Some(config) = self.token_configs.get(symbol) {
            return Some(config);
        }
        // Otherwise, try to match by base token.
        let base_token = self.extract_base_token(symbol);
        self.token_configs.get(&base_token)
    }
    
    /// Helper to extract the base token from a symbol.
    fn extract_base_token(&self, symbol: &str) -> String {
        let quote_currencies = ["USDT", "USD", "BTC", "ETH", "USDC", "BNB"];
        for &quote in &quote_currencies {
            if symbol.ends_with(quote) && symbol.len() > quote.len() {
                return symbol[..symbol.len() - quote.len()].to_string();
            }
        }
        symbol.to_string()
    }
    
    /// Get WebSocket optimization configuration.
    pub fn get_websocket_optimization(&self) -> &WebSocketOptimizationConfig {
        &self.websocket_optimization
    }
    
    /// Get advanced connector configuration by exchange name.
    pub fn get_advanced_connector_config(&self, exchange: &str) -> Option<&AdvancedConnectorConfig> {
        self.advanced_connectors.get(exchange)
    }
    
    /// Set advanced connector configuration for an exchange.
    pub fn set_advanced_connector_config(&mut self, exchange: String, config: AdvancedConnectorConfig) {
        self.advanced_connectors.insert(exchange, config);
    }
    
    /// Check if WebSocket optimization is enabled.
    pub fn is_websocket_optimization_enabled(&self) -> bool {
        self.websocket_optimization.enable_emergency_ping ||
        self.websocket_optimization.enable_adaptive_timeout ||
        self.websocket_optimization.enable_batch_subscription
    }
}
