//! Binance期货性能监控模块
//! 
//! 监控系统性能指标，包括延迟、吞吐量、错误率等

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use log::{warn, error};
use serde::{Serialize, Deserialize};

/// 性能指标类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    /// API请求延迟
    ApiLatency,
    /// WebSocket延迟
    WebSocketLatency,
    /// 订单执行延迟
    OrderLatency,
    /// 市场数据延迟
    MarketDataLatency,
    /// 错误率
    ErrorRate,
    /// 吞吐量
    Throughput,
    /// 缓存命中率
    CacheHitRate,
    /// 内存使用
    MemoryUsage,
    /// CPU使用
    CpuUsage,
}

/// 性能指标数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub tags: HashMap<String, String>,
}

/// 性能统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub count: u64,
    pub sum: f64,
}

impl PerformanceStats {
    fn new() -> Self {
        Self {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            avg: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
            count: 0,
            sum: 0.0,
        }
    }
    
    fn update(&mut self, values: &[f64]) {
        if values.is_empty() {
            return;
        }
        
        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        self.min = sorted_values[0];
        self.max = sorted_values[sorted_values.len() - 1];
        self.sum = sorted_values.iter().sum();
        self.count = sorted_values.len() as u64;
        self.avg = self.sum / self.count as f64;
        
        // 计算百分位数
        let len = sorted_values.len();
        self.p50 = sorted_values[len * 50 / 100];
        self.p95 = sorted_values[len * 95 / 100];
        self.p99 = sorted_values[len * 99 / 100];
    }
}

/// 延迟测量器
#[derive(Debug)]
pub struct LatencyMeasurer {
    start_time: Instant,
    metric_type: MetricType,
    tags: HashMap<String, String>,
}

impl LatencyMeasurer {
    pub fn new(metric_type: MetricType) -> Self {
        Self {
            start_time: Instant::now(),
            metric_type,
            tags: HashMap::new(),
        }
    }
    
    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }
    
    pub fn finish(self, monitor: &PerformanceMonitor) {
        let duration = self.start_time.elapsed();
        let latency_ms = duration.as_secs_f64() * 1000.0;
        
        tokio::spawn({
            let monitor = monitor.clone();
            let metric_type = self.metric_type;
            let tags = self.tags;
            
            async move {
                monitor.record_metric(metric_type, latency_ms, tags).await;
            }
        });
    }
}

/// 性能监控器
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// 指标数据存储
    metrics: Arc<RwLock<HashMap<MetricType, Vec<MetricDataPoint>>>>,
    /// 性能统计缓存
    stats_cache: Arc<RwLock<HashMap<MetricType, PerformanceStats>>>,
    /// 监控配置
    config: MonitorConfig,
    /// 告警阈值
    alert_thresholds: Arc<RwLock<HashMap<MetricType, AlertThreshold>>>,
}

/// 监控配置
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// 数据保留时间
    pub retention_duration: Duration,
    /// 统计计算间隔
    pub stats_interval: Duration,
    /// 最大数据点数量
    pub max_data_points: usize,
    /// 是否启用告警
    pub enable_alerts: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            retention_duration: Duration::from_secs(3600), // 1小时
            stats_interval: Duration::from_secs(60),       // 1分钟
            max_data_points: 10000,
            enable_alerts: true,
        }
    }
}

/// 告警阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThreshold {
    pub warning_threshold: f64,
    pub critical_threshold: f64,
    pub enabled: bool,
}

/// 告警级别
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AlertLevel {
    Warning,
    Critical,
}

