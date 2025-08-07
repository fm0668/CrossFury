//! 符号转换器
//! 实现WebSocket优化重构方案中的符号格式智能转换功能

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{debug, warn, error};
use serde::{Deserialize, Serialize};
use regex::Regex;

/// 符号转换器
/// 负责在不同交易所之间转换交易对符号格式
#[derive(Debug, Clone)]
pub struct SymbolConverter {
    /// 配置参数
    config: SymbolConverterConfig,
    /// 符号映射缓存
    symbol_cache: Arc<RwLock<SymbolCache>>,
    /// 正则表达式缓存
    regex_cache: Arc<RwLock<RegexCache>>,
}

/// 符号转换器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolConverterConfig {
    /// 是否启用缓存
    pub enable_cache: bool,
    /// 缓存最大大小
    pub max_cache_size: usize,
    /// 是否启用严格模式（严格验证符号格式）
    pub strict_mode: bool,
    /// 默认基础货币
    pub default_base_currency: String,
    /// 默认报价货币
    pub default_quote_currency: String,
}

/// 符号缓存
#[derive(Debug)]
struct SymbolCache {
    /// 标准化符号到交易所符号的映射
    standard_to_exchange: HashMap<String, HashMap<String, String>>,
    /// 交易所符号到标准化符号的映射
    exchange_to_standard: HashMap<String, HashMap<String, String>>,
    /// 缓存使用计数
    usage_count: HashMap<String, u64>,
}

/// 正则表达式缓存
#[derive(Debug)]
struct RegexCache {
    /// 下划线格式正则
    underscore_regex: Option<Regex>,
    /// 斜杠格式正则
    slash_regex: Option<Regex>,
    /// 无分隔符格式正则
    no_separator_regex: Option<Regex>,
    /// 连字符格式正则
    hyphen_regex: Option<Regex>,
}

/// 符号格式类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolFormat {
    /// 下划线格式 (BTC_USDT)
    Underscore,
    /// 斜杠格式 (BTC/USDT)
    Slash,
    /// 无分隔符格式 (BTCUSDT)
    NoSeparator,
    /// 连字符格式 (BTC-USDT)
    Hyphen,
    /// 自定义格式
    Custom(String),
}

/// 符号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// 基础货币
    pub base_currency: String,
    /// 报价货币
    pub quote_currency: String,
    /// 原始符号
    pub original_symbol: String,
    /// 标准化符号
    pub normalized_symbol: String,
    /// 检测到的格式
    pub detected_format: SymbolFormat,
}

/// 转换结果
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// 转换后的符号
    pub converted_symbol: String,
    /// 符号信息
    pub symbol_info: SymbolInfo,
    /// 是否使用了缓存
    pub from_cache: bool,
    /// 转换耗时（纳秒）
    pub conversion_time_ns: u64,
}

/// 转换错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionError {
    /// 无效的符号格式
    InvalidFormat(String),
    /// 不支持的格式
    UnsupportedFormat(String),
    /// 解析失败
    ParseError(String),
    /// 缓存错误
    CacheError(String),
    /// 未知货币对
    UnknownPair(String),
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConversionError::InvalidFormat(msg) => write!(f, "无效格式: {}", msg),
            ConversionError::UnsupportedFormat(msg) => write!(f, "不支持的格式: {}", msg),
            ConversionError::ParseError(msg) => write!(f, "解析错误: {}", msg),
            ConversionError::CacheError(msg) => write!(f, "缓存错误: {}", msg),
            ConversionError::UnknownPair(msg) => write!(f, "未知货币对: {}", msg),
        }
    }
}

impl std::error::Error for ConversionError {}

impl Default for SymbolConverterConfig {
    fn default() -> Self {
        Self {
            enable_cache: true,
            max_cache_size: 10000,
            strict_mode: false,
            default_base_currency: "BTC".to_string(),
            default_quote_currency: "USDT".to_string(),
        }
    }
}

impl Default for SymbolCache {
    fn default() -> Self {
        Self {
            standard_to_exchange: HashMap::new(),
            exchange_to_standard: HashMap::new(),
            usage_count: HashMap::new(),
        }
    }
}

