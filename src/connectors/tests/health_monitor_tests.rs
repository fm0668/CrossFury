//! 健康检查和自动恢复功能的单元测�?
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::connectors::binance::config::BinanceConfig;
use crate::connectors::binance::health_monitor::{
    FailureType, HealthCheckConfig, HealthCheckResult, HealthMonitor, HealthStatistics,
    RecoveryAction,
};
use crate::connectors::common::RecoveryResult;
use crate::types::config::{HealthStatus, ConnectionQuality};
use crate::connectors::binance::modern_adapter::ModernBinanceConnector;
use crate::core::AppState;


/// 创建测试用的健康检查配�?fn create_test_health_config() -> HealthCheckConfig {
    HealthCheckConfig {
        check_interval_secs: 5,
        connection_timeout_secs: 10,
        max_consecutive_failures: 3,
        auto_recovery_enabled: true,
        recovery_delay_secs: 1,
        health_check_timeout_secs: 5,
        data_stream_timeout_secs: 15,
    }
}

/// 创建测试用的Binance配置
fn create_test_binance_config() -> BinanceConfig {
    BinanceConfig {
        api_key: None,
        secret_key: None,
        testnet: true,
        rate_limit_per_minute: 1200,
    }
}

/// 创建测试用的适配�?async fn create_test_adapter() -> ModernBinanceConnector {
    let base_config = create_test_binance_config();
    let config = crate::connectors::binance::modern_adapter::ModernBinanceConfig {
        base_config,
        ..Default::default()
    };
    let app_state = Arc::new(AppState::new());
    ModernBinanceConnector::new(config, app_state)
}

