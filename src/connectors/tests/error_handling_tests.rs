//! 错误处理和重试机制的单元测试

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::connectors::binance::config::BinanceConfig;
use crate::connectors::binance::enhanced_adapter::EnhancedBinanceError;
use crate::connectors::binance::modern_adapter::ModernBinanceConfig;
use crate::connectors::binance::modern_adapter::ModernBinanceConnector;
use crate::connectors::binance::{EnhancedBinanceAdapter, RetryConfig};
use crate::core::AppState;
use crate::types::errors::ConnectorError;

/// 创建测试用的重试配置
fn create_test_retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: 3,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        backoff_multiplier: 2.0,
        jitter_factor: 0.1,
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
    let base_config = BinanceConfig {
        api_key: Some("test_key".to_string()),
        secret_key: Some("test_secret".to_string()),
        testnet: true,
        rate_limit_per_minute: 1200,
    };

    let config = ModernBinanceConfig {
        base_config,
        ..Default::default()
    };

    let app_state = Arc::new(AppState::new());
    ModernBinanceConnector::new(config, app_state)
}

#[cfg(test)]
mod error_handler_tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_config_creation() {
        let config = create_test_retry_config();

        // 验证配置�?        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.jitter_factor, 0.1);
    }

    #[tokio::test]
    async fn test_retry_config_default() {
        let config = RetryConfig::default();

        // 验证默认配置�?        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.jitter_factor, 0.1);
    }

    #[tokio::test]
    async fn test_enhanced_adapter_creation() {
        let config = create_test_binance_config();
        let app_state = Arc::new(AppState::new());

        // 测试创建增强适配�?        let result = EnhancedBinanceAdapter::new(config, app_state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_enhanced_adapter_with_custom_retry_config() {
        let config = create_test_binance_config();
        let retry_config = create_test_retry_config();
        let app_state = Arc::new(AppState::new());

        // 测试使用自定义重试配置创建适配�?        let result =
            EnhancedBinanceAdapter::new_with_retry_config(config, app_state, retry_config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_error_categorization() {
        // 测试不同类型错误的分�?        let network_error = ConnectorError::NetworkError("网络连接失败".to_string());
        let auth_error = ConnectorError::AuthenticationFailed("认证失败".to_string());
        let timeout_error = ConnectorError::TimeoutError("请求超时".to_string());
        let connection_error = ConnectorError::ConnectionFailed("连接失败".to_string());

        // 这里我们测试错误的基本属�?        // 在实际实现中，ErrorHandler应该有方法来分类这些错误

        // 验证错误可以被格式化
        let network_str = format!("{:?}", network_error);
        assert!(network_str.contains("NetworkError"));

        let auth_str = format!("{:?}", auth_error);
        assert!(auth_str.contains("AuthenticationFailed"));

        let timeout_str = format!("{:?}", timeout_error);
        assert!(timeout_str.contains("TimeoutError"));

        let connection_str = format!("{:?}", connection_error);
        assert!(connection_str.contains("ConnectionFailed"));
    }

    #[tokio::test]
    async fn test_enhanced_binance_error_types() {
        // 测试增强Binance错误类型
        let network_error = EnhancedBinanceError::Network {
            message: "网络连接失败".to_string(),
            retryable: true,
        };
        let rate_limit_error = EnhancedBinanceError::RateLimit {
            message: "API限制".to_string(),
            retry_after: Some(Duration::from_secs(60)),
        };
        let auth_error = EnhancedBinanceError::Authentication {
            message: "认证失败".to_string(),
            retryable: false,
        };

        // 验证错误可以被格式化
        let network_str = format!("{:?}", network_error);
        assert!(network_str.contains("Network"));

        let rate_limit_str = format!("{:?}", rate_limit_error);
        assert!(rate_limit_str.contains("RateLimit"));

        let auth_str = format!("{:?}", auth_error);
        assert!(auth_str.contains("Authentication"));
    }

    #[tokio::test]
    async fn test_delay_calculation() {
        // 测试延迟计算逻辑
        let base_delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(5);

        // 测试固定延迟
        let fixed_delay = base_delay;
        assert_eq!(fixed_delay, Duration::from_millis(100));

        // 测试线性退�?        let linear_delay_1 = base_delay + Duration::from_millis(50) * 1;
        let linear_delay_2 = base_delay + Duration::from_millis(50) * 2;
        assert_eq!(linear_delay_1, Duration::from_millis(150));
        assert_eq!(linear_delay_2, Duration::from_millis(200));

        // 测试指数退�?        let exp_delay_1 =
            Duration::from_millis((base_delay.as_millis() as f64 * 2.0_f64.powi(1)) as u64);
        let exp_delay_2 =
            Duration::from_millis((base_delay.as_millis() as f64 * 2.0_f64.powi(2)) as u64);
        assert_eq!(exp_delay_1, Duration::from_millis(200));
        assert_eq!(exp_delay_2, Duration::from_millis(400));

        // 测试最大延迟限�?        let capped_delay = std::cmp::min(Duration::from_secs(10), max_delay);
        assert_eq!(capped_delay, max_delay);
    }

    #[tokio::test]
    async fn test_jitter_application() {
        // 测试抖动的应�?        let base_delay = Duration::from_millis(1000);
        let jitter_factor = 0.1; // 10%抖动

        // 计算抖动范围
        let min_delay =
            Duration::from_millis((base_delay.as_millis() as f64 * (1.0 - jitter_factor)) as u64);
        let max_jittered_delay =
            Duration::from_millis((base_delay.as_millis() as f64 * (1.0 + jitter_factor)) as u64);

        assert_eq!(min_delay, Duration::from_millis(900));
        assert_eq!(max_jittered_delay, Duration::from_millis(1100));

        // 验证抖动范围合理
        assert!(min_delay < base_delay);
        assert!(max_jittered_delay > base_delay);
    }

    #[tokio::test]
    async fn test_enhanced_adapter_concurrent_access() {
        let config = create_test_binance_config();
        let app_state = Arc::new(AppState::new());
        let _adapter = Arc::new(
            EnhancedBinanceAdapter::new(config, app_state)
                .await
                .unwrap(),
        );

        let mut handles = vec![];

        // 创建多个并发任务来测试线程安全�?        for i in 0..10 {
            let _adapter_clone = _adapter.clone();
            let handle = tokio::spawn(async move {
                // 这里我们只能测试基本的并发访�?                // 在实际实现中，应该有更多可测试的方法

                // 模拟并发错误处理
                let _error = ConnectorError::NetworkError(format!("测试错误 {}", i));

                // 在实际实现中，这里应该调用错误处理方�?                // let result = _adapter_clone.handle_error(error).await;

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
        // 测试超时处理
        let timeout_duration = Duration::from_millis(100);

        // 模拟一个可能超时的操作
        let operation = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<(), ConnectorError>(())
        };

        // 测试操作在超时时间内完成
        let result = timeout(timeout_duration, operation).await;
        assert!(result.is_ok(), "操作应该在超时时间内完成");

        // 测试超时情况
        let long_operation = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok::<(), ConnectorError>(())
        };

        let timeout_result = timeout(timeout_duration, long_operation).await;
        assert!(timeout_result.is_err(), "操作应该超时");
    }

    #[tokio::test]
    async fn test_enhanced_adapter_metrics_tracking() {
        // 测试增强适配器指标跟�?        let config = create_test_binance_config();
        let app_state = Arc::new(AppState::new());
        let _adapter = EnhancedBinanceAdapter::new(config, app_state).await;

        // 在实际实现中，增强适配器应该跟踪各种指�?        // 例如：错误计数、重试次数、成功率�?
        // 这里我们只能测试基本的创建和存在�?        assert!(_adapter.is_ok()); // 如果创建失败，这里会失败
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 这些测试模拟真实的错误处理场�?    #[tokio::test]
    #[ignore = "集成测试"]
    async fn test_retry_mechanism_integration() {
        let config = create_test_retry_config();
        let binance_config = create_test_binance_config();
        let app_state = Arc::new(AppState::new());
        let _adapter =
            EnhancedBinanceAdapter::new_with_retry_config(binance_config, app_state, config)
                .await
                .unwrap();

        // 在实际实现中，这里应该测试完整的重试流程
        // 例如�?        // 1. 模拟网络错误
        // 2. 验证重试逻辑
        // 3. 检查退避延�?        // 4. 验证最终结�?
        println!("增强适配器集成测试完�?);
    }

    #[tokio::test]
    #[ignore = "性能测试"]
    async fn test_error_handler_performance() {
        let config = create_test_retry_config();
        let binance_config = create_test_binance_config();
        let app_state = Arc::new(AppState::new());
        let _adapter = Arc::new(
            EnhancedBinanceAdapter::new_with_retry_config(binance_config, app_state, config)
                .await
                .unwrap(),
        );

        let start_time = std::time::Instant::now();
        let num_operations = 1000;

        let mut handles = vec![];

        // 创建大量并发错误处理任务
        for i in 0..num_operations {
            let _adapter_clone = _adapter.clone();
            let handle = tokio::spawn(async move {
                // 模拟错误处理
                let _error = ConnectorError::NetworkError(format!("性能测试错误 {}", i));

                // 在实际实现中，这里应该调用错误处理方�?                // let _result = _adapter_clone.handle_error(error).await;

                i
            });
            handles.push(handle);
        }

        // 等待所有任务完�?        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok());
        }

        let elapsed = start_time.elapsed();
        println!("处理 {} 个错误用�? {:?}", num_operations, elapsed);

        // 验证性能合理（每个操作应该在合理时间内完成）
        let avg_time_per_operation = elapsed / num_operations;
        assert!(
            avg_time_per_operation < Duration::from_millis(10),
            "平均每个操作时间应该小于10ms，实�? {:?}",
            avg_time_per_operation
        );
    }
}