impl Default for RegexCache {
    fn default() -> Self {
        Self {
            underscore_regex: None,
            slash_regex: None,
            no_separator_regex: None,
            hyphen_regex: None,
        }
    }
}

impl SymbolConverter {
    /// 创建新的符号转换器
    pub fn new(config: SymbolConverterConfig) -> Self {
        Self {
            config,
            symbol_cache: Arc::new(RwLock::new(SymbolCache::default())),
            regex_cache: Arc::new(RwLock::new(RegexCache::default())),
        }
    }

    /// 使用默认配置创建转换器
    pub fn with_default_config() -> Self {
        Self::new(SymbolConverterConfig::default())
    }

    /// 初始化正则表达式
    async fn init_regex(&self) -> Result<(), ConversionError> {
        let mut cache = self.regex_cache.write().await;
        
        if cache.underscore_regex.is_none() {
            cache.underscore_regex = Some(
                Regex::new(r"^([A-Z0-9]+)_([A-Z0-9]+)$")
                    .map_err(|e| ConversionError::ParseError(format!("下划线正则编译失败: {}", e)))?
            );
        }
        
        if cache.slash_regex.is_none() {
            cache.slash_regex = Some(
                Regex::new(r"^([A-Z0-9]+)/([A-Z0-9]+)$")
                    .map_err(|e| ConversionError::ParseError(format!("斜杠正则编译失败: {}", e)))?
            );
        }
        
        if cache.hyphen_regex.is_none() {
            cache.hyphen_regex = Some(
                Regex::new(r"^([A-Z0-9]+)-([A-Z0-9]+)$")
                    .map_err(|e| ConversionError::ParseError(format!("连字符正则编译失败: {}", e)))?
            );
        }
        
        if cache.no_separator_regex.is_none() {
            // 无分隔符格式的正则比较复杂，需要智能识别
            cache.no_separator_regex = Some(
                Regex::new(r"^([A-Z0-9]{2,10})([A-Z0-9]{3,6})$")
                    .map_err(|e| ConversionError::ParseError(format!("无分隔符正则编译失败: {}", e)))?
            );
        }
        
        Ok(())
    }

    /// 检测符号格式
    pub async fn detect_format(&self, symbol: &str) -> Result<SymbolFormat, ConversionError> {
        self.init_regex().await?;
        
        let cache = self.regex_cache.read().await;
        let symbol_upper = symbol.to_uppercase();
        
        if let Some(ref regex) = cache.underscore_regex {
            if regex.is_match(&symbol_upper) {
                return Ok(SymbolFormat::Underscore);
            }
        }
        
        if let Some(ref regex) = cache.slash_regex {
            if regex.is_match(&symbol_upper) {
                return Ok(SymbolFormat::Slash);
            }
        }
        
        if let Some(ref regex) = cache.hyphen_regex {
            if regex.is_match(&symbol_upper) {
                return Ok(SymbolFormat::Hyphen);
            }
        }
        
        if let Some(ref regex) = cache.no_separator_regex {
            if regex.is_match(&symbol_upper) {
                return Ok(SymbolFormat::NoSeparator);
            }
        }
        
        Err(ConversionError::InvalidFormat(format!("无法识别符号格式: {}", symbol)))
    }

    /// 解析符号信息
    pub async fn parse_symbol(&self, symbol: &str) -> Result<SymbolInfo, ConversionError> {
        let start_time = std::time::Instant::now();
        
        let format = self.detect_format(symbol).await?;
        let symbol_upper = symbol.to_uppercase();
        
        let (base, quote) = match format {
            SymbolFormat::Underscore => self.parse_with_separator(&symbol_upper, '_').await?,
            SymbolFormat::Slash => self.parse_with_separator(&symbol_upper, '/').await?,
            SymbolFormat::Hyphen => self.parse_with_separator(&symbol_upper, '-').await?,
            SymbolFormat::NoSeparator => self.parse_no_separator(&symbol_upper).await?,
            SymbolFormat::Custom(ref pattern) => {
                return Err(ConversionError::UnsupportedFormat(format!("自定义格式暂不支持: {}", pattern)));
            }
        };
        
        let normalized_symbol = format!("{}_{}", base, quote);
        
        debug!("[SymbolConverter] 解析符号: {} -> {}_{} (格式: {:?}, 耗时: {}μs)", 
               symbol, base, quote, format, start_time.elapsed().as_micros());
        
        Ok(SymbolInfo {
            base_currency: base,
            quote_currency: quote,
            original_symbol: symbol.to_string(),
            normalized_symbol,
            detected_format: format,
        })
    }