/// 告警事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub metric_type: MetricType,
    pub level: AlertLevel,
    pub value: f64,
    pub threshold: f64,
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new() -> Self {
        Self::with_config(MonitorConfig::default())
    }
    
    /// 使用指定配置创建监控器
    pub fn with_config(config: MonitorConfig) -> Self {
        let monitor = Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            stats_cache: Arc::new(RwLock::new(HashMap::new())),
            config: config.clone(),
            alert_thresholds: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // 启动定期统计计算任务
        monitor.start_stats_calculation_task();
        
        // 启动数据清理任务
        monitor.start_cleanup_task();
        
        monitor
    }
    
    /// 记录指标
    pub async fn record_metric(
        &self,
        metric_type: MetricType,
        value: f64,
        tags: HashMap<String, String>,
    ) {
        let data_point = MetricDataPoint {
            timestamp: Utc::now(),
            value,
            tags,
        };
        
        {
            let mut metrics = self.metrics.write().await;
            let entry = metrics.entry(metric_type.clone()).or_insert_with(Vec::new);
            entry.push(data_point);
            
            // 限制数据点数量
            if entry.len() > self.config.max_data_points {
                entry.remove(0);
            }
        }
        
        // 检查告警
        if self.config.enable_alerts {
            self.check_alert(metric_type, value).await;
        }
    }
    
    /// 开始延迟测量
    pub fn start_latency_measurement(&self, metric_type: MetricType) -> LatencyMeasurer {
        LatencyMeasurer::new(metric_type)
    }
    
    /// 记录计数器指标
    pub async fn increment_counter(&self, metric_type: MetricType, tags: HashMap<String, String>) {
        self.record_metric(metric_type, 1.0, tags).await;
    }
    
    /// 记录仪表盘指标
    pub async fn record_gauge(&self, metric_type: MetricType, value: f64, tags: HashMap<String, String>) {
        self.record_metric(metric_type, value, tags).await;
    }
    
    /// 获取性能统计
    pub async fn get_stats(&self, metric_type: &MetricType) -> Option<PerformanceStats> {
        self.stats_cache.read().await.get(metric_type).cloned()
    }
    
    /// 获取所有统计信息
    pub async fn get_all_stats(&self) -> HashMap<MetricType, PerformanceStats> {
        self.stats_cache.read().await.clone()
    }
    
    /// 获取最近的指标数据
    pub async fn get_recent_metrics(
        &self,
        metric_type: &MetricType,
        duration: Duration,
    ) -> Vec<MetricDataPoint> {
        let metrics = self.metrics.read().await;
        
        if let Some(data_points) = metrics.get(metric_type) {
            let cutoff_time = Utc::now() - chrono::Duration::from_std(duration).unwrap();
            
            data_points
                .iter()
                .filter(|point| point.timestamp > cutoff_time)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// 设置告警阈值
    pub async fn set_alert_threshold(&self, metric_type: MetricType, threshold: AlertThreshold) {
        let mut thresholds = self.alert_thresholds.write().await;
        thresholds.insert(metric_type, threshold);
    }
    
    /// 检查告警
    async fn check_alert(&self, metric_type: MetricType, value: f64) {
        let thresholds = self.alert_thresholds.read().await;
        
        if let Some(threshold) = thresholds.get(&metric_type) {
            if !threshold.enabled {
                return;
            }
            
            let alert_event = if value >= threshold.critical_threshold {
                Some(AlertEvent {
                    metric_type: metric_type.clone(),
                    level: AlertLevel::Critical,
                    value,
                    threshold: threshold.critical_threshold,
                    timestamp: Utc::now(),
                    message: format!(
                        "Critical alert: {:?} value {} exceeds critical threshold {}",
                        metric_type, value, threshold.critical_threshold
                    ),
                })
            } else if value >= threshold.warning_threshold {
                Some(AlertEvent {
                    metric_type: metric_type.clone(),
                    level: AlertLevel::Warning,
                    value,
                    threshold: threshold.warning_threshold,
                    timestamp: Utc::now(),
                    message: format!(
                        "Warning alert: {:?} value {} exceeds warning threshold {}",
                        metric_type, value, threshold.warning_threshold
                    ),
                })
            } else {
                None
            };
            
            if let Some(alert) = alert_event {
                self.handle_alert(alert).await;
            }
        }
    }
    
    /// 处理告警事件
    async fn handle_alert(&self, alert: AlertEvent) {
        match alert.level {
            AlertLevel::Warning => {
                warn!("Performance Warning: {}", alert.message);
            }
            AlertLevel::Critical => {
                error!("Performance Critical: {}", alert.message);
            }
        }
        
        // 这里可以添加更多告警处理逻辑，如发送通知等
    }
    
    /// 计算统计信息
    async fn calculate_stats(&self) {
        let metrics = self.metrics.read().await;
        let mut stats_cache = self.stats_cache.write().await;
        
        for (metric_type, data_points) in metrics.iter() {
            if !data_points.is_empty() {
                let values: Vec<f64> = data_points.iter().map(|p| p.value).collect();
                let mut stats = PerformanceStats::new();
                stats.update(&values);
                stats_cache.insert(metric_type.clone(), stats);
            }
        }
    }
    
    /// 清理过期数据
    async fn cleanup_expired_data(&self) {
        let mut metrics = self.metrics.write().await;
        let cutoff_time = Utc::now() - chrono::Duration::from_std(self.config.retention_duration).unwrap();
        
        for data_points in metrics.values_mut() {
            data_points.retain(|point| point.timestamp > cutoff_time);
        }
    }
    
    /// 启动统计计算任务
    fn start_stats_calculation_task(&self) {
        let monitor = self.clone();
        let interval = self.config.stats_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                monitor.calculate_stats().await;
            }
        });
    }
    
    /// 启动数据清理任务
    fn start_cleanup_task(&self) {
        let monitor = self.clone();
        let cleanup_interval = Duration::from_secs(300); // 5分钟清理一次
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(cleanup_interval);
            
            loop {
                interval_timer.tick().await;
                monitor.cleanup_expired_data().await;
            }
        });
    }
    
    /// 获取监控摘要
    pub async fn get_summary(&self) -> MonitorSummary {
        let all_stats = self.get_all_stats().await;
        let metrics = self.metrics.read().await;
        
        let total_data_points: usize = metrics.values().map(|v| v.len()).sum();
        
        MonitorSummary {
            total_metrics: metrics.len(),
            total_data_points,
            stats: all_stats,
            last_updated: Utc::now(),
        }
    }
}

