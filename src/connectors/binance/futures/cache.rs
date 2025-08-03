//! Binance期货本地缓存模块
//! 
//! 实现市场数据缓存，提升响应速度和减少API调用

use crate::types::market_data::{DepthUpdate, Ticker, Kline};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use log::{debug, warn};
use chrono::Utc;

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    data: T,
    timestamp: Instant,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            data,
            timestamp: now,
            expires_at: now + ttl,
        }
    }
    
    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
    
    fn age(&self) -> Duration {
        Instant::now().duration_since(self.timestamp)
    }
}

/// 市场数据缓存
#[derive(Debug)]
pub struct MarketDataCache {
    /// 深度数据缓存
    depth_cache: Arc<RwLock<HashMap<String, CacheEntry<DepthUpdate>>>>,
    /// 行情数据缓存
    ticker_cache: Arc<RwLock<HashMap<String, CacheEntry<Ticker>>>>,
    /// K线数据缓存
    kline_cache: Arc<RwLock<HashMap<String, CacheEntry<Vec<Kline>>>>>,
    /// 缓存TTL配置
    cache_ttl: CacheTTLConfig,
    /// 缓存统计
    stats: Arc<RwLock<CacheStats>>,
}

/// 缓存TTL配置
#[derive(Debug, Clone)]
pub struct CacheTTLConfig {
    pub depth_ttl: Duration,
    pub ticker_ttl: Duration,
    pub kline_ttl: Duration,
}

impl Default for CacheTTLConfig {
    fn default() -> Self {
        Self {
            depth_ttl: Duration::from_secs(5),    // 深度数据5秒TTL
            ticker_ttl: Duration::from_secs(10),   // 行情数据10秒TTL
            kline_ttl: Duration::from_secs(60),    // K线数据60秒TTL
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub depth_hits: u64,
    pub depth_misses: u64,
    pub ticker_hits: u64,
    pub ticker_misses: u64,
    pub kline_hits: u64,
    pub kline_misses: u64,
    pub cleanup_count: u64,
    pub expired_entries: u64,
}

impl CacheStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let total_hits = self.depth_hits + self.ticker_hits + self.kline_hits;
        let total_requests = total_hits + self.depth_misses + self.ticker_misses + self.kline_misses;
        
        if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        }
    }
    
    /// 获取总请求数
    pub fn total_requests(&self) -> u64 {
        self.depth_hits + self.depth_misses + 
        self.ticker_hits + self.ticker_misses + 
        self.kline_hits + self.kline_misses
    }
}

impl MarketDataCache {
    /// 创建新的市场数据缓存
    pub fn new() -> Self {
        Self::with_config(CacheTTLConfig::default())
    }
    