    /// 使用分隔符解析
    async fn parse_with_separator(&self, symbol: &str, separator: char) -> Result<(String, String), ConversionError> {
        let parts: Vec<&str> = symbol.split(separator).collect();
        
        if parts.len() != 2 {
            return Err(ConversionError::ParseError(
                format!("符号 {} 使用分隔符 '{}' 解析失败，期望2个部分，实际{}个", symbol, separator, parts.len())
            ));
        }
        
        let base = parts[0].trim().to_string();
        let quote = parts[1].trim().to_string();
        
        if base.is_empty() || quote.is_empty() {
            return Err(ConversionError::ParseError(
                format!("符号 {} 解析后基础货币或报价货币为空", symbol)
            ));
        }
        
        Ok((base, quote))
    }

    /// 解析无分隔符格式
    async fn parse_no_separator(&self, symbol: &str) -> Result<(String, String), ConversionError> {
        // 常见的报价货币列表（按长度排序，优先匹配长的）
        let common_quotes = vec![
            "USDT", "USDC", "BUSD", "TUSD", "FDUSD",
            "BTC", "ETH", "BNB", "ADA", "DOT", "SOL",
            "USD", "EUR", "GBP", "JPY", "CNY",
        ];
        
        // 尝试匹配常见报价货币
        for quote in &common_quotes {
            if symbol.ends_with(quote) && symbol.len() > quote.len() {
                let base = &symbol[..symbol.len() - quote.len()];
                if !base.is_empty() && base.len() >= 2 {
                    return Ok((base.to_string(), quote.to_string()));
                }
            }
        }
        
        // 如果没有匹配到常见报价货币，使用启发式方法
        if symbol.len() >= 6 {
            // 假设最后3-4个字符是报价货币
            for quote_len in [4, 3] {
                if symbol.len() > quote_len {
                    let base = &symbol[..symbol.len() - quote_len];
                    let quote = &symbol[symbol.len() - quote_len..];
                    
                    if base.len() >= 2 && quote.len() >= 3 {
                        debug!("[SymbolConverter] 启发式解析: {} -> {}_{}", symbol, base, quote);
                        return Ok((base.to_string(), quote.to_string()));
                    }
                }
            }
        }
        
        Err(ConversionError::ParseError(
            format!("无法解析无分隔符符号: {}", symbol)
        ))
    }

    /// 转换为下划线格式
    pub async fn convert_to_underscore_format(&self, symbol: &str) -> Result<ConversionResult, ConversionError> {
        let start_time = std::time::Instant::now();
        
        // 检查缓存
        if self.config.enable_cache {
            if let Some(cached) = self.get_from_cache(symbol, "underscore").await {
                return Ok(ConversionResult {
                    converted_symbol: cached.clone(),
                    symbol_info: self.parse_symbol(symbol).await?,
                    from_cache: true,
                    conversion_time_ns: start_time.elapsed().as_nanos() as u64,
                });
            }
        }
        
        let symbol_info = self.parse_symbol(symbol).await?;
        let converted = format!("{}_{}", symbol_info.base_currency, symbol_info.quote_currency);
        
        // 更新缓存
        if self.config.enable_cache {
            self.update_cache(symbol, "underscore", &converted).await;
        }
        
        Ok(ConversionResult {
            converted_symbol: converted,
            symbol_info,
            from_cache: false,
            conversion_time_ns: start_time.elapsed().as_nanos() as u64,
        })
    }

