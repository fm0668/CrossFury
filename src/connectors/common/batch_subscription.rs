//! 批量订阅管理器
//! 实现WebSocket优化重构方案中的小批量分批订阅功能

use std::time::{Duration, SystemTime};
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{sleep, timeout};
use log::{debug, info, warn, error};
use serde::{Deserialize, Serialize};
use crate::types::config::SubscriptionStatus;
use crate::types::common::DataType;
use crate::types::errors::ConnectorError;

/// 批量订阅管理器
/// 负责将大量订阅请求分批处理，避免交易所限制
#[derive(Debug, Clone)]
pub struct BatchSubscriptionManager {
    /// 配置参数
    config: BatchSubscriptionConfig,
    /// 当前状态
    state: Arc<RwLock<BatchSubscriptionState>>,
}

/// 批量订阅配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSubscriptionConfig {
    /// 每批订阅的最大数量
    pub batch_size: usize,
    /// 批次间的延迟时间（毫秒）
    pub batch_delay_ms: u64,
    /// 订阅超时时间（毫秒）
    pub subscription_timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试延迟（毫秒）
    pub retry_delay_ms: u64,
    /// 是否启用批量订阅
    pub enabled: bool,
    /// 并发批次数量限制
    pub max_concurrent_batches: usize,
}

/// 批量订阅状态
#[derive(Debug)]
struct BatchSubscriptionState {
    /// 待处理的订阅队列
    pending_queue: VecDeque<SubscriptionRequest>,
    /// 正在处理的批次
    active_batches: HashMap<String, BatchInfo>,
    /// 订阅状态映射
    subscription_status: HashMap<String, SubscriptionStatus>,
    /// 统计信息
    stats: BatchSubscriptionStats,
    /// 最后一次批处理时间
    last_batch_time: Option<SystemTime>,
}

/// 订阅请求
#[derive(Debug, Clone)]
pub struct SubscriptionRequest {
    /// 交易对符号
    pub symbol: String,
    /// 数据类型
    pub data_types: Vec<DataType>,
    /// 请求时间
    pub request_time: SystemTime,
    /// 重试次数
    pub retry_count: u32,
    /// 优先级（数字越小优先级越高）
    pub priority: u32,
}

/// 批次信息
#[derive(Debug, Clone)]
struct BatchInfo {
    /// 批次ID
    pub batch_id: String,
    /// 包含的订阅请求
    pub requests: Vec<SubscriptionRequest>,
    /// 开始时间
    pub start_time: SystemTime,
    /// 状态
    pub status: BatchStatus,
    /// 重试次数
    pub retry_count: u32,
}

/// 批次状态
#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchStatus {
    Pending,
    Processing,
    Completed,
    Failed(String),
    Retrying,
}

/// 批量订阅统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSubscriptionStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功订阅数
    pub successful_subscriptions: u64,
    /// 失败订阅数
    pub failed_subscriptions: u64,
    /// 处理的批次数
    pub total_batches: u64,
    /// 平均批处理时间（毫秒）
    pub avg_batch_time_ms: f64,
    /// 当前队列长度
    pub queue_length: usize,
    /// 活跃批次数
    pub active_batches: usize,
}

/// 批量订阅结果
#[derive(Debug, Clone)]
pub struct BatchSubscriptionResult {
    /// 批次ID
    pub batch_id: String,
    /// 总请求数
    pub total_requests: usize,
    /// 成功数
    pub successful: usize,
    /// 失败数
    pub failed: usize,
    /// 处理时间
    pub processing_time: Duration,
    /// 失败的符号和原因
    pub failed_symbols: Vec<(String, String)>,
}

impl Default for BatchSubscriptionConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            batch_delay_ms: 1000,        // 1秒
            subscription_timeout_ms: 5000, // 5秒
            max_retries: 3,
            retry_delay_ms: 2000,         // 2秒
            enabled: true,
            max_concurrent_batches: 3,
        }
    }
}

impl Default for BatchSubscriptionState {
    fn default() -> Self {
        Self {
            pending_queue: VecDeque::new(),
            active_batches: HashMap::new(),
            subscription_status: HashMap::new(),
            stats: BatchSubscriptionStats::default(),
            last_batch_time: None,
        }
    }
}

