//! Binance期货连接器模块
//! 
//! 提供Binance期货交易所的连接器实现
//! 支持永续合约和交割合约的WebSocket实时数据和REST API交易功能

pub mod connector;
pub mod websocket;
pub mod rest_api;
pub mod message_parser;
pub mod config;
pub mod risk_manager;
pub mod cache;
pub mod performance_monitor;
pub mod test_framework;
pub mod advanced_features;

// 重新导出主要类型
pub use connector::BinanceFuturesConnector;
pub use websocket::BinanceFuturesWebSocketHandler;
pub use rest_api::BinanceFuturesRestClient;
pub use message_parser::BinanceFuturesMessageParser;
pub use config::BinanceFuturesConfig;
pub use risk_manager::RiskManager;
pub use cache::MarketDataCache;
pub use performance_monitor::PerformanceMonitor;
pub use test_framework::{TestScenarioBuilder, TestEnvironment, MockMarketDataGenerator, MockTradeExecutor};
pub use advanced_features::{AlgoTradingEngine, SmartRouter, AlgoStrategy, AlgoOrder};

// 期货特有的常量
pub mod constants {
    // Binance期货WebSocket URLs
    pub const BINANCE_FUTURES_WS_URL: &str = "wss://fstream.binance.com/ws";
    pub const BINANCE_FUTURES_TESTNET_WS_URL: &str = "wss://stream.binancefuture.com/ws";
    
    // Binance期货REST API URLs
    pub const BINANCE_FUTURES_API_URL: &str = "https://fapi.binance.com";
    pub const BINANCE_FUTURES_TESTNET_API_URL: &str = "https://testnet.binancefuture.com";
    
    // API版本常量
    pub const FUTURES_API_VERSION: &str = "v1";
    pub const FUTURES_API_VERSION_V2: &str = "v2";
    
    // 期货特有端点路径
    pub const FUTURES_EXCHANGE_INFO_PATH: &str = "/fapi/v1/exchangeInfo";
    pub const FUTURES_DEPTH_PATH: &str = "/fapi/v1/depth";
    pub const FUTURES_TICKER_24HR_PATH: &str = "/fapi/v1/ticker/24hr";
    pub const FUTURES_MARK_PRICE_PATH: &str = "/fapi/v1/premiumIndex";
    pub const FUTURES_FUNDING_RATE_PATH: &str = "/fapi/v1/fundingRate";
    pub const FUTURES_OPEN_INTEREST_PATH: &str = "/fapi/v1/openInterest";
    
    // 账户和交易端点
    pub const FUTURES_ACCOUNT_PATH: &str = "/fapi/v2/account";
    pub const FUTURES_POSITION_PATH: &str = "/fapi/v2/positionRisk";
    pub const FUTURES_ORDER_PATH: &str = "/fapi/v1/order";
    pub const FUTURES_LEVERAGE_PATH: &str = "/fapi/v1/leverage";
    pub const FUTURES_MARGIN_TYPE_PATH: &str = "/fapi/v1/marginType";
    
    // WebSocket流类型
    pub const DEPTH_STREAM_SUFFIX: &str = "@depth20@100ms";
    pub const TRADE_STREAM_SUFFIX: &str = "@aggTrade";
    pub const KLINE_STREAM_SUFFIX: &str = "@kline_";
    pub const TICKER_STREAM_SUFFIX: &str = "@ticker";
    pub const MARK_PRICE_STREAM_SUFFIX: &str = "@markPrice";
    pub const FUNDING_RATE_STREAM: &str = "!markPrice@arr";
    pub const OPEN_INTEREST_STREAM_SUFFIX: &str = "@openInterest";
    
    // 期货特有参数
    pub const DEFAULT_LEVERAGE: u8 = 1;
    pub const MAX_LEVERAGE: u8 = 125;
    pub const DEFAULT_MARGIN_TYPE: &str = "ISOLATED"; // ISOLATED 或 CROSSED
    pub const DEFAULT_POSITION_SIDE: &str = "BOTH"; // BOTH, LONG, SHORT
    
    // 速率限制
    pub const FUTURES_RATE_LIMIT_PER_MINUTE: u32 = 2400;
    pub const FUTURES_ORDER_RATE_LIMIT: u32 = 300; // 每10秒
}