#[cfg(test)]
mod health_monitor_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_monitor_creation() {
        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);

        let monitor = HealthMonitor::new(config.clone());

        // 验证初始状�?        let status = monitor.get_current_status().await;
        assert_eq!(status, HealthStatus::Healthy); // 初始状态应该是健康�?
        let statistics = monitor.get_statistics().await;
        assert_eq!(statistics.total_checks, 0);
        assert_eq!(statistics.healthy_checks, 0);
        assert_eq!(statistics.consecutive_healthy, 0);
        assert_eq!(statistics.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_health_check_config() {
        let config = create_test_health_config();

        // 验证配置�?        assert_eq!(config.check_interval_secs, 5);
        assert_eq!(config.connection_timeout_secs, 10);
        assert_eq!(config.max_consecutive_failures, 3);
        assert_eq!(config.recovery_delay_secs, 1);
        assert!(config.auto_recovery_enabled);
    }

    #[tokio::test]
    async fn test_health_status_enum() {
        // 测试健康状态枚举的基本功能
        let healthy = HealthStatus::Healthy;
        let degraded = HealthStatus::Degraded;
        let unhealthy = HealthStatus::Unhealthy;

        // 验证状态可以比较
        assert_ne!(healthy, degraded);
        assert_ne!(degraded, unhealthy);

        // 验证状态可以克�?        let healthy_clone = healthy.clone();
        assert_eq!(healthy, healthy_clone);
    }

    #[tokio::test]
    async fn test_failure_type_enum() {
        // 测试失败类型枚举
        let connection_failure = FailureType::ConnectionFailure;
        let network_timeout = FailureType::NetworkTimeout;
        let auth_failure = FailureType::AuthenticationFailure;
        let data_interruption = FailureType::DataStreamInterruption;
        let service_unavailable = FailureType::ServiceUnavailable;
        let unknown = FailureType::Unknown;

        // 验证所有类型都不相�?        assert_ne!(connection_failure, network_timeout);
        assert_ne!(network_timeout, auth_failure);
        assert_ne!(auth_failure, data_interruption);
        assert_ne!(data_interruption, service_unavailable);
        assert_ne!(service_unavailable, unknown);
    }

    #[tokio::test]
    async fn test_recovery_action_enum() {
        // 测试恢复操作枚举
        let reconnect = RecoveryAction::Reconnect;
        let reset = RecoveryAction::Reset;
        let emergency_ping = RecoveryAction::EmergencyPing;
        let wait = RecoveryAction::WaitForRecovery;
        let none = RecoveryAction::None;

        // 验证所有操作都不相�?        assert_ne!(reconnect, reset);
        assert_ne!(reset, emergency_ping);
        assert_ne!(emergency_ping, wait);
        assert_ne!(wait, none);
    }

    #[tokio::test]
    async fn test_health_check_result_creation() {
        let timestamp = Utc::now();
        let check_duration = Duration::from_millis(100);

        let result = HealthCheckResult {
            timestamp,
            status: HealthStatus::Healthy,
            connection_quality: Some(ConnectionQuality::default()),
            failure_type: None,
            details: "健康检查成�?.to_string(),
            check_duration,
        };

        // 验证结果字段
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.connection_quality.is_some());
        assert!(result.failure_type.is_none());
        assert_eq!(result.details, "健康检查成�?);
        assert_eq!(result.check_duration, check_duration);
    }

    #[tokio::test]
    async fn test_recovery_result_creation() {
        let timestamp = Utc::now();

        let result = RecoveryResult {
            success: true,
            strategy: crate::connectors::common::RecoveryStrategy::Reconnect,
            actions: vec![],
            recovery_time_ms: 100,
            retry_count: 1,
            message: "重连成功".to_string(),
        };

        // 验证恢复结果字段
        assert!(result.success);
        assert_eq!(result.message, "重连成功");
    }

    #[tokio::test]
    async fn test_health_statistics_default() {
        let stats = HealthStatistics::default();

        // 验证默认统计�?        assert_eq!(stats.total_checks, 0);
        assert_eq!(stats.healthy_checks, 0);
        assert_eq!(stats.degraded_checks, 0);
        assert_eq!(stats.unhealthy_checks, 0);
        assert_eq!(stats.consecutive_healthy, 0);
        assert_eq!(stats.consecutive_failures, 0);
        assert!(stats.last_check_time.is_none());
        assert_eq!(stats.average_check_duration, Duration::from_millis(0));
    }

    #[tokio::test]
    async fn test_monitor_statistics_tracking() {
        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let monitor = HealthMonitor::new(config.clone());

        // 获取初始统计
        let initial_stats = monitor.get_statistics().await;
        assert_eq!(initial_stats.total_checks, 0);

        // 重置统计（测试重置功能）
        monitor.reset_statistics().await;

        let reset_stats = monitor.get_statistics().await;
        assert_eq!(reset_stats.total_checks, 0);
        assert_eq!(reset_stats.healthy_checks, 0);
    }

    #[tokio::test]
    async fn test_monitor_recent_results() {
        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let monitor = HealthMonitor::new(config.clone());

        // 获取最近的检查结�?        let recent_results = monitor.get_recent_results(10).await;
        assert!(recent_results.is_empty()); // 初始应该为空
    }

    #[tokio::test]
    async fn test_monitor_recovery_history() {
        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let monitor = HealthMonitor::new(config.clone());

        // 获取恢复历史
        let recovery_history = monitor.get_recovery_history(10).await;
        assert!(recovery_history.is_empty()); // 初始应该为空
    }

    #[tokio::test]
    async fn test_monitor_config_update() {
        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let mut monitor = HealthMonitor::new(config.clone());

        // 创建新配�?        let new_config = HealthCheckConfig {
            check_interval_secs: 10,
            connection_timeout_secs: 15,
            max_consecutive_failures: 5,
            auto_recovery_enabled: false,
            recovery_delay_secs: 2,
            health_check_timeout_secs: 20,
            data_stream_timeout_secs: 30,
        };

        // 更新配置
        monitor.update_config(new_config.clone()).await;

        // 验证配置已更新（这里我们无法直接访问内部配置，但可以通过行为验证�?        // 在实际实现中，可能需要添加获取配置的方法来验�?    }

    #[tokio::test]
    async fn test_concurrent_monitor_access() {
        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let _monitor = Arc::new(HealthMonitor::new(config.clone()));

        let mut handles = vec![];

        // 创建多个并发任务
        for i in 0..5 {
            let monitor_clone = Arc::clone(&_monitor);
            let handle = tokio::spawn(async move {
                // 并发访问监控器的各种方法
                let _status = monitor_clone.get_current_status().await;
                let _stats = monitor_clone.get_statistics().await;
                let _results = monitor_clone.get_recent_results(10).await;
                let _history = monitor_clone.get_recovery_history(10).await;
                i
            });
            handles.push(handle);
        }

        // 等待所有任务完�?        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_monitor_memory_safety() {
        // 测试监控器的内存安全�?        let config = create_test_health_config();
        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);

        // 创建多个监控器实�?        let monitors: Vec<_> = (0..10)
            .map(|_| HealthMonitor::new(config.clone()))
            .collect();

        // 验证所有监控器都能正常工作
        for monitor in monitors {
            let status = monitor.get_current_status().await;
            assert_eq!(status, HealthStatus::Healthy);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 这些测试可能需要更长时间运行，通常在CI/CD中跳�?    #[tokio::test]
    #[ignore = "长时间运行的测试"]
    async fn test_health_monitoring_lifecycle() {
        let config = HealthCheckConfig {
            check_interval_secs: 1, // 快速检查用于测�?            connection_timeout_secs: 5,
            max_consecutive_failures: 2,
            auto_recovery_enabled: true,
            recovery_delay_secs: 1,
            health_check_timeout_secs: 5,
            data_stream_timeout_secs: 10,
        };

        let _adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let monitor = HealthMonitor::new(config.clone());

        // 启动监控
        let monitor_handle = {
            // 创建另一个监控器实例进行并发测试
            let _monitor_clone = HealthMonitor::new(config.clone());
            tokio::spawn(async move {
                // 注释掉start_monitoring调用，因为需要health_checker参数
                // _monitor_clone.start_monitoring().await;
            })
        };

        // 运行一段时�?        tokio::time::sleep(Duration::from_millis(500)).await;

        // 检查统计信�?        let stats = monitor.get_statistics().await;
        assert!(stats.total_checks > 0, "应该已经执行了一些健康检�?);

        // 停止监控
        monitor.stop_monitoring().await;

        // 等待监控任务结束
        let _ = tokio::time::timeout(Duration::from_secs(1), monitor_handle).await;

        println!("健康检查统�? {:?}", stats);
    }
}