/// 监控摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSummary {
    pub total_metrics: usize,
    pub total_data_points: usize,
    pub stats: HashMap<MetricType, PerformanceStats>,
    pub last_updated: DateTime<Utc>,
}

impl Clone for PerformanceMonitor {
    fn clone(&self) -> Self {
        Self {
            metrics: Arc::clone(&self.metrics),
            stats_cache: Arc::clone(&self.stats_cache),
            config: self.config.clone(),
            alert_thresholds: Arc::clone(&self.alert_thresholds),
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_metric_recording() {
        let monitor = PerformanceMonitor::new();
        
        // 记录一些指标
        monitor.record_metric(MetricType::ApiLatency, 100.0, HashMap::new()).await;
        monitor.record_metric(MetricType::ApiLatency, 150.0, HashMap::new()).await;
        monitor.record_metric(MetricType::ApiLatency, 200.0, HashMap::new()).await;
        
        // 等待统计计算
        sleep(Duration::from_millis(100)).await;
        
        let recent_metrics = monitor
            .get_recent_metrics(&MetricType::ApiLatency, Duration::from_secs(60))
            .await;
        
        assert_eq!(recent_metrics.len(), 3);
    }
    
    #[tokio::test]
    async fn test_latency_measurement() {
        let monitor = PerformanceMonitor::new();
        
        let measurer = monitor.start_latency_measurement(MetricType::OrderLatency);
        
        // 模拟一些工作
        sleep(Duration::from_millis(10)).await;
        
        measurer.finish(&monitor);
        
        // 等待异步记录完成
        sleep(Duration::from_millis(100)).await;
        
        let recent_metrics = monitor
            .get_recent_metrics(&MetricType::OrderLatency, Duration::from_secs(60))
            .await;
        
        assert_eq!(recent_metrics.len(), 1);
        assert!(recent_metrics[0].value >= 10.0); // 至少10ms
    }
    
    #[tokio::test]
    async fn test_alert_threshold() {
        let monitor = PerformanceMonitor::new();
        
        // 设置告警阈值
        let threshold = AlertThreshold {
            warning_threshold: 100.0,
            critical_threshold: 200.0,
            enabled: true,
        };
        
        monitor.set_alert_threshold(MetricType::ApiLatency, threshold).await;
        
        // 记录超过阈值的指标
        monitor.record_metric(MetricType::ApiLatency, 250.0, HashMap::new()).await;
        
        // 这里应该触发告警，但在测试中我们只验证不会panic
    }
}