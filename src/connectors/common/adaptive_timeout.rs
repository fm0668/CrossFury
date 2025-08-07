//! 自适应超时管理器
//! 实现WebSocket优化重构方案中的智能超时处理功能

use std::time::{Duration, SystemTime};
use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::RwLock;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

/// 自适应超时管理器
/// 根据网络质量动态调整超时时间
#[derive(Debug, Clone)]
pub struct AdaptiveTimeoutManager {
    /// 配置参数
    config: AdaptiveTimeoutConfig,
    /// 当前状态
    state: Arc<RwLock<AdaptiveTimeoutState>>,
}

/// 自适应超时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveTimeoutConfig {
    /// 基础超时时间（毫秒）
    pub base_timeout_ms: u64,
    /// 最小超时时间（毫秒）
    pub min_timeout_ms: u64,
    /// 最大超时时间（毫秒）
    pub max_timeout_ms: u64,
    /// 网络质量采样窗口大小
    pub sample_window_size: usize,
    /// 调整因子（0.0-1.0）
    pub adjustment_factor: f64,
    /// 是否启用自适应调整
    pub enabled: bool,
    /// 质量评估间隔（毫秒）
    pub evaluation_interval_ms: u64,
}

/// 自适应超时状态
#[derive(Debug)]
struct AdaptiveTimeoutState {
    /// 当前超时时间
    current_timeout: Duration,
    /// 延迟历史记录
    latency_history: VecDeque<Duration>,
    /// 丢包率历史记录
    packet_loss_history: VecDeque<f64>,
    /// 最后一次评估时间
    last_evaluation: Option<SystemTime>,
    /// 网络质量分数（0.0-1.0）
    network_quality_score: f64,
    /// 连续超时次数
    consecutive_timeouts: u32,
    /// 连续成功次数
    consecutive_successes: u32,
}

/// 网络质量指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkQualityMetrics {
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// 延迟标准差
    pub latency_std_dev: f64,
    /// 丢包率（0.0-1.0）
    pub packet_loss_rate: f64,
    /// 质量分数（0.0-1.0）
    pub quality_score: f64,
    /// 建议超时时间（毫秒）
    pub recommended_timeout_ms: u64,
}

/// 超时调整结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeoutAdjustmentResult {
    /// 增加超时时间
    Increased(Duration),
    /// 减少超时时间
    Decreased(Duration),
    /// 保持不变
    Unchanged(Duration),
    /// 达到最大值
    MaxReached(Duration),
    /// 达到最小值
    MinReached(Duration),
}

impl Default for AdaptiveTimeoutConfig {
    fn default() -> Self {
        Self {
            base_timeout_ms: 5000,      // 5秒
            min_timeout_ms: 1000,       // 1秒
            max_timeout_ms: 30000,      // 30秒
            sample_window_size: 50,     // 保留50个样本
            adjustment_factor: 0.2,     // 20%调整幅度
            enabled: true,
            evaluation_interval_ms: 10000, // 10秒评估一次
        }
    }
}

impl Default for AdaptiveTimeoutState {
    fn default() -> Self {
        Self {
            current_timeout: Duration::from_millis(5000),
            latency_history: VecDeque::new(),
            packet_loss_history: VecDeque::new(),
            last_evaluation: None,
            network_quality_score: 1.0, // 初始假设网络质量良好
            consecutive_timeouts: 0,
            consecutive_successes: 0,
        }
    }
}

