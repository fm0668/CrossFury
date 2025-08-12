//! 连接器强化功能的测试模块
//!
//! 这个模块包含了对现代化连接器功能的全面测试，包括�?//! - ModernExchangeConnector trait的测�?//! - 健康检查和自动恢复功能的测�?//! - 错误处理和重试机制的测试

pub mod error_handling_tests;
pub mod health_monitor_tests;
pub mod modern_connector_tests;

// 重新导出测试模块，方便外部访�?// pub use modern_connector_tests::*;
// pub use health_monitor_tests::*;
// pub use error_handling_tests::*;

/// 测试工具函数和通用测试配置
pub mod test_utils {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::connectors::binance::config::BinanceConfig;
    use crate::connectors::binance::health_monitor::HealthCheckConfig;
    use crate::connectors::binance::modern_adapter::ModernBinanceConnector;
    // 注释掉不存在的导�?    // use crate::connectors::binance::error_handler::RetryConfig;
    // use crate::connectors::binance::error_handler::BackoffStrategy;
    use crate::core::AppState;

    /// 创建标准的测试Binance配置
    pub fn create_standard_test_config() -> BinanceConfig {
        BinanceConfig {
            api_key: None,
            secret_key: None,
            testnet: true,
            rate_limit_per_minute: 1200,
        }
    }

    /// 创建用于快速测试的健康检查配�?    pub fn create_fast_health_config() -> HealthCheckConfig {
        HealthCheckConfig {
            check_interval_secs: 1,
            connection_timeout_secs: 5,
            max_consecutive_failures: 2,
            auto_recovery_enabled: true,
            recovery_delay_secs: 1,
            health_check_timeout_secs: 5,
            data_stream_timeout_secs: 10,
        }
    }

    /// 创建用于测试的重试配置（暂时注释掉）
    // pub fn create_test_retry_config() -> RetryConfig {
    //     RetryConfig {
    //         max_attempts: 3,
    //         base_delay: Duration::from_millis(10),
    //         max_delay: Duration::from_millis(100),
    //         backoff_strategy: BackoffStrategy::Exponential,
    //         jitter: false, // 禁用抖动以便测试预测
    //         retry_on_timeout: true,
    //         retry_on_network_error: true,
    //         retry_on_rate_limit: true,
    //         retry_on_server_error: true,
    //     }
    // }

    /// 创建测试用的现代化适配�?    pub async fn create_test_modern_adapter(
    ) -> Result<ModernBinanceConnector, Box<dyn std::error::Error + Send + Sync>> {
        let base_config = create_standard_test_config();
        let config = crate::connectors::binance::modern_adapter::ModernBinanceConfig {
            base_config,
            ..Default::default()
        };
        let app_state = Arc::new(AppState::new());
        Ok(ModernBinanceConnector::new(config, app_state))
    }

    /// 初始化测试日�?    pub fn init_test_logging() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    /// 等待指定时间，用于测试中的延�?    pub async fn test_delay(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    /// 创建测试用的超时配置
    pub fn create_test_timeout() -> Duration {
        Duration::from_secs(5)
    }

    /// 验证测试结果的辅助宏
    #[macro_export]
    macro_rules! assert_test_result {
        ($result:expr, $expected:expr, $message:expr) => {
            match $result {
                Ok(value) => assert_eq!(value, $expected, $message),
                Err(e) => panic!("测试失败: {} - 错误: {:?}", $message, e),
            }
        };
    }

    /// 验证异步操作超时的辅助函�?    pub async fn assert_timeout<F, T>(
        operation: F,
        timeout_duration: Duration,
        should_timeout: bool,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
    where
        F: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
    {
        let result = tokio::time::timeout(timeout_duration, operation).await;

        match (result, should_timeout) {
            (Ok(inner_result), false) => inner_result,
            (Err(_), true) => Err("操作按预期超�?.into()),
            (Ok(_), true) => Err("操作应该超时但没有超�?.into()),
            (Err(_), false) => Err("操作意外超时".into()),
        }
    }

    /// 并发测试辅助函数
    pub async fn run_concurrent_test<F, T>(
        task_count: usize,
        task_factory: F,
    ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(
            usize,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<T, Box<dyn std::error::Error + Send + Sync>>,
                    > + Send,
            >,
        >,
        T: Send + 'static,
    {
        let mut handles = Vec::new();

        for i in 0..task_count {
            let task = task_factory(i);
            let handle = tokio::spawn(task);
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            let result = handle.await??;
            results.push(result);
        }

        Ok(results)
    }
}

/// 集成测试模块
/// 这些测试需要更多的设置和可能的网络连接
#[cfg(test)]
mod integration_tests {
    use super::test_utils::*;
    use crate::connectors::traits::modern::ModernExchangeConnector;
    use std::time::Duration;

    #[tokio::test]
    #[ignore = "集成测试"]
    async fn test_full_connector_lifecycle() {
        init_test_logging();

        // 创建适配�?        let mut adapter = create_test_modern_adapter().await.unwrap();

        // 测试完整的生命周�?        // 1. 初始状态检�?        let initial_status = adapter.connection_status().await;
        println!("初始状�? {:?}", initial_status);

        // 2. 尝试连接（可能失败，这是正常的）
        let connect_result = adapter.connect().await;
        println!("连接结果: {:?}", connect_result);

        // 3. 检查指�?        let metrics = adapter.metrics().await;
        println!("连接指标: {:?}", metrics);

        // 4. 检查健康状�?        let is_healthy = adapter.is_healthy().await;
        println!("健康状�? {}", is_healthy);

        // 5. 清理
        let disconnect_result = adapter.disconnect().await;
        println!("断开连接结果: {:?}", disconnect_result);
    }

    #[tokio::test]
    #[ignore = "性能测试"]
    async fn test_connector_performance() {
        init_test_logging();

        let adapter = create_test_modern_adapter().await.unwrap();
        let start_time = std::time::Instant::now();

        // 执行大量并发操作
        let operations = 1000;
        let results = run_concurrent_test(operations, |i| {
            let adapter_clone = adapter.clone();
            Box::pin(async move {
                let _status = adapter_clone.connection_status().await;
                let _metrics = adapter_clone.metrics().await;
                Ok::<usize, Box<dyn std::error::Error + Send + Sync>>(i)
            })
        })
        .await
        .unwrap();

        let elapsed = start_time.elapsed();
        println!("执行 {} 个操作用�? {:?}", operations, elapsed);
        println!("平均每个操作: {:?}", elapsed / operations as u32);

        assert_eq!(results.len(), operations);
        assert!(
            elapsed < Duration::from_secs(10),
            "性能测试应该�?0秒内完成"
        );
    }

    #[tokio::test]
    #[ignore = "压力测试"]
    async fn test_connector_stress() {
        init_test_logging();

        let adapter = create_test_modern_adapter().await.unwrap();

        // 长时间运行测�?        let test_duration = Duration::from_secs(30);
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < test_duration {
            // 持续执行操作
            let _status = adapter.get_connection_status().await;
            let _metrics = adapter.get_metrics().await;
            let _health = adapter.get_connection_health().await;

            // 短暂休息
            test_delay(Duration::from_millis(10)).await;
        }

        println!("压力测试完成，运行时�? {:?}", start_time.elapsed());
    }
}
