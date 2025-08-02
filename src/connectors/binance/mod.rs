//! Binance连接器模块
//! 
//! 提供Binance交易所的现货和期货连接器实现
//! 支持WebSocket实时数据和REST API交易功能

pub mod spot;
pub mod futures;
pub mod adapter;
pub mod websocket;
pub mod test;

// 重新导出主要类型
pub use spot::BinanceSpotConnector;
pub use futures::BinanceFuturesConnector;
pub use adapter::BinanceAdapter;
pub use websocket::BinanceWebSocketHandler;

// Binance特定的配置和常量
pub mod config {
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BinanceConfig {
        pub api_key: Option<String>,
        pub secret_key: Option<String>,
        pub testnet: bool,
        pub rate_limit_per_minute: u32,
    }
    
    impl Default for BinanceConfig {
        fn default() -> Self {
            Self {
                api_key: None,
                secret_key: None,
                testnet: false,
                rate_limit_per_minute: 1200,
            }
        }
    }
    
    // Binance现货WebSocket URLs - 参考Hummingbot实现
    pub const BINANCE_SPOT_WS_URL: &str = "wss://stream.binance.com:9443/ws";
    pub const BINANCE_SPOT_TESTNET_WS_URL: &str = "wss://testnet.binance.vision:9443/ws";
    
    // Binance期货WebSocket URLs
    pub const BINANCE_FUTURES_WS_URL: &str = "wss://fstream.binance.com/ws";
    pub const BINANCE_FUTURES_TESTNET_WS_URL: &str = "wss://stream.binancefuture.com/ws";
    
    // Binance现货REST API URLs - 参考Hummingbot实现
    pub const BINANCE_SPOT_API_URL: &str = "https://api.binance.com";
    pub const BINANCE_SPOT_TESTNET_API_URL: &str = "https://testnet.binance.vision";
    
    // Binance期货REST API URLs
    pub const BINANCE_FUTURES_API_URL: &str = "https://fapi.binance.com";
    pub const BINANCE_FUTURES_TESTNET_API_URL: &str = "https://testnet.binancefuture.com";
    
    // API版本常量
    pub const PUBLIC_API_VERSION: &str = "v3";
    pub const PRIVATE_API_VERSION: &str = "v3";
    
    // 常用端点路径
    pub const SERVER_TIME_PATH: &str = "/api/v3/time";
    pub const EXCHANGE_INFO_PATH: &str = "/api/v3/exchangeInfo";
    pub const DEPTH_PATH: &str = "/api/v3/depth";
    pub const TICKER_24HR_PATH: &str = "/api/v3/ticker/24hr";
}