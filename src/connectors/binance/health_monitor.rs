//! 健康监控模块
//!
//! 提供连接健康监控、自动恢复和统计功能

use crate::connectors::common::smart_error_recovery::{
    RecoveryAction as CommonRecoveryAction, RecoveryResult, RecoveryStrategy,
};
use crate::types::{
    config::{ConnectionQuality, HealthStatus},
    errors::ConnectorError,
};
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{Mutex, RwLock},
    time::{interval, sleep, timeout},
};

/// 健康检查配置
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// 检查间隔（秒）
    pub check_interval_secs: u64,
    /// 数据超时阈值（秒）
    pub data_timeout_secs: u64,
    /// 最大连续失败次数
    pub max_consecutive_failures: u32,
    /// 自动恢复启用
    pub auto_recovery_enabled: bool,
    /// 健康检查超时（秒）
    pub health_check_timeout_secs: u64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 30,
            data_timeout_secs: 120,
            max_consecutive_failures: 3,
            auto_recovery_enabled: true,
            health_check_timeout_secs: 10,
        }
    }
}

/// 故障类型
#[derive(Debug, Clone, PartialEq)]
pub enum FailureType {
    /// 连接失败
    ConnectionFailure,
    /// 数据流中断
    DataStreamInterruption,
    /// 认证失败
    AuthenticationFailure,
    /// 网络超时
    NetworkTimeout,
    /// 服务不可用
    ServiceUnavailable,
    /// 未知错误
    Unknown,
}

/// 健康检查结果
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// 检查时间
    pub timestamp: DateTime<Utc>,
    /// 健康状态
    pub status: HealthStatus,
    /// 连接质量
    pub connection_quality: Option<ConnectionQuality>,
    /// 故障类型（如果有）
    pub failure_type: Option<FailureType>,
    /// 详细信息
    pub details: String,
    /// 检查耗时
    pub check_duration: Duration,
}

/// 恢复操作类型
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryAction {
    /// 重新连接
    Reconnect,
    /// 重置连接
    Reset,
    /// 紧急ping
    EmergencyPing,
    /// 等待恢复
    WaitForRecovery,
    /// 无操作
    None,
}

/// 健康统计信息
#[derive(Debug, Clone)]
pub struct HealthStatistics {
    /// 总检查次数
    pub total_checks: u64,
    /// 健康检查次数
    pub healthy_checks: u64,
    /// 警告检查次数
    pub warning_checks: u64,
    /// 不健康检查次数
    pub unhealthy_checks: u64,
    /// 严重检查次数
    pub critical_checks: u64,
    /// 总恢复次数
    pub total_recoveries: u64,
    /// 成功恢复次数
    pub successful_recoveries: u64,
    /// 平均检查耗时
    pub average_check_duration: Duration,
    /// 最后检查时间
    pub last_check_time: Option<DateTime<Utc>>,
    /// 连续健康次数
    pub consecutive_healthy: u32,
    /// 连续失败次数
    pub consecutive_failures: u32,
}

impl Default for HealthStatistics {
    fn default() -> Self {
        Self {
            total_checks: 0,
            healthy_checks: 0,
            warning_checks: 0,
            unhealthy_checks: 0,
            critical_checks: 0,
            total_recoveries: 0,
            successful_recoveries: 0,
            average_check_duration: Duration::from_millis(0),
            last_check_time: None,
            consecutive_healthy: 0,
            consecutive_failures: 0,
        }
    }
}

/// 健康监控器
pub struct HealthMonitor {
    /// 配置
    config: HealthCheckConfig,
    /// 当前健康状态
    current_status: Arc<RwLock<HealthStatus>>,
    /// 统计信息
    statistics: Arc<RwLock<HealthStatistics>>,
    /// 最近的检查结果
    recent_results: Arc<RwLock<Vec<HealthCheckResult>>>,
    /// 恢复历史
    recovery_history: Arc<RwLock<Vec<RecoveryResult>>>,
    /// 最后数据接收时间
    last_data_received: Arc<RwLock<Option<Instant>>>,
    /// 监控任务句柄
    monitor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 是否正在运行
    is_running: Arc<RwLock<bool>>,
}

