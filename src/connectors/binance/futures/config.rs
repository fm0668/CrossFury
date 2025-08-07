//! Binance期货连接器配置模块
//! 
//! 定义期货交易所连接的配置参数和常量

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Binance期货连接器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceFuturesConfig {
    /// API密钥
    pub api_key: Option<String>,
    /// 密钥
    pub secret_key: Option<String>,
    /// 是否使用测试网
    pub testnet: bool,
    /// 每分钟速率限制
    pub rate_limit_per_minute: u32,
    /// 订单速率限制（每10秒）
    pub order_rate_limit_per_10s: u32,
    /// 默认杠杆倍数
    pub default_leverage: u8,
    /// 默认保证金模式
    pub default_margin_type: MarginType,
    /// 默认持仓模式
    pub default_position_mode: PositionMode,
    /// 是否启用对冲模式
    pub hedge_mode: bool,
    /// WebSocket重连间隔（秒）
    pub ws_reconnect_interval: u64,
    /// REST API超时时间（秒）
    pub rest_timeout: u64,
    /// 订阅的交易对列表
    pub subscribed_symbols: Vec<String>,
    /// 每个交易对的杠杆配置
    pub symbol_leverage: HashMap<String, u8>,
    /// 每个交易对的保证金模式配置
    pub symbol_margin_type: HashMap<String, MarginType>,
}

/// 保证金模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarginType {
    /// 逐仓模式
    Isolated,
    /// 全仓模式
    Crossed,
}

/// 持仓模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PositionMode {
    /// 单向持仓模式
    OneWay,
    /// 双向持仓模式（对冲模式）
    Hedge,
}

/// 持仓方向
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PositionSide {
    /// 双向持仓（单向模式下使用）
    Both,
    /// 多头持仓
    Long,
    /// 空头持仓
    Short,
}

/// 订单类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FuturesOrderType {
    /// 限价单
    Limit,
    /// 市价单
    Market,
    /// 止损单
    Stop,
    /// 止损限价单
    StopMarket,
    /// 止盈单
    TakeProfit,
    /// 止盈限价单
    TakeProfitMarket,
    /// 跟踪止损单
    TrailingStopMarket,
}

/// 时间有效性
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimeInForce {
    /// 成交为止
    GTC,
    /// 立即成交或取消
    IOC,
    /// 全部成交或取消
    FOK,
    /// 当日有效
    GTX,
}

impl Default for BinanceFuturesConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            secret_key: None,
            testnet: false,
            rate_limit_per_minute: 2400,
            order_rate_limit_per_10s: 300,
            default_leverage: 1,
            default_margin_type: MarginType::Isolated,
            default_position_mode: PositionMode::OneWay,
            hedge_mode: false,
            ws_reconnect_interval: 30,
            rest_timeout: 10,
            subscribed_symbols: Vec::new(),
            symbol_leverage: HashMap::new(),
            symbol_margin_type: HashMap::new(),
        }
    }
}

impl BinanceFuturesConfig {
    /// 创建配置构建器
    pub fn builder() -> BinanceFuturesConfigBuilder {
        BinanceFuturesConfigBuilder::new()
    }
}

impl MarginType {
    /// 转换为API字符串
    pub fn to_api_string(&self) -> &'static str {
        match self {
            MarginType::Isolated => "ISOLATED",
            MarginType::Crossed => "CROSSED",
        }
    }
    
    /// 从API字符串解析
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ISOLATED" => Some(MarginType::Isolated),
            "CROSSED" => Some(MarginType::Crossed),
            _ => None,
        }
    }
}

impl PositionSide {
    /// 转换为API字符串
    pub fn to_api_string(&self) -> &'static str {
        match self {
            PositionSide::Both => "BOTH",
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
        }
    }
    
    /// 从API字符串解析
    pub fn from_api_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BOTH" => Some(PositionSide::Both),
            "LONG" => Some(PositionSide::Long),
            "SHORT" => Some(PositionSide::Short),
            _ => None,
        }
    }
}

impl FuturesOrderType {
    /// 转换为API字符串
    pub fn to_api_string(&self) -> &'static str {
        match self {
            FuturesOrderType::Limit => "LIMIT",
            FuturesOrderType::Market => "MARKET",
            FuturesOrderType::Stop => "STOP",
            FuturesOrderType::StopMarket => "STOP_MARKET",
            FuturesOrderType::TakeProfit => "TAKE_PROFIT",
            FuturesOrderType::TakeProfitMarket => "TAKE_PROFIT_MARKET",
            FuturesOrderType::TrailingStopMarket => "TRAILING_STOP_MARKET",
        }
    }
}

impl TimeInForce {
    /// 转换为API字符串
    pub fn to_api_string(&self) -> &'static str {
        match self {
            TimeInForce::GTC => "GTC",
            TimeInForce::IOC => "IOC",
            TimeInForce::FOK => "FOK",
            TimeInForce::GTX => "GTX",
        }
    }
}

/// 期货连接器配置构建器
pub struct BinanceFuturesConfigBuilder {
    config: BinanceFuturesConfig,
}

impl BinanceFuturesConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: BinanceFuturesConfig::default(),
        }
    }
    
    pub fn api_key(mut self, api_key: String) -> Self {
        self.config.api_key = Some(api_key);
        self
    }
    
    pub fn secret_key(mut self, secret_key: String) -> Self {
        self.config.secret_key = Some(secret_key);
        self
    }
    
    pub fn testnet(mut self, testnet: bool) -> Self {
        self.config.testnet = testnet;
        self
    }
    
    pub fn default_leverage(mut self, leverage: u8) -> Self {
        self.config.default_leverage = leverage.min(125).max(1);
        self
    }
    
    pub fn default_margin_type(mut self, margin_type: MarginType) -> Self {
        self.config.default_margin_type = margin_type;
        self
    }
    
    pub fn hedge_mode(mut self, hedge_mode: bool) -> Self {
        self.config.hedge_mode = hedge_mode;
        if hedge_mode {
            self.config.default_position_mode = PositionMode::Hedge;
        }
        self
    }
    
    pub fn subscribe_symbol(mut self, symbol: String) -> Self {
        self.config.subscribed_symbols.push(symbol);
        self
    }
    
    pub fn subscribed_symbols(mut self, symbols: Vec<String>) -> Self {
        self.config.subscribed_symbols = symbols;
        self
    }
    
    pub fn symbol_leverage(mut self, symbol: String, leverage: u8) -> Self {
        self.config.symbol_leverage.insert(symbol, leverage.min(125).max(1));
        self
    }
    
    pub fn build(self) -> BinanceFuturesConfig {
        self.config
    }
}

impl Default for BinanceFuturesConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}