impl AdaptiveTimeoutManager {
    /// 创建新的自适应超时管理器
    pub fn new(config: AdaptiveTimeoutConfig) -> Self {
        let initial_timeout = Duration::from_millis(config.base_timeout_ms);
        let mut state = AdaptiveTimeoutState::default();
        state.current_timeout = initial_timeout;
        
        Self {
            config,
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// 使用默认配置创建管理器
    pub fn with_default_config() -> Self {
        Self::new(AdaptiveTimeoutConfig::default())
    }

    /// 获取当前超时时间
    pub async fn get_current_timeout(&self) -> Duration {
        let state = self.state.read().await;
        state.current_timeout
    }

    /// 记录延迟样本
    pub async fn record_latency(&self, latency: Duration) {
        if !self.config.enabled {
            return;
        }

        let mut state = self.state.write().await;
        
        // 添加到历史记录
        state.latency_history.push_back(latency);
        
        // 保持窗口大小
        if state.latency_history.len() > self.config.sample_window_size {
            state.latency_history.pop_front();
        }
        
        // 记录成功
        state.consecutive_successes += 1;
        state.consecutive_timeouts = 0;
        
        debug!("[AdaptiveTimeout] 记录延迟: {}ms, 历史样本数: {}", 
               latency.as_millis(), state.latency_history.len());
    }

    /// 记录超时事件
    pub async fn record_timeout(&self) {
        if !self.config.enabled {
            return;
        }

        let mut state = self.state.write().await;
        
        state.consecutive_timeouts += 1;
        state.consecutive_successes = 0;
        
        // 立即调整超时时间（增加）
        let adjustment = (state.current_timeout.as_millis() as f64 * self.config.adjustment_factor) as u64;
        let new_timeout_ms = (state.current_timeout.as_millis() as u64 + adjustment)
            .min(self.config.max_timeout_ms);
        
        state.current_timeout = Duration::from_millis(new_timeout_ms);
        
        warn!("[AdaptiveTimeout] 记录超时事件，调整超时时间至: {}ms (连续超时: {}次)", 
              new_timeout_ms, state.consecutive_timeouts);
    }

    /// 记录丢包率
    pub async fn record_packet_loss(&self, loss_rate: f64) {
        if !self.config.enabled {
            return;
        }

        let mut state = self.state.write().await;
        
        // 添加到历史记录
        state.packet_loss_history.push_back(loss_rate.clamp(0.0, 1.0));
        
        // 保持窗口大小
        if state.packet_loss_history.len() > self.config.sample_window_size {
            state.packet_loss_history.pop_front();
        }
        
        debug!("[AdaptiveTimeout] 记录丢包率: {:.2}%", loss_rate * 100.0);
    }

    /// 评估网络质量并调整超时时间
    pub async fn evaluate_and_adjust(&self) -> Option<TimeoutAdjustmentResult> {
        if !self.config.enabled {
            return None;
        }

        let mut state = self.state.write().await;
        
        // 检查是否需要评估
        let now = SystemTime::now();
        if let Some(last_eval) = state.last_evaluation {
            let interval = Duration::from_millis(self.config.evaluation_interval_ms);
            if now.duration_since(last_eval).unwrap_or(Duration::from_secs(0)) < interval {
                return None;
            }
        }
        
        state.last_evaluation = Some(now);
        
        // 计算网络质量指标
        let metrics = self.calculate_network_metrics(&state);
        state.network_quality_score = metrics.quality_score;
        
        // 根据质量分数调整超时时间
        let old_timeout = state.current_timeout;
        let recommended_timeout = Duration::from_millis(metrics.recommended_timeout_ms);
        
        let result = if recommended_timeout > old_timeout {
            if recommended_timeout.as_millis() as u64 >= self.config.max_timeout_ms {
                state.current_timeout = Duration::from_millis(self.config.max_timeout_ms);
                TimeoutAdjustmentResult::MaxReached(state.current_timeout)
            } else {
                state.current_timeout = recommended_timeout;
                TimeoutAdjustmentResult::Increased(state.current_timeout)
            }
        } else if recommended_timeout < old_timeout {
            if recommended_timeout.as_millis() as u64 <= self.config.min_timeout_ms {
                state.current_timeout = Duration::from_millis(self.config.min_timeout_ms);
                TimeoutAdjustmentResult::MinReached(state.current_timeout)
            } else {
                state.current_timeout = recommended_timeout;
                TimeoutAdjustmentResult::Decreased(state.current_timeout)
            }
        } else {
            TimeoutAdjustmentResult::Unchanged(state.current_timeout)
        };
        
        if old_timeout != state.current_timeout {
            info!("[AdaptiveTimeout] 超时时间调整: {}ms -> {}ms (质量分数: {:.2})", 
                  old_timeout.as_millis(), state.current_timeout.as_millis(), metrics.quality_score);
        }
        
        Some(result)
    }

    /// 计算网络质量指标
    fn calculate_network_metrics(&self, state: &AdaptiveTimeoutState) -> NetworkQualityMetrics {
        let mut avg_latency = 0.0;
        let mut latency_std_dev = 0.0;
        let mut packet_loss_rate = 0.0;
        
        // 计算平均延迟
        if !state.latency_history.is_empty() {
            let sum: u128 = state.latency_history.iter().map(|d| d.as_millis()).sum();
            avg_latency = sum as f64 / state.latency_history.len() as f64;
            
            // 计算标准差
            let variance: f64 = state.latency_history.iter()
                .map(|d| {
                    let diff = d.as_millis() as f64 - avg_latency;
                    diff * diff
                })
                .sum::<f64>() / state.latency_history.len() as f64;
            latency_std_dev = variance.sqrt();
        }
        
        // 计算平均丢包率
        if !state.packet_loss_history.is_empty() {
            packet_loss_rate = state.packet_loss_history.iter().sum::<f64>() / state.packet_loss_history.len() as f64;
        }
        
        // 计算质量分数（0.0-1.0）
        let latency_score = if avg_latency > 0.0 {
            // 延迟越低分数越高，100ms以下为满分
            (100.0 / (avg_latency + 100.0)).min(1.0)
        } else {
            1.0
        };
        
        let stability_score = if latency_std_dev > 0.0 {
            // 稳定性越高分数越高，标准差50ms以下为满分
            (50.0 / (latency_std_dev + 50.0)).min(1.0)
        } else {
            1.0
        };
        
        let loss_score = 1.0 - packet_loss_rate; // 丢包率越低分数越高
        
        // 综合质量分数（加权平均）
        let quality_score = (latency_score * 0.4 + stability_score * 0.3 + loss_score * 0.3).clamp(0.0, 1.0);
        
        // 根据质量分数计算推荐超时时间
        let base_timeout = self.config.base_timeout_ms as f64;
        let quality_multiplier = if quality_score > 0.8 {
            0.8 // 高质量网络，减少超时时间
        } else if quality_score > 0.6 {
            1.0 // 中等质量网络，保持基础超时时间
        } else if quality_score > 0.4 {
            1.5 // 低质量网络，增加超时时间
        } else {
            2.0 // 极差网络，大幅增加超时时间
        };
        
        let recommended_timeout_ms = (base_timeout * quality_multiplier) as u64;
        let recommended_timeout_ms = recommended_timeout_ms
            .max(self.config.min_timeout_ms)
            .min(self.config.max_timeout_ms);
        
        NetworkQualityMetrics {
            avg_latency_ms: avg_latency,
            latency_std_dev,
            packet_loss_rate,
            quality_score,
            recommended_timeout_ms,
        }
    }

    /// 获取当前网络质量指标
    pub async fn get_network_metrics(&self) -> NetworkQualityMetrics {
        let state = self.state.read().await;
        self.calculate_network_metrics(&state)
    }

    /// 重置状态
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        let initial_timeout = Duration::from_millis(self.config.base_timeout_ms);
        *state = AdaptiveTimeoutState::default();
        state.current_timeout = initial_timeout;
        
        info!("[AdaptiveTimeout] 状态已重置，超时时间恢复至: {}ms", self.config.base_timeout_ms);
    }

    /// 强制设置超时时间
    pub async fn set_timeout(&self, timeout: Duration) {
        let mut state = self.state.write().await;
        let timeout_ms = timeout.as_millis() as u64;
        let clamped_timeout_ms = timeout_ms
            .max(self.config.min_timeout_ms)
            .min(self.config.max_timeout_ms);
        
        state.current_timeout = Duration::from_millis(clamped_timeout_ms);
        
        info!("[AdaptiveTimeout] 强制设置超时时间: {}ms", clamped_timeout_ms);
    }

    /// 获取状态摘要
    pub async fn get_status_summary(&self) -> AdaptiveTimeoutStatus {
        let state = self.state.read().await;
        let metrics = self.calculate_network_metrics(&state);
        
        AdaptiveTimeoutStatus {
            current_timeout_ms: state.current_timeout.as_millis() as u64,
            network_quality_score: state.network_quality_score,
            consecutive_timeouts: state.consecutive_timeouts,
            consecutive_successes: state.consecutive_successes,
            sample_count: state.latency_history.len(),
            metrics,
            config: self.config.clone(),
        }
    }

    /// 更新配置
    pub async fn update_config(&mut self, new_config: AdaptiveTimeoutConfig) {
        self.config = new_config;
        
        // 重新验证当前超时时间是否在新的范围内
        let mut state = self.state.write().await;
        let current_ms = state.current_timeout.as_millis() as u64;
        let adjusted_ms = current_ms
            .max(self.config.min_timeout_ms)
            .min(self.config.max_timeout_ms);
        
        if adjusted_ms != current_ms {
            state.current_timeout = Duration::from_millis(adjusted_ms);
            info!("[AdaptiveTimeout] 配置更新后调整超时时间: {}ms -> {}ms", current_ms, adjusted_ms);
        }
        
        info!("[AdaptiveTimeout] 配置已更新");
    }
}

/// 自适应超时状态摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveTimeoutStatus {
    pub current_timeout_ms: u64,
    pub network_quality_score: f64,
    pub consecutive_timeouts: u32,
    pub consecutive_successes: u32,
    pub sample_count: usize,
    pub metrics: NetworkQualityMetrics,
    pub config: AdaptiveTimeoutConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration as TokioDuration};

    #[tokio::test]
    async fn test_adaptive_timeout_manager_creation() {
        let manager = AdaptiveTimeoutManager::with_default_config();
        let timeout = manager.get_current_timeout().await;
        
        assert_eq!(timeout, Duration::from_millis(5000));
    }

    #[tokio::test]
    async fn test_record_latency() {
        let manager = AdaptiveTimeoutManager::with_default_config();
        
        // 记录一些延迟样本
        manager.record_latency(Duration::from_millis(50)).await;
        manager.record_latency(Duration::from_millis(100)).await;
        manager.record_latency(Duration::from_millis(75)).await;
        
        let metrics = manager.get_network_metrics().await;
        assert!((metrics.avg_latency_ms - 75.0).abs() < 1.0);
        assert!(metrics.quality_score > 0.5);
    }

    #[tokio::test]
    async fn test_record_timeout() {
        let manager = AdaptiveTimeoutManager::with_default_config();
        let initial_timeout = manager.get_current_timeout().await;
        
        // 记录超时事件
        manager.record_timeout().await;
        
        let new_timeout = manager.get_current_timeout().await;
        assert!(new_timeout > initial_timeout);
    }

    #[tokio::test]
    async fn test_network_quality_calculation() {
        let manager = AdaptiveTimeoutManager::with_default_config();
        
        // 记录高质量网络样本
        for _ in 0..10 {
            manager.record_latency(Duration::from_millis(20)).await;
        }
        manager.record_packet_loss(0.0).await;
        
        let metrics = manager.get_network_metrics().await;
        assert!(metrics.quality_score > 0.8);
        assert_eq!(metrics.avg_latency_ms, 20.0);
        assert_eq!(metrics.packet_loss_rate, 0.0);
    }

    #[tokio::test]
    async fn test_timeout_adjustment() {
        let config = AdaptiveTimeoutConfig {
            evaluation_interval_ms: 100, // 快速评估用于测试
            base_timeout_ms: 5000,
            max_timeout_ms: 15000, // 确保有足够的调整空间
            ..Default::default()
        };
        let manager = AdaptiveTimeoutManager::new(config);
        
        // 记录高延迟样本（2000ms延迟会导致更低的质量分数）
        for _ in 0..10 {
            manager.record_latency(Duration::from_millis(2000)).await;
        }
        
        // 记录一些丢包以进一步降低质量分数
        manager.record_packet_loss(0.1).await;
        
        // 等待评估间隔
        sleep(TokioDuration::from_millis(150)).await;
        
        let result = manager.evaluate_and_adjust().await;
        assert!(result.is_some());
        
        // 高延迟应该导致超时时间增加
        let timeout = manager.get_current_timeout().await;
        assert!(timeout > Duration::from_millis(5000));
        
        // 验证网络质量指标
        let metrics = manager.get_network_metrics().await;
        assert!(metrics.avg_latency_ms >= 2000.0);
        assert!(metrics.quality_score < 0.6); // 低质量网络
    }

    #[tokio::test]
    async fn test_reset() {
        let manager = AdaptiveTimeoutManager::with_default_config();
        
        // 记录一些数据
        manager.record_timeout().await;
        manager.record_latency(Duration::from_millis(1000)).await;
        
        // 重置
        manager.reset().await;
        
        let timeout = manager.get_current_timeout().await;
        assert_eq!(timeout, Duration::from_millis(5000));
        
        let status = manager.get_status_summary().await;
        assert_eq!(status.consecutive_timeouts, 0);
        assert_eq!(status.sample_count, 0);
    }
}