    /// 使用指定配置创建缓存
    pub fn with_config(config: CacheTTLConfig) -> Self {
        Self {
            depth_cache: Arc::new(RwLock::new(HashMap::new())),
            ticker_cache: Arc::new(RwLock::new(HashMap::new())),
            kline_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }
    
    /// 获取深度数据
    pub async fn get_depth(&self, symbol: &str) -> Option<DepthUpdate> {
        let mut cache = self.depth_cache.write().await;
        let mut stats = self.stats.write().await;
        
        if let Some(entry) = cache.get(symbol) {
            if !entry.is_expired() {
                stats.depth_hits += 1;
                debug!("深度数据缓存命中: {} (age: {:?})", symbol, entry.age());
                return Some(entry.data.clone());
            } else {
                // 移除过期条目
                cache.remove(symbol);
                stats.expired_entries += 1;
            }
        }
        
        stats.depth_misses += 1;
        debug!("深度数据缓存未命中: {symbol}");
        None
    }
    
    /// 更新深度数据
    pub async fn update_depth(&self, symbol: &str, depth: DepthUpdate) {
        let mut cache = self.depth_cache.write().await;
        let entry = CacheEntry::new(depth, self.cache_ttl.depth_ttl);
        cache.insert(symbol.to_string(), entry);
        debug!("更新深度数据缓存: {symbol}");
    }
    
    /// 获取行情数据
    pub async fn get_ticker(&self, symbol: &str) -> Option<Ticker> {
        let mut cache = self.ticker_cache.write().await;
        let mut stats = self.stats.write().await;
        
        if let Some(entry) = cache.get(symbol) {
            if !entry.is_expired() {
                stats.ticker_hits += 1;
                debug!("行情数据缓存命中: {} (age: {:?})", symbol, entry.age());
                return Some(entry.data.clone());
            } else {
                cache.remove(symbol);
                stats.expired_entries += 1;
            }
        }
        
        stats.ticker_misses += 1;
        debug!("行情数据缓存未命中: {symbol}");
        None
    }
    
    /// 更新行情数据
    pub async fn update_ticker(&self, symbol: &str, ticker: Ticker) {
        let mut cache = self.ticker_cache.write().await;
        let entry = CacheEntry::new(ticker, self.cache_ttl.ticker_ttl);
        cache.insert(symbol.to_string(), entry);
        debug!("更新行情数据缓存: {symbol}");
    }
    
    /// 获取K线数据
    pub async fn get_klines(&self, symbol: &str) -> Option<Vec<Kline>> {
        let mut cache = self.kline_cache.write().await;
        let mut stats = self.stats.write().await;
        
        if let Some(entry) = cache.get(symbol) {
            if !entry.is_expired() {
                stats.kline_hits += 1;
                debug!("K线数据缓存命中: {} (age: {:?})", symbol, entry.age());
                return Some(entry.data.clone());
            } else {
                cache.remove(symbol);
                stats.expired_entries += 1;
            }
        }
        
        stats.kline_misses += 1;
        debug!("K线数据缓存未命中: {symbol}");
        None
    }
    
    /// 更新K线数据
    pub async fn update_klines(&self, symbol: &str, klines: Vec<Kline>) {
        let mut cache = self.kline_cache.write().await;
        let entry = CacheEntry::new(klines, self.cache_ttl.kline_ttl);
        cache.insert(symbol.to_string(), entry);
        debug!("更新K线数据缓存: {symbol}");
    }
    
    /// 清理过期条目
    pub async fn cleanup_expired(&self) {
        let mut stats = self.stats.write().await;
        let cleanup_start = Instant::now();
        
        // 清理深度数据缓存
        {
            let mut cache = self.depth_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|_, entry| !entry.is_expired());
            let removed = initial_count - cache.len();
            stats.expired_entries += removed as u64;
        }
        
        // 清理行情数据缓存
        {
            let mut cache = self.ticker_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|_, entry| !entry.is_expired());
            let removed = initial_count - cache.len();
            stats.expired_entries += removed as u64;
        }
        
        // 清理K线数据缓存
        {
            let mut cache = self.kline_cache.write().await;
            let initial_count = cache.len();
            cache.retain(|_, entry| !entry.is_expired());
            let removed = initial_count - cache.len();
            stats.expired_entries += removed as u64;
        }
        
        stats.cleanup_count += 1;
        let cleanup_duration = cleanup_start.elapsed();
        debug!("缓存清理完成，耗时: {cleanup_duration:?}");
    }
    
    /// 获取缓存统计信息
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }
    
    /// 重置缓存统计
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = CacheStats::default();
    }
    
    /// 清空所有缓存
    pub async fn clear_all(&self) {
        {
            let mut cache = self.depth_cache.write().await;
            cache.clear();
        }
        {
            let mut cache = self.ticker_cache.write().await;
            cache.clear();
        }
        {
            let mut cache = self.kline_cache.write().await;
            cache.clear();
        }
        
        warn!("所有缓存已清空");
    }
    
    /// 获取缓存大小信息
    pub async fn get_cache_sizes(&self) -> (usize, usize, usize) {
        let depth_size = self.depth_cache.read().await.len();
        let ticker_size = self.ticker_cache.read().await.len();
        let kline_size = self.kline_cache.read().await.len();
        
        (depth_size, ticker_size, kline_size)
    }
    
    /// 启动定期清理任务
    pub fn start_cleanup_task(&self, interval: Duration) {
        let cache = self.clone();
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                cache.cleanup_expired().await;
            }
        });
    }
}