    /// 转换为斜杠格式
    pub async fn convert_to_slash_format(&self, symbol: &str) -> Result<ConversionResult, ConversionError> {
        let start_time = std::time::Instant::now();
        
        // 检查缓存
        if self.config.enable_cache {
            if let Some(cached) = self.get_from_cache(symbol, "slash").await {
                return Ok(ConversionResult {
                    converted_symbol: cached.clone(),
                    symbol_info: self.parse_symbol(symbol).await?,
                    from_cache: true,
                    conversion_time_ns: start_time.elapsed().as_nanos() as u64,
                });
            }
        }
        
        let symbol_info = self.parse_symbol(symbol).await?;
        let converted = format!("{}/{}", symbol_info.base_currency, symbol_info.quote_currency);
        
        // 更新缓存
        if self.config.enable_cache {
            self.update_cache(symbol, "slash", &converted).await;
        }
        
        Ok(ConversionResult {
            converted_symbol: converted,
            symbol_info,
            from_cache: false,
            conversion_time_ns: start_time.elapsed().as_nanos() as u64,
        })
    }

    /// 转换为无分隔符格式
    pub async fn convert_to_no_separator_format(&self, symbol: &str) -> Result<ConversionResult, ConversionError> {
        let start_time = std::time::Instant::now();
        
        // 检查缓存
        if self.config.enable_cache {
            if let Some(cached) = self.get_from_cache(symbol, "no_separator").await {
                return Ok(ConversionResult {
                    converted_symbol: cached.clone(),
                    symbol_info: self.parse_symbol(symbol).await?,
                    from_cache: true,
                    conversion_time_ns: start_time.elapsed().as_nanos() as u64,
                });
            }
        }
        
        let symbol_info = self.parse_symbol(symbol).await?;
        let converted = format!("{}{}", symbol_info.base_currency, symbol_info.quote_currency);
        
        // 更新缓存
        if self.config.enable_cache {
            self.update_cache(symbol, "no_separator", &converted).await;
        }
        
        Ok(ConversionResult {
            converted_symbol: converted,
            symbol_info,
            from_cache: false,
            conversion_time_ns: start_time.elapsed().as_nanos() as u64,
        })
    }

    /// 转换为连字符格式
    pub async fn convert_to_hyphen_format(&self, symbol: &str) -> Result<ConversionResult, ConversionError> {
        let start_time = std::time::Instant::now();
        
        // 检查缓存
        if self.config.enable_cache {
            if let Some(cached) = self.get_from_cache(symbol, "hyphen").await {
                return Ok(ConversionResult {
                    converted_symbol: cached.clone(),
                    symbol_info: self.parse_symbol(symbol).await?,
                    from_cache: true,
                    conversion_time_ns: start_time.elapsed().as_nanos() as u64,
                });
            }
        }
        
        let symbol_info = self.parse_symbol(symbol).await?;
        let converted = format!("{}-{}", symbol_info.base_currency, symbol_info.quote_currency);
        
        // 更新缓存
        if self.config.enable_cache {
            self.update_cache(symbol, "hyphen", &converted).await;
        }
        
        Ok(ConversionResult {
            converted_symbol: converted,
            symbol_info,
            from_cache: false,
            conversion_time_ns: start_time.elapsed().as_nanos() as u64,
        })
    }

    /// 转换为指定格式
    pub async fn convert_to_format(&self, symbol: &str, target_format: SymbolFormat) -> Result<ConversionResult, ConversionError> {
        match target_format {
            SymbolFormat::Underscore => self.convert_to_underscore_format(symbol).await,
            SymbolFormat::Slash => self.convert_to_slash_format(symbol).await,
            SymbolFormat::NoSeparator => self.convert_to_no_separator_format(symbol).await,
            SymbolFormat::Hyphen => self.convert_to_hyphen_format(symbol).await,
            SymbolFormat::Custom(ref pattern) => {
                Err(ConversionError::UnsupportedFormat(format!("自定义格式暂不支持: {}", pattern)))
            }
        }
    }

