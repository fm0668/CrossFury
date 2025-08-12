use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use futures;
use std::time::Duration;
use std::time::SystemTime;
use tokio::runtime::Runtime;
use trifury::connectors::common::batch_subscription::{
    BatchSubscriptionConfig, BatchSubscriptionManager, SubscriptionRequest,
};
use trifury::types::common::DataType;
use trifury::types::errors::ConnectorError;

/// 批量订阅管理器初始化基准测试
fn benchmark_batch_manager_init(c: &mut Criterion) {
    c.bench_function("batch_manager_init", |b| {
        b.iter(|| {
            let config = BatchSubscriptionConfig::default();
            let manager = BatchSubscriptionManager::new(config);
            black_box(manager)
        })
    });
}

/// 单个订阅请求添加基准测试
fn benchmark_single_subscription_add(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("single_subscription_add", |b| {
        b.iter(|| {
            rt.block_on(async {
                let manager = BatchSubscriptionManager::with_default_config();
                let result = manager
                    .add_subscription_request(
                        "BTCUSDT".to_string(),
                        vec![DataType::OrderBook, DataType::Trade],
                        Some(1),
                    )
                    .await;
                black_box(result)
            })
        })
    });
}

/// 批量订阅请求添加基准测试
fn benchmark_batch_subscription_add(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_subscription_add");

    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            format!("batch_size_{}", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    rt.block_on(async {
                        let manager = BatchSubscriptionManager::with_default_config();
                        let symbols: Vec<String> =
                            (0..size).map(|i| format!("SYMBOL{}", i)).collect();

                        let result = manager
                            .add_batch_subscription_requests(
                                symbols,
                                vec![DataType::OrderBook],
                                Some(1),
                            )
                            .await;
                        black_box(result)
                    })
                })
            },
        );
    }
    group.finish();
}

/// 批次处理性能基准测试
fn benchmark_batch_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_processing");

    for batch_size in [5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            format!("batch_size_{}", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut config = BatchSubscriptionConfig::default();
                        config.batch_size = size;
                        config.batch_delay_ms = 0; // 无延迟以提高测试速度
                        let manager = BatchSubscriptionManager::new(config);

                        // 添加订阅请求
                        for i in 0..size {
                            let _ = manager
                                .add_subscription_request(
                                    format!("SYMBOL{}", i),
                                    vec![DataType::OrderBook],
                                    Some(1),
                                )
                                .await;
                        }

                        // 模拟订阅处理函数
                        let subscription_handler = |requests: Vec<SubscriptionRequest>| async move {
                            // 模拟处理延迟
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            let results: Vec<(String, bool, Option<String>)> = requests
                                .into_iter()
                                .map(|req| (req.symbol, true, None))
                                .collect();
                            Ok::<_, ConnectorError>(results)
                        };

                        let result = manager.process_next_batch(subscription_handler).await;
                        black_box(result)
                    })
                })
            },
        );
    }
    group.finish();
}

/// 并发批次处理基准测试
fn benchmark_concurrent_batch_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_batch_processing");

    for concurrent_batches in [1, 2, 3, 5].iter() {
        group.throughput(Throughput::Elements(*concurrent_batches as u64));
        group.bench_with_input(
            format!("concurrent_batches_{}", concurrent_batches),
            concurrent_batches,
            |b, &concurrent_batches| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut config = BatchSubscriptionConfig::default();
                        config.max_concurrent_batches = concurrent_batches;
                        config.batch_delay_ms = 0;
                        let manager = BatchSubscriptionManager::new(config);

                        // 添加大量订阅请求
                        for i in 0..100 {
                            let _ = manager
                                .add_subscription_request(
                                    format!("SYMBOL{}", i),
                                    vec![DataType::OrderBook],
                                    Some(1),
                                )
                                .await;
                        }

                        // 模拟订阅处理函数
                        let subscription_handler = |requests: Vec<SubscriptionRequest>| async move {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                            let results: Vec<(String, bool, Option<String>)> = requests
                                .into_iter()
                                .map(|req| (req.symbol, true, None))
                                .collect();
                            Ok::<_, ConnectorError>(results)
                        };

                        // 并发处理多个批次
                        let mut tasks = Vec::new();
                        for _ in 0..concurrent_batches {
                            let manager_clone = manager.clone();
                            let handler_clone = subscription_handler.clone();
                            tasks.push(tokio::spawn(async move {
                                manager_clone.process_next_batch(handler_clone).await
                            }));
                        }

                        let results = futures::future::join_all(tasks).await;
                        black_box(results)
                    })
                })
            },
        );
    }
    group.finish();
}