impl Clone for MarketDataCache {
    fn clone(&self) -> Self {
        Self {
            depth_cache: Arc::clone(&self.depth_cache),
            ticker_cache: Arc::clone(&self.ticker_cache),
            kline_cache: Arc::clone(&self.kline_cache),
            cache_ttl: self.cache_ttl.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

impl Default for MarketDataCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = MarketDataCache::new();
        
        // 测试深度数据缓存
        let depth = DepthUpdate {
            symbol: "BTCUSDT".to_string(),
            first_update_id: 1,
            final_update_id: 12345,
            event_time: Utc::now().timestamp_millis(),
            best_bid_price: 50000.0,
            best_ask_price: 50001.0,
            depth_bids: vec![crate::types::market_data::PriceLevel { price: 50000.0, quantity: 1.0 }],
            depth_asks: vec![crate::types::market_data::PriceLevel { price: 50001.0, quantity: 1.0 }],
        };
        
        // 缓存未命中
        assert!(cache.get_depth("BTCUSDT").await.is_none());
        
        // 更新缓存
        cache.update_depth("BTCUSDT", depth.clone()).await;
        
        // 缓存命中
        let cached_depth = cache.get_depth("BTCUSDT").await;
        assert!(cached_depth.is_some());
        assert_eq!(cached_depth.unwrap().symbol, "BTCUSDT");
    }
    
    #[tokio::test]
    async fn test_cache_expiration() {
        let config = CacheTTLConfig {
            depth_ttl: Duration::from_millis(100),
            ticker_ttl: Duration::from_millis(100),
            kline_ttl: Duration::from_millis(100),
        };
        
        let cache = MarketDataCache::with_config(config);
        
        let depth = DepthUpdate {
            symbol: "BTCUSDT".to_string(),
            first_update_id: 1,
            final_update_id: 12345,
            event_time: Utc::now().timestamp_millis(),
            best_bid_price: 50000.0,
            best_ask_price: 50001.0,
            depth_bids: vec![crate::types::market_data::PriceLevel { price: 50000.0, quantity: 1.0 }],
            depth_asks: vec![crate::types::market_data::PriceLevel { price: 50001.0, quantity: 1.0 }],
        };
        
        // 更新缓存
        cache.update_depth("BTCUSDT", depth).await;
        
        // 立即获取应该命中
        assert!(cache.get_depth("BTCUSDT").await.is_some());
        
        // 等待过期
        sleep(Duration::from_millis(150)).await;
        
        // 过期后应该未命中
        assert!(cache.get_depth("BTCUSDT").await.is_none());
    }
    
    #[tokio::test]
    async fn test_cache_stats() {
        let cache = MarketDataCache::new();
        
        // 初始统计应该为0
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_requests(), 0);
        assert_eq!(stats.hit_rate(), 0.0);
        
        // 缓存未命中
        cache.get_depth("BTCUSDT").await;
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.depth_misses, 1);
        assert_eq!(stats.hit_rate(), 0.0);
        
        // 添加数据并命中
        let depth = DepthUpdate {
            symbol: "BTCUSDT".to_string(),
            first_update_id: 1,
            final_update_id: 12345,
            event_time: Utc::now().timestamp_millis(),
            best_bid_price: 50000.0,
            best_ask_price: 50001.0,
            depth_bids: vec![crate::types::market_data::PriceLevel { price: 50000.0, quantity: 1.0 }],
            depth_asks: vec![crate::types::market_data::PriceLevel { price: 50001.0, quantity: 1.0 }],
        };
        
        cache.update_depth("BTCUSDT", depth).await;
        cache.get_depth("BTCUSDT").await;
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.depth_hits, 1);
        assert_eq!(stats.depth_misses, 1);
        assert_eq!(stats.hit_rate(), 0.5);
    }
}