    /// 批量转换符号
    pub async fn batch_convert(
        &self,
        symbols: Vec<String>,
        target_format: SymbolFormat,
    ) -> Vec<Result<ConversionResult, ConversionError>> {
        let mut results = Vec::with_capacity(symbols.len());
        
        for symbol in symbols {
            let result = self.convert_to_format(&symbol, target_format.clone()).await;
            results.push(result);
        }
        
        results
    }

    /// 从缓存获取
    async fn get_from_cache(&self, symbol: &str, format: &str) -> Option<String> {
        let cache = self.symbol_cache.read().await;
        let key = format!("{}-{}", symbol, format);
        
        if let Some(exchange_map) = cache.standard_to_exchange.get(&key) {
            if let Some(converted) = exchange_map.get("default") {
                return Some(converted.clone());
            }
        }
        
        None
    }

    /// 更新缓存
    async fn update_cache(&self, symbol: &str, format: &str, converted: &str) {
        let mut cache = self.symbol_cache.write().await;
        
        // 检查缓存大小限制
        if cache.standard_to_exchange.len() >= self.config.max_cache_size {
            self.evict_cache_entries(&mut cache).await;
        }
        
        let key = format!("{}-{}", symbol, format);
        
        cache.standard_to_exchange
            .entry(key.clone())
            .or_insert_with(HashMap::new)
            .insert("default".to_string(), converted.to_string());
        
        // 更新使用计数
        *cache.usage_count.entry(key).or_insert(0) += 1;
    }

    /// 清理缓存条目
    async fn evict_cache_entries(&self, cache: &mut SymbolCache) {
        // 移除使用次数最少的条目
        let mut entries: Vec<_> = cache.usage_count.iter().map(|(k, v)| (k.clone(), *v)).collect();
        entries.sort_by_key(|(_, count)| *count);
        
        let remove_count = cache.standard_to_exchange.len() / 4; // 移除25%的条目
        
        for (key, _) in entries.iter().take(remove_count) {
            cache.standard_to_exchange.remove(key);
            cache.exchange_to_standard.remove(key);
            cache.usage_count.remove(key);
        }
        
        debug!("[SymbolConverter] 清理缓存，移除 {} 个条目", remove_count);
    }

    /// 清空缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.symbol_cache.write().await;
        cache.standard_to_exchange.clear();
        cache.exchange_to_standard.clear();
        cache.usage_count.clear();
        