impl HealthMonitor {
    /// 创建新的健康监控器
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            current_status: Arc::new(RwLock::new(HealthStatus::Healthy)),
            statistics: Arc::new(RwLock::new(HealthStatistics::default())),
            recent_results: Arc::new(RwLock::new(Vec::new())),
            recovery_history: Arc::new(RwLock::new(Vec::new())),
            last_data_received: Arc::new(RwLock::new(None)),
            monitor_handle: Arc::new(Mutex::new(None)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 使用默认配置创建健康监控器
    pub fn new_default() -> Self {
        Self::new(HealthCheckConfig::default())
    }

    /// 启动健康监控
    pub async fn start_monitoring<F, Fut>(&self, health_checker: F) -> Result<(), ConnectorError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ConnectionQuality, ConnectorError>> + Send + 'static,
    {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("[HealthMonitor] 健康监控已在运行");
            return Ok(());
        }

        info!(
            "[HealthMonitor] 启动健康监控，检查间隔: {}秒",
            self.config.check_interval_secs
        );

        let config = self.config.clone();
        let current_status = self.current_status.clone();
        let statistics = self.statistics.clone();
        let recent_results = self.recent_results.clone();
        let recovery_history = self.recovery_history.clone();
        let last_data_received = self.last_data_received.clone();
        let is_running_clone = self.is_running.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.check_interval_secs));

            loop {
                interval.tick().await;

                // 检查是否应该停止
                if !*is_running_clone.read().await {
                    break;
                }

                let start_time = Instant::now();

                // 执行健康检查
                let check_result = Self::perform_detailed_health_check(
                    &health_checker,
                    &config,
                    &last_data_received,
                    start_time,
                )
                .await;

                // 更新状态和统计
                Self::update_status_and_statistics(
                    &current_status,
                    &statistics,
                    &recent_results,
                    check_result.clone(),
                )
                .await;

                // 检查是否需要恢复
                if config.auto_recovery_enabled {
                    if let Some(recovery_action) =
                        Self::determine_recovery_action(&check_result, &config).await
                    {
                        let recovery_result = Self::execute_recovery_action(recovery_action).await;

                        // 记录恢复结果
                        let mut history = recovery_history.write().await;
                        history.push(recovery_result.clone());

                        // 保持历史记录在合理范围内
                        if history.len() > 100 {
                            history.remove(0);
                        }

                        // 更新恢复统计
                        let mut stats = statistics.write().await;
                        stats.total_recoveries += 1;
                        if recovery_result.success {
                            stats.successful_recoveries += 1;
                        }
                    }
                }

                debug!("[HealthMonitor] 健康检查完成: {:?}", check_result.status);
            }

            info!("[HealthMonitor] 健康监控已停止");
        });

        *self.monitor_handle.lock().await = Some(handle);
        *is_running = true;

        Ok(())
    }

    /// 停止健康监控
    pub async fn stop_monitoring(&self) {
        info!("[HealthMonitor] 停止健康监控...");

        {
            let mut is_running = self.is_running.write().await;
            *is_running = false;
        }

        if let Some(handle) = self.monitor_handle.lock().await.take() {
            handle.abort();
        }
    }

    /// 记录数据接收
    pub async fn record_data_received(&self) {
        let mut last_received = self.last_data_received.write().await;
        *last_received = Some(Instant::now());
    }

    /// 获取当前健康状态
    pub async fn get_current_status(&self) -> HealthStatus {
        let status = self.current_status.read().await;
        status.clone()
    }

    /// 获取健康状态（别名方法）
    pub async fn get_health_status(&self) -> HealthStatus {
        self.get_current_status().await
    }

    /// 记录错误
    pub async fn record_error(&mut self, error_message: String) {
        warn!("[HealthMonitor] 记录错误: {error_message}");
        let mut stats = self.statistics.write().await;
        stats.consecutive_failures += 1;
        stats.consecutive_healthy = 0;
    }

    /// 记录成功操作
    pub async fn record_success(&mut self) {
        debug!("[HealthMonitor] 记录成功操作");
        let mut stats = self.statistics.write().await;
        stats.consecutive_healthy += 1;
        stats.consecutive_failures = 0;
    }

    /// 执行简单健康检查
    pub async fn simple_health_check(&mut self) -> HealthStatus {
        let stats = self.statistics.read().await;

        if stats.consecutive_failures >= self.config.max_consecutive_failures {
            HealthStatus::Unhealthy
        } else if stats.consecutive_failures > 0 {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Healthy
        }
    }

    /// 获取恢复操作
    pub async fn get_recovery_action(&self) -> Option<RecoveryAction> {
        let status = self.get_current_status().await;
        let stats = self.statistics.read().await;

        match status {
            HealthStatus::Unhealthy => {
                if stats.consecutive_failures >= self.config.max_consecutive_failures {
                    Some(RecoveryAction::Reconnect)
                } else {
                    Some(RecoveryAction::Reset)
                }
            }
            HealthStatus::Degraded => Some(RecoveryAction::EmergencyPing),
            HealthStatus::Healthy => None,
        }
    }

    /// 获取健康统计信息
    pub async fn get_statistics(&self) -> HealthStatistics {
        let stats = self.statistics.read().await;
        stats.clone()
    }

    /// 执行详细健康检查
    async fn perform_detailed_health_check<F, Fut>(
        health_checker: &F,
        config: &HealthCheckConfig,
        last_data_received: &Arc<RwLock<Option<Instant>>>,
        start_time: Instant,
    ) -> HealthCheckResult
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<ConnectionQuality, ConnectorError>>,
    {
        let check_duration = start_time.elapsed();

        // 检查数据流是否中断
        let data_stream_ok = {
            let last_received = last_data_received.read().await;
            match *last_received {
                Some(last_time) => {
                    let elapsed = last_time.elapsed();
                    elapsed <= Duration::from_secs(config.data_timeout_secs)
                }
                None => false, // 从未接收到数据
            }
        };

        // 执行健康检查，带超时
        let timeout_duration = Duration::from_secs(config.health_check_timeout_secs);
        match timeout(timeout_duration, health_checker()).await {
            Ok(Ok(quality)) => {
                let status = if !data_stream_ok {
                    HealthStatus::Degraded // 连接正常但数据流中断
                } else {
                    if quality.is_poor() {
                         HealthStatus::Unhealthy
                     } else if quality.latency_ms > 200.0 {
                         HealthStatus::Degraded
                     } else {
                         HealthStatus::Healthy
                     }
                };

                HealthCheckResult {
                    timestamp: Utc::now(),
                    status,
                    connection_quality: Some(quality.clone()),
                    failure_type: if data_stream_ok { None } else { Some(FailureType::DataStreamInterruption) },
                    details: if data_stream_ok {
                        format!("健康检查成功，连接质量: {:?}", quality)
                    } else {
                        "连接正常但数据流中断".to_string()
                    },
                    check_duration,
                }
            }
            Ok(Err(error)) => {
                let (status, failure_type) = match &error {
                    ConnectorError::NetworkError(_) => (HealthStatus::Unhealthy, FailureType::NetworkTimeout),
                    ConnectorError::AuthenticationFailed(_) => (HealthStatus::Unhealthy, FailureType::AuthenticationFailure),
                    ConnectorError::RateLimitExceeded(_) => (HealthStatus::Degraded, FailureType::ServiceUnavailable),
                    ConnectorError::ConnectionError(_) => (HealthStatus::Unhealthy, FailureType::ConnectionFailure),
                    _ => (HealthStatus::Unhealthy, FailureType::Unknown),
                };

                HealthCheckResult {
                    timestamp: Utc::now(),
                    status,
                    connection_quality: None,
                    failure_type: Some(failure_type),
                    details: format!("健康检查失败: {error}"),
                    check_duration,
                }
            }
            Err(_) => {
                // 超时
                HealthCheckResult {
                    timestamp: Utc::now(),
                    status: HealthStatus::Unhealthy,
                    connection_quality: None,
                    failure_type: Some(FailureType::NetworkTimeout),
                    details: "健康检查超时".to_string(),
                    check_duration,
                }
            }
        }
    }

    /// 更新状态和统计信息
    async fn update_status_and_statistics(
        current_status: &Arc<RwLock<HealthStatus>>,
        statistics: &Arc<RwLock<HealthStatistics>>,
        recent_results: &Arc<RwLock<Vec<HealthCheckResult>>>,
        result: HealthCheckResult,
    ) {
        // 更新当前状态
        {
            let mut status = current_status.write().await;
            *status = result.status.clone();
        }

        // 更新统计信息
        {
            let mut stats = statistics.write().await;
            stats.total_checks += 1;
            stats.last_check_time = Some(result.timestamp);

            match result.status {
                HealthStatus::Healthy => {
                    stats.healthy_checks += 1;
                    stats.consecutive_healthy += 1;
                    stats.consecutive_failures = 0;
                }
                HealthStatus::Degraded => {
                    stats.warning_checks += 1;
                    stats.consecutive_healthy = 0;
                    stats.consecutive_failures += 1;
                }
                HealthStatus::Unhealthy => {
                    stats.unhealthy_checks += 1;
                    stats.consecutive_healthy = 0;
                    stats.consecutive_failures += 1;
                }
            }

            let total_duration = stats.average_check_duration.as_millis() as u64
                * (stats.total_checks - 1)
                + result.check_duration.as_millis() as u64;
            stats.average_check_duration =
                Duration::from_millis(total_duration / stats.total_checks);
        }

        // 保存检查结果
        {
            let mut results = recent_results.write().await;
            results.push(result);

            // 保持最近100个结果
            if results.len() > 100 {
                results.remove(0);
            }
        }
    }

    /// 确定恢复操作
    async fn determine_recovery_action(
        result: &HealthCheckResult,
        _config: &HealthCheckConfig,
    ) -> Option<RecoveryAction> {
        match result.status {
            HealthStatus::Healthy => None,
            HealthStatus::Degraded => {
                // 降级状态下，等待观察
                Some(RecoveryAction::WaitForRecovery)
            }
            HealthStatus::Unhealthy => match result.failure_type {
                Some(FailureType::ConnectionFailure) => Some(RecoveryAction::Reconnect),
                Some(FailureType::DataStreamInterruption) => Some(RecoveryAction::Reset),
                Some(FailureType::NetworkTimeout) => Some(RecoveryAction::Reconnect),
                Some(FailureType::AuthenticationFailure) => Some(RecoveryAction::Reconnect),
                Some(FailureType::ServiceUnavailable) => Some(RecoveryAction::WaitForRecovery),
                Some(FailureType::Unknown) => Some(RecoveryAction::Reset),
                None => Some(RecoveryAction::Reset),
            },
        }
    }

    /// 执行恢复操作
    async fn execute_recovery_action(action: RecoveryAction) -> RecoveryResult {
        let start_time = Instant::now();

        match action {
            RecoveryAction::Reconnect => {
                info!("[HealthMonitor] 执行恢复操作: 重新连接");
                // 这里应该调用实际的重连逻辑
                // 目前返回模拟结果
                RecoveryResult {
                    success: true,
                    strategy: RecoveryStrategy::Reconnect,
                    actions: vec![CommonRecoveryAction::CloseConnection, CommonRecoveryAction::EstablishConnection],
                    recovery_time_ms: start_time.elapsed().as_millis() as u64,
                    retry_count: 1,
                    message: "重新连接操作已触发".to_string(),
                }
            }
            RecoveryAction::Reset => {
                info!("[HealthMonitor] 执行恢复操作: 重置连接");
                RecoveryResult {
                    success: true,
                    strategy: RecoveryStrategy::ClearCache,
                    actions: vec![CommonRecoveryAction::ClearState],
                    recovery_time_ms: start_time.elapsed().as_millis() as u64,
                    retry_count: 1,
                    message: "连接重置操作已触发".to_string(),
                }
            }
            RecoveryAction::EmergencyPing => {
                info!("[HealthMonitor] 执行恢复操作: 紧急ping");
                RecoveryResult {
                    success: true,
                    strategy: RecoveryStrategy::ImmediateRetry,
                    actions: vec![CommonRecoveryAction::SendHeartbeat],
                    recovery_time_ms: start_time.elapsed().as_millis() as u64,
                    retry_count: 1,
                    message: "紧急ping操作已触发".to_string(),
                }
            }
            RecoveryAction::WaitForRecovery => {
                debug!("[HealthMonitor] 执行恢复操作: 等待恢复");
                sleep(Duration::from_secs(5)).await;
                RecoveryResult {
                    success: true,
                    strategy: RecoveryStrategy::WaitAndRecover,
                    actions: vec![CommonRecoveryAction::Wait(Duration::from_secs(5))],
                    recovery_time_ms: start_time.elapsed().as_millis() as u64,
                    retry_count: 0,
                    message: "等待恢复完成".to_string(),
                }
            }
            RecoveryAction::None => RecoveryResult {
                success: true,
                strategy: RecoveryStrategy::WaitAndRecover,
                actions: vec![],
                recovery_time_ms: start_time.elapsed().as_millis() as u64,
                retry_count: 0,
                message: "无需恢复操作".to_string(),
            },
        }
    }

    /// 重置统计信息
    pub async fn reset_statistics(&self) {
        let mut stats = self.statistics.write().await;
        *stats = HealthStatistics::default();

        let mut results = self.recent_results.write().await;
        results.clear();

        let mut history = self.recovery_history.write().await;
        history.clear();

        info!("[HealthMonitor] 统计信息已重置");
    }

    /// 更新配置
    pub async fn update_config(&mut self, new_config: HealthCheckConfig) {
        self.config = new_config;
        info!("[HealthMonitor] 配置已更新");
    }

    /// 获取配置
    pub fn get_config(&self) -> &HealthCheckConfig {
        &self.config
    }

    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

/// 健康监控器的Drop实现，确保资源清理
impl Drop for HealthMonitor {
    fn drop(&mut self) {
        // 注意：这里不能使用async，所以只能记录日志
        info!("[HealthMonitor] 健康监控器正在销毁");
    }
}