impl Default for BatchSubscriptionStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_subscriptions: 0,
            failed_subscriptions: 0,
            total_batches: 0,
            avg_batch_time_ms: 0.0,
            queue_length: 0,
            active_batches: 0,
        }
    }
}

impl BatchSubscriptionManager {
    /// 创建新的批量订阅管理器
    pub fn new(config: BatchSubscriptionConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(BatchSubscriptionState::default())),
        }
    }

    /// 使用默认配置创建管理器
    pub fn with_default_config() -> Self {
        Self::new(BatchSubscriptionConfig::default())
    }

    /// 添加订阅请求
    pub async fn add_subscription_request(
        &self,
        symbol: String,
        data_types: Vec<DataType>,
        priority: Option<u32>,
    ) -> Result<(), ConnectorError> {
        if !self.config.enabled {
            return Err(ConnectorError::SubscriptionFailed(
                "批量订阅管理器未启用".to_string()
            ));
        }

        let mut state = self.state.write().await;
        
        // 检查是否已经在订阅中
        if let Some(status) = state.subscription_status.get(&symbol) {
            match status {
                SubscriptionStatus::Active => {
                    debug!("[BatchSubscription] 符号 {} 已经订阅", symbol);
                    return Ok(());
                }
                SubscriptionStatus::Subscribing => {
                    debug!("[BatchSubscription] 符号 {} 正在订阅中", symbol);
                    return Ok(());
                }
                _ => {}
            }
        }
        
        let request = SubscriptionRequest {
            symbol: symbol.clone(),
            data_types,
            request_time: SystemTime::now(),
            retry_count: 0,
            priority: priority.unwrap_or(100), // 默认优先级
        };
        
        // 按优先级插入队列
        let insert_pos = state.pending_queue
            .iter()
            .position(|req| req.priority > request.priority)
            .unwrap_or(state.pending_queue.len());
        
        state.pending_queue.insert(insert_pos, request);
        state.subscription_status.insert(symbol.clone(), SubscriptionStatus::Pending);
        state.stats.total_requests += 1;
        state.stats.queue_length = state.pending_queue.len();
        
        debug!("[BatchSubscription] 添加订阅请求: {} (优先级: {}, 队列长度: {})", 
               symbol, priority.unwrap_or(100), state.pending_queue.len());
        
        Ok(())
    }

    /// 批量添加订阅请求
    pub async fn add_batch_subscription_requests(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>,
        priority: Option<u32>,
    ) -> Result<(), ConnectorError> {
        for symbol in symbols {
            self.add_subscription_request(symbol, data_types.clone(), priority).await?;
        }
        Ok(())
    }

    /// 处理下一个批次
    pub async fn process_next_batch<F, Fut>(
        &self,
        subscription_handler: F,
    ) -> Option<BatchSubscriptionResult>
    where
        F: Fn(Vec<SubscriptionRequest>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<Vec<(String, bool, Option<String>)>, ConnectorError>> + Send,
    {
        if !self.config.enabled {
            return None;
        }

        let batch_requests = {
            let mut state = self.state.write().await;
            
            // 检查是否有待处理的请求
            if state.pending_queue.is_empty() {
                return None;
            }
            
            // 检查并发批次限制
            if state.active_batches.len() >= self.config.max_concurrent_batches {
                debug!("[BatchSubscription] 达到最大并发批次限制: {}", self.config.max_concurrent_batches);
                return None;
            }
            
            // 检查批次间延迟
            if let Some(last_time) = state.last_batch_time {
                let delay = Duration::from_millis(self.config.batch_delay_ms);
                if last_time.elapsed().unwrap_or(Duration::from_secs(0)) < delay {
                    debug!("[BatchSubscription] 等待批次间延迟");
                    return None;
                }
            }
            
            // 提取一批请求
            let batch_size = self.config.batch_size.min(state.pending_queue.len());
            let mut batch_requests = Vec::with_capacity(batch_size);
            
            for _ in 0..batch_size {
                if let Some(request) = state.pending_queue.pop_front() {
                    // 更新状态为订阅中
                    state.subscription_status.insert(
                        request.symbol.clone(),
                        SubscriptionStatus::Subscribing,
                    );
                    batch_requests.push(request);
                }
            }
            
            state.stats.queue_length = state.pending_queue.len();
            state.last_batch_time = Some(SystemTime::now());
            
            batch_requests
        };
        
        if batch_requests.is_empty() {
            return None;
        }
        
        let batch_id = format!("batch_{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        let start_time = SystemTime::now();
        
        // 创建批次信息
        {
            let mut state = self.state.write().await;
            let batch_info = BatchInfo {
                batch_id: batch_id.clone(),
                requests: batch_requests.clone(),
                start_time,
                status: BatchStatus::Processing,
                retry_count: 0,
            };
            state.active_batches.insert(batch_id.clone(), batch_info);
            state.stats.active_batches = state.active_batches.len();
        }
        
        info!("[BatchSubscription] 开始处理批次 {} (包含 {} 个订阅)", batch_id, batch_requests.len());
        
        // 执行订阅
        let timeout_duration = Duration::from_millis(self.config.subscription_timeout_ms);
        let subscription_result = timeout(timeout_duration, subscription_handler(batch_requests.clone())).await;
        
        let processing_time = start_time.elapsed().unwrap_or(Duration::from_secs(0));
        
        let result = match subscription_result {
            Ok(Ok(results)) => {
                self.handle_batch_success(&batch_id, &batch_requests, results, processing_time).await
            }
            Ok(Err(e)) => {
                self.handle_batch_error(&batch_id, &batch_requests, &e.to_string(), processing_time).await
            }
            Err(_) => {
                self.handle_batch_timeout(&batch_id, &batch_requests, processing_time).await
            }
        };
        
        // 清理批次信息
        {
            let mut state = self.state.write().await;
            state.active_batches.remove(&batch_id);
            state.stats.active_batches = state.active_batches.len();
            state.stats.total_batches += 1;
            
            // 更新平均处理时间
            let total_time = state.stats.avg_batch_time_ms * (state.stats.total_batches - 1) as f64;
            state.stats.avg_batch_time_ms = (total_time + processing_time.as_millis() as f64) / state.stats.total_batches as f64;
        }
        
        Some(result)
    }

    /// 处理批次成功
    async fn handle_batch_success(
        &self,
        batch_id: &str,
        requests: &[SubscriptionRequest],
        results: Vec<(String, bool, Option<String>)>,
        processing_time: Duration,
    ) -> BatchSubscriptionResult {
        let mut state = self.state.write().await;
        let mut successful = 0;
        let mut failed = 0;
        let mut failed_symbols = Vec::new();
        
        // 创建结果映射
        let result_map: HashMap<String, (bool, Option<String>)> = results
            .into_iter()
            .map(|(symbol, success, reason)| (symbol, (success, reason)))
            .collect();
        
        for request in requests {
            if let Some((success, reason)) = result_map.get(&request.symbol) {
                if *success {
                    state.subscription_status.insert(request.symbol.clone(), SubscriptionStatus::Active);
                    state.stats.successful_subscriptions += 1;
                    successful += 1;
                    debug!("[BatchSubscription] 订阅成功: {}", request.symbol);
                } else {
                    let error_reason = reason.clone().unwrap_or_else(|| "未知错误".to_string());
                    state.subscription_status.insert(
                        request.symbol.clone(),
                        SubscriptionStatus::Failed(error_reason.clone()),
                    );
                    state.stats.failed_subscriptions += 1;
                    failed += 1;
                    failed_symbols.push((request.symbol.clone(), error_reason.clone()));
                    warn!("[BatchSubscription] 订阅失败: {} - {}", request.symbol, error_reason);
                }
            } else {
                // 没有结果，视为失败
                let error_reason = "没有返回结果".to_string();
                state.subscription_status.insert(
                    request.symbol.clone(),
                    SubscriptionStatus::Failed(error_reason.clone()),
                );
                state.stats.failed_subscriptions += 1;
                failed += 1;
                failed_symbols.push((request.symbol.clone(), error_reason));
            }
        }
        
        info!("[BatchSubscription] 批次 {} 处理完成: 成功 {}, 失败 {}, 耗时 {}ms", 
              batch_id, successful, failed, processing_time.as_millis());
        
        BatchSubscriptionResult {
            batch_id: batch_id.to_string(),
            total_requests: requests.len(),
            successful,
            failed,
            processing_time,
            failed_symbols,
        }
    }

    /// 处理批次错误
    async fn handle_batch_error(
        &self,
        batch_id: &str,
        requests: &[SubscriptionRequest],
        error: &str,
        processing_time: Duration,
    ) -> BatchSubscriptionResult {
        let mut state = self.state.write().await;
        let mut failed_symbols = Vec::new();
        
        // 所有请求都标记为失败
        for request in requests {
            state.subscription_status.insert(
                request.symbol.clone(),
                SubscriptionStatus::Failed(error.to_string()),
            );
            state.stats.failed_subscriptions += 1;
            failed_symbols.push((request.symbol.clone(), error.to_string()));
        }
        
        error!("[BatchSubscription] 批次 {} 处理失败: {} (耗时 {}ms)", 
               batch_id, error, processing_time.as_millis());
        
        BatchSubscriptionResult {
            batch_id: batch_id.to_string(),
            total_requests: requests.len(),
            successful: 0,
            failed: requests.len(),
            processing_time,
            failed_symbols,
        }
    }

    /// 处理批次超时
    async fn handle_batch_timeout(
        &self,
        batch_id: &str,
        requests: &[SubscriptionRequest],
        processing_time: Duration,
    ) -> BatchSubscriptionResult {
        let timeout_error = "订阅超时".to_string();
        self.handle_batch_error(batch_id, requests, &timeout_error, processing_time).await
    }

    /// 重试失败的订阅
    pub async fn retry_failed_subscriptions(&self) -> Result<usize, ConnectorError> {
        let mut state = self.state.write().await;
        let mut retry_count = 0;
        
        // 收集需要重试的订阅
        let failed_symbols: Vec<String> = state
            .subscription_status
            .iter()
            .filter_map(|(symbol, status)| {
                if matches!(status, SubscriptionStatus::Failed(_)) {
                    Some(symbol.clone())
                } else {
                    None
                }
            })
            .collect();
        
        for symbol in failed_symbols {
            // 重新添加到队列（高优先级）
            let request = SubscriptionRequest {
                symbol: symbol.clone(),
                data_types: vec![DataType::OrderBook], // 默认订阅订单簿
                request_time: SystemTime::now(),
                retry_count: 0,
                priority: 1, // 高优先级
            };
            
            state.pending_queue.push_front(request);
            state.subscription_status.insert(symbol, SubscriptionStatus::Retrying(1));
            retry_count += 1;
        }
        
        state.stats.queue_length = state.pending_queue.len();
        
        if retry_count > 0 {
            info!("[BatchSubscription] 重新排队 {} 个失败的订阅", retry_count);
        }
        
        Ok(retry_count)
    }

    /// 获取订阅状态
    pub async fn get_subscription_status(&self, symbol: &str) -> Option<SubscriptionStatus> {
        let state = self.state.read().await;
        state.subscription_status.get(symbol).cloned()
    }

    /// 获取所有订阅状态
    pub async fn get_all_subscription_status(&self) -> HashMap<String, SubscriptionStatus> {
        let state = self.state.read().await;
        state.subscription_status.clone()
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> BatchSubscriptionStats {
        let state = self.state.read().await;
        state.stats.clone()
    }

    /// 清空队列
    pub async fn clear_queue(&self) {
        let mut state = self.state.write().await;
        state.pending_queue.clear();
        state.stats.queue_length = 0;
        info!("[BatchSubscription] 队列已清空");
    }

    /// 获取队列长度
    pub async fn get_queue_length(&self) -> usize {
        let state = self.state.read().await;
        state.pending_queue.len()
    }

    /// 是否有待处理的请求
    pub async fn has_pending_requests(&self) -> bool {
        let state = self.state.read().await;
        !state.pending_queue.is_empty()
    }

    /// 更新配置
    pub async fn update_config(&mut self, new_config: BatchSubscriptionConfig) {
        self.config = new_config;
        info!("[BatchSubscription] 配置已更新");
    }

    /// 启动自动批处理循环
    pub async fn start_auto_processing<F, Fut>(
        &self,
        subscription_handler: F,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) where
        F: Fn(Vec<SubscriptionRequest>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<Vec<(String, bool, Option<String>)>, ConnectorError>> + Send,
    {
        let manager = self.clone();
        
        tokio::spawn(async move {
            info!("[BatchSubscription] 启动自动批处理循环");
            
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("[BatchSubscription] 收到关闭信号，停止自动批处理");
                        break;
                    }
                    _ = sleep(Duration::from_millis(manager.config.batch_delay_ms)) => {
                        if let Some(result) = manager.process_next_batch(subscription_handler.clone()).await {
                            debug!("[BatchSubscription] 自动处理批次完成: {}", result.batch_id);
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration as TokioDuration};

    #[tokio::test]
    async fn test_batch_subscription_manager_creation() {
        let manager = BatchSubscriptionManager::with_default_config();
        let stats = manager.get_stats().await;
        
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.queue_length, 0);
    }

    #[tokio::test]
    async fn test_add_subscription_request() {
        let manager = BatchSubscriptionManager::with_default_config();
        
        let result = manager.add_subscription_request(
            "BTCUSDT".to_string(),
            vec![DataType::OrderBook],
            Some(1),
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(manager.get_queue_length().await, 1);
        
        let status = manager.get_subscription_status("BTCUSDT").await;
        assert_eq!(status, Some(SubscriptionStatus::Pending));
    }

    #[tokio::test]
    async fn test_batch_processing() {
        let manager = BatchSubscriptionManager::with_default_config();
        
        // 添加一些订阅请求
        for i in 0..5 {
            let symbol = format!("SYMBOL{}", i);
            manager.add_subscription_request(
                symbol,
                vec![DataType::OrderBook],
                None,
            ).await.unwrap();
        }
        
        // 模拟订阅处理器
        let handler = |requests: Vec<SubscriptionRequest>| async move {
            let mut results = Vec::new();
            for request in requests {
                // 模拟成功订阅
                results.push((request.symbol, true, None));
            }
            Ok(results)
        };
        
        let result = manager.process_next_batch(handler).await;
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert!(result.successful > 0);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let manager = BatchSubscriptionManager::with_default_config();
        
        // 添加不同优先级的请求
        manager.add_subscription_request("LOW".to_string(), vec![DataType::OrderBook], Some(100)).await.unwrap();
        manager.add_subscription_request("HIGH".to_string(), vec![DataType::OrderBook], Some(1)).await.unwrap();
        manager.add_subscription_request("MEDIUM".to_string(), vec![DataType::OrderBook], Some(50)).await.unwrap();
        
        // 处理批次
        let handler = |requests: Vec<SubscriptionRequest>| async move {
            // 验证第一个请求是高优先级的
            assert_eq!(requests[0].symbol, "HIGH");
            assert_eq!(requests[1].symbol, "MEDIUM");
            assert_eq!(requests[2].symbol, "LOW");
            
            let mut results = Vec::new();
            for request in requests {
                results.push((request.symbol, true, None));
            }
            Ok(results)
        };
        
        let result = manager.process_next_batch(handler).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_retry_failed_subscriptions() {
        let manager = BatchSubscriptionManager::with_default_config();
        
        // 手动设置一些失败状态
        {
            let mut state = manager.state.write().await;
            state.subscription_status.insert("FAILED1".to_string(), SubscriptionStatus::Failed("测试失败".to_string()));
            state.subscription_status.insert("FAILED2".to_string(), SubscriptionStatus::Failed("测试失败".to_string()));
        }
        
        let retry_count = manager.retry_failed_subscriptions().await.unwrap();
        assert_eq!(retry_count, 2);
        assert_eq!(manager.get_queue_length().await, 2);
    }
}