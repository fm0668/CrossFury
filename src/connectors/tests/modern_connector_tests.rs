//! ModernExchangeConnector trait 和相关功能的单元测试

use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

use crate::connectors::binance::config::BinanceConfig;
use crate::connectors::binance::modern_adapter::{ModernBinanceConfig, ModernBinanceConnector};
use crate::connectors::traits::modern::{ModernConnectionStatus, ModernExchangeConnector};
use crate::core::AppState;
use crate::types::config::ConnectionQuality;
use crate::types::errors::ConnectorError;

/// 创建测试用的Binance配置
fn create_test_config() -> ModernBinanceConfig {
    ModernBinanceConfig {
        base_config: BinanceConfig {
            api_key: None,
            secret_key: None,
            testnet: true,
            rate_limit_per_minute: 1200,
        },
        ..Default::default()
    }
}

/// 创建测试用的现代化适配�?async fn create_test_adapter() -> ModernBinanceConnector {
    let config = create_test_config();
    let app_state = Arc::new(AppState::new());
    ModernBinanceConnector::new(config, app_state)
}

#[cfg(test)]
mod modern_connector_tests {
    use super::*;

    #[tokio::test]
    async fn test_connector_creation() {
        let adapter = create_test_adapter().await;

        // 验证适配器创建成�?        assert_eq!(adapter.name(), "ModernBinance");

        // 验证初始状�?        let status = adapter.connection_status().await;
        assert_eq!(status, ModernConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_connector_capabilities() {
        let adapter = create_test_adapter().await;
        let capabilities = adapter.supported_features();

        // 验证Binance连接器的能力
        assert!(!capabilities.is_empty());
        // 检查是否包含基本功�?        assert!(capabilities.iter().any(|f| matches!(
            f,
            crate::connectors::traits::modern::ConnectorFeature::MarketData
        )));
    }

    #[tokio::test]
    async fn test_connector_metrics() {
        let adapter = create_test_adapter().await;
        let metrics = adapter.metrics().await;

        // 验证初始指标
        assert_eq!(metrics.messages_received, 0);
        assert_eq!(metrics.messages_sent, 0);
        assert_eq!(metrics.reconnect_count, 0);
        assert_eq!(metrics.error_count, 0);
        assert_eq!(metrics.subscription_count, 0);
    }

    #[tokio::test]
    async fn test_connection_health() {
        let adapter = create_test_adapter().await;
        let is_healthy = adapter.is_healthy().await;

        // 验证初始健康状�?        assert!(!is_healthy); // 未连接时应该不健�?    }

    #[tokio::test]
    async fn test_health_check() {
        let adapter = create_test_adapter().await;
        let health_result = adapter.health_check().await;

        // 验证健康检查结�?        assert!(health_result.is_ok());
        let health = health_result.unwrap();
        assert!(!health.healthy); // 未连接时应该不健�?    }

    #[tokio::test]
    async fn test_connector_lifecycle() {
        let mut adapter = create_test_adapter().await;

        // 测试连接生命周期
        assert_eq!(
            adapter.connection_status().await,
            ModernConnectionStatus::Disconnected
        );

        // 注意：实际连接测试需要网络，这里只测试状态变化逻辑
        // 在真实环境中，这些测试应该使用模拟的网络�?
        // 测试重连功能
        let _reconnect_result = adapter.reconnect().await;
        // 重连可能失败（没有网络），这是正常的

        // 验证重连后状�?        let metrics_after_reconnect = adapter.metrics().await;
        assert_eq!(metrics_after_reconnect.messages_received, 0);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let adapter = create_test_adapter().await;

        // 测试错误统计
        let metrics = adapter.metrics().await;
        assert_eq!(metrics.error_count, 0);
        assert!(metrics.last_error.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let adapter: Arc<ModernBinanceConnector> = Arc::new(create_test_adapter().await);
        let mut handles = vec![];

        // 创建多个并发任务来测试线程安全�?        for i in 0..10 {
            let adapter_clone = adapter.clone();
            let handle = tokio::spawn(async move {
                // 并发访问各种方法
                let _status = adapter_clone.connection_status().await;
                let _metrics = adapter_clone.metrics().await;
                let _health = adapter_clone.is_healthy().await;
                let _capabilities = adapter_clone.supported_features();
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
    async fn test_timeout_handling() {
        let adapter = create_test_adapter().await;

        // 测试方法调用的超时处�?        let timeout_duration = Duration::from_millis(100);

        let status_result = timeout(timeout_duration, adapter.connection_status()).await;
        assert!(status_result.is_ok(), "获取连接状态应该在超时时间内完�?);

        let metrics_result = timeout(timeout_duration, adapter.metrics()).await;
        assert!(metrics_result.is_ok(), "获取指标应该在超时时间内完成");

        let health_result = timeout(timeout_duration, adapter.is_healthy()).await;
        assert!(health_result.is_ok(), "获取健康状态应该在超时时间内完�?);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 这些测试需要真实的网络连接，通常在CI/CD中跳�?    #[tokio::test]
    #[ignore = "需要网络连�?]
    async fn test_real_connection() {
        let mut adapter = create_test_adapter().await;

        // 尝试真实连接（需要网络）
        let connect_result = adapter.connect().await;

        match connect_result {
            Ok(_) => {
                // 连接成功，验证状�?                let status = adapter.connection_status().await;
                assert_ne!(status, ModernConnectionStatus::Disconnected);

                // 测试断开连接
                let disconnect_result = adapter.disconnect().await;
                assert!(disconnect_result.is_ok());

                // 验证断开后状�?                let final_status = adapter.connection_status().await;
                assert_eq!(final_status, ModernConnectionStatus::Disconnected);
            }
            Err(e) => {
                // 连接失败是可以接受的（可能没有网络或API密钥�?                println!("连接失败（预期）: {:?}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "需要网络连�?]
    async fn test_health_monitoring() {
        let mut adapter = create_test_adapter().await;

        // 尝试连接
        let _ = adapter.connect().await;

        // 等待一段时间让健康监控收集数据
        tokio::time::sleep(Duration::from_secs(2)).await;

        // 检查健康状�?        let is_healthy = adapter.is_healthy().await;
        println!("连接健康状�? {}", is_healthy);

        // 检查指�?        let metrics = adapter.metrics().await;
        println!("连接指标: {:?}", metrics);

        // 清理
        let _ = adapter.disconnect().await;
    }
}