/// 批量订阅配置对比基准测试
fn benchmark_batch_config_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_config_comparison");

    let configs = vec![
        (
            "small_batch",
            BatchSubscriptionConfig {
                batch_size: 5,
                batch_delay_ms: 100,
                subscription_timeout_ms: 1000,
                max_retries: 2,
                retry_delay_ms: 500,
                enabled: true,
                max_concurrent_batches: 2,
            },
        ),
        (
            "medium_batch",
            BatchSubscriptionConfig {
                batch_size: 10,
                batch_delay_ms: 200,
                subscription_timeout_ms: 2000,
                max_retries: 3,
                retry_delay_ms: 1000,
                enabled: true,
                max_concurrent_batches: 3,
            },
        ),
        (
            "large_batch",
            BatchSubscriptionConfig {
                batch_size: 20,
                batch_delay_ms: 500,
                subscription_timeout_ms: 5000,
                max_retries: 5,
                retry_delay_ms: 2000,
                enabled: true,
                max_concurrent_batches: 5,
            },
        ),
    ];

    for (name, config) in configs {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let manager = BatchSubscriptionManager::new(config.clone());

                    // 添加订阅请求
                    for i in 0..config.batch_size {
                        let _ = manager
                            .add_subscription_request(
                                format!("SYMBOL{}", i),
                                vec![DataType::OrderBook],
                                Some(1),
                            )
                            .await;
                    }

                    let subscription_handler = |requests: Vec<SubscriptionRequest>| async move {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        let results: Vec<(String, bool, Option<String>)> = requests
                            .into_iter()
                            .map(|req| (req.symbol, true, None))
                            .collect();
                        Ok::<_, ConnectorError>(results)
                    };

                    let result = manager.process_next_batch(subscription_handler).await;
                    black_box(result)
                })
            })
        });
    }
    group.finish();
}

/// 批量订阅内存使用基准测试
fn benchmark_batch_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_memory_usage");

    for request_count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*request_count as u64));
        group.bench_with_input(
            format!("requests_{}", request_count),
            request_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let start_time = SystemTime::now();
                        let manager = BatchSubscriptionManager::with_default_config();

                        // 添加大量订阅请求以测试内存使用
                        for i in 0..count {
                            let _ = manager
                                .add_subscription_request(
                                    format!("SYMBOL{}", i),
                                    vec![DataType::OrderBook, DataType::Trade],
                                    Some(1),
                                )
                                .await;
                        }

                        let duration = start_time.elapsed().unwrap();
                        black_box((manager, duration))
                    })
                })
            },
        );
    }
    group.finish();
}

/// 批量订阅错误处理基准测试
fn benchmark_batch_error_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("batch_error_handling", |b| {
        b.iter(|| {
            rt.block_on(async {
                let manager = BatchSubscriptionManager::with_default_config();

                // 添加订阅请求
                for i in 0..10 {
                    let _ = manager
                        .add_subscription_request(
                            format!("SYMBOL{}", i),
                            vec![DataType::OrderBook],
                            Some(1),
                        )
                        .await;
                }

                // 模拟失败的订阅处理函数
                let subscription_handler = |_requests: Vec<SubscriptionRequest>| async move {
                    Err::<Vec<(String, bool, Option<String>)>, ConnectorError>(
                        ConnectorError::SubscriptionFailed("模拟错误".to_string()),
                    )
                };

                let result = manager.process_next_batch(subscription_handler).await;
                black_box(result)
            })
        })
    });
}

criterion_group!(
    benches,
    benchmark_batch_manager_init,
    benchmark_single_subscription_add,
    benchmark_batch_subscription_add,
    benchmark_batch_processing,
    benchmark_concurrent_batch_processing,
    benchmark_batch_config_comparison,
    benchmark_batch_memory_usage,
    benchmark_batch_error_handling
);
criterion_main!(benches);