        debug!("[SymbolConverter] 缓存已清空");
    }

    /// 获取缓存统计信息
    pub async fn get_cache_stats(&self) -> CacheStats {
        let cache = self.symbol_cache.read().await;
        
        CacheStats {
            total_entries: cache.standard_to_exchange.len(),
            max_size: self.config.max_cache_size,
            hit_rate: 0.0, // TODO: 实现命中率统计
        }
    }

    /// 验证符号格式
    pub async fn validate_symbol(&self, symbol: &str) -> Result<bool, ConversionError> {
        if symbol.is_empty() {
            return Ok(false);
        }
        
        if self.config.strict_mode {
            // 严格模式下，必须能够解析符号
            self.parse_symbol(symbol).await.map(|_| true)
        } else {
            // 宽松模式下，只要格式能识别就行
            self.detect_format(symbol).await.map(|_| true)
        }
    }

    /// 更新配置
    pub async fn update_config(&mut self, new_config: SymbolConverterConfig) {
        self.config = new_config;
        debug!("[SymbolConverter] 配置已更新");
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub max_size: usize,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_symbol_converter_creation() {
        let converter = SymbolConverter::with_default_config();
        let stats = converter.get_cache_stats().await;
        
        assert_eq!(stats.total_entries, 0);
    }

    #[tokio::test]
    async fn test_detect_format() {
        let converter = SymbolConverter::with_default_config();
        
        assert_eq!(converter.detect_format("BTC_USDT").await.unwrap(), SymbolFormat::Underscore);
        assert_eq!(converter.detect_format("BTC/USDT").await.unwrap(), SymbolFormat::Slash);
        assert_eq!(converter.detect_format("BTC-USDT").await.unwrap(), SymbolFormat::Hyphen);
        assert_eq!(converter.detect_format("BTCUSDT").await.unwrap(), SymbolFormat::NoSeparator);
    }

    #[tokio::test]
    async fn test_parse_symbol() {
        let converter = SymbolConverter::with_default_config();
        
        let info = converter.parse_symbol("BTC_USDT").await.unwrap();
        assert_eq!(info.base_currency, "BTC");
        assert_eq!(info.quote_currency, "USDT");
        assert_eq!(info.detected_format, SymbolFormat::Underscore);
        
        let info = converter.parse_symbol("ETH/BTC").await.unwrap();
        assert_eq!(info.base_currency, "ETH");
        assert_eq!(info.quote_currency, "BTC");
        assert_eq!(info.detected_format, SymbolFormat::Slash);
    }

    #[tokio::test]
    async fn test_parse_no_separator() {
        let converter = SymbolConverter::with_default_config();
        
        let info = converter.parse_symbol("BTCUSDT").await.unwrap();
        assert_eq!(info.base_currency, "BTC");
        assert_eq!(info.quote_currency, "USDT");
        
        let info = converter.parse_symbol("ETHBUSD").await.unwrap();
        assert_eq!(info.base_currency, "ETH");
        assert_eq!(info.quote_currency, "BUSD");
    }

    #[tokio::test]
    async fn test_convert_formats() {
        let converter = SymbolConverter::with_default_config();
        
        // 测试转换为下划线格式
        let result = converter.convert_to_underscore_format("BTC/USDT").await.unwrap();
        assert_eq!(result.converted_symbol, "BTC_USDT");
        
        // 测试转换为斜杠格式
        let result = converter.convert_to_slash_format("BTC_USDT").await.unwrap();
        assert_eq!(result.converted_symbol, "BTC/USDT");
        
        // 测试转换为无分隔符格式
        let result = converter.convert_to_no_separator_format("BTC-USDT").await.unwrap();
        assert_eq!(result.converted_symbol, "BTCUSDT");
        
        // 测试转换为连字符格式
        let result = converter.convert_to_hyphen_format("BTCUSDT").await.unwrap();
        assert_eq!(result.converted_symbol, "BTC-USDT");
    }

    #[tokio::test]
    async fn test_batch_convert() {
        let converter = SymbolConverter::with_default_config();
        
        let symbols = vec![
            "BTC_USDT".to_string(),
            "ETH/BTC".to_string(),
            "ADAUSDT".to_string(),
        ];
        
        let results = converter.batch_convert(symbols, SymbolFormat::Slash).await;
        
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap().converted_symbol, "BTC/USDT");
        assert_eq!(results[1].as_ref().unwrap().converted_symbol, "ETH/BTC");
        assert_eq!(results[2].as_ref().unwrap().converted_symbol, "ADA/USDT");
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let converter = SymbolConverter::with_default_config();
        
        // 第一次转换
        let result1 = converter.convert_to_underscore_format("BTC/USDT").await.unwrap();
        assert!(!result1.from_cache);
        
        // 第二次转换应该使用缓存
        let result2 = converter.convert_to_underscore_format("BTC/USDT").await.unwrap();
        assert!(result2.from_cache);
        assert_eq!(result1.converted_symbol, result2.converted_symbol);
    }

    #[tokio::test]
    async fn test_validate_symbol() {
        let converter = SymbolConverter::with_default_config();
        
        assert!(converter.validate_symbol("BTC_USDT").await.unwrap());
        assert!(converter.validate_symbol("ETH/BTC").await.unwrap());
        assert!(!converter.validate_symbol("").await.unwrap());
        
        // 测试严格模式下的验证
        let mut strict_config = SymbolConverterConfig::default();
        strict_config.strict_mode = true;
        let strict_converter = SymbolConverter::new(strict_config);
        // 使用一个真正无法解析的格式：包含特殊字符
        assert!(strict_converter.validate_symbol("INVALID@FORMAT").await.is_err());
        // 使用一个太短的字符串
        assert!(strict_converter.validate_symbol("A").await.is_err());
    }
}