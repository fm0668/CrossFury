use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

use trifury::connectors::{
    traits::modern::ModernExchangeConnector,
    traits::modern_binance::{ModernBinanceConfig, ModernBinanceConnector},
    traits::subscription_manager::{DataType, SubscriptionConfig, SubscriptionPriority},
};

/// 基准测试：连接器初始化性能
fn benchmark_connector_initialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("connector_initialization", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut connector = ModernBinanceConnector::new();
                let config = ModernBinanceConfig::default();

                black_box(connector.initialize(config).await.unwrap());
            })
        })
    });
}

/// 基准测试：连接建立性能
fn benchmark_connection_establishment(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("connection_establishment", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut connector = ModernBinanceConnector::new();
                let config = ModernBinanceConfig::default();

                connector.initialize(config).await.unwrap();
                black_box(connector.connect().await.unwrap());
                connector.disconnect().await.unwrap();
            })
        })
    });
}

/// 基准测试：订阅性能（单个符号）
fn benchmark_single_subscription(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("single_subscription", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut connector = ModernBinanceConnector::new();
                let config = ModernBinanceConfig::default();

                connector.initialize(config).await.unwrap();
                connector.connect().await.unwrap();

                let sub_config = SubscriptionConfig {
                    symbols: vec!["BTCUSDT".to_string()],
                    data_types: vec![DataType::Ticker],
                    batch_size: Some(1),
                    priority: SubscriptionPriority::Medium,
                };

                black_box(connector.subscribe(sub_config).await.unwrap());
                connector.disconnect().await.unwrap();
            })
        })
    });
}

/// 基准测试：批量订阅性能
fn benchmark_batch_subscription(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_subscription");

    for batch_size in [10, 50, 100, 200].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("symbols", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut connector = ModernBinanceConnector::new();
                        let config = ModernBinanceConfig::default();

                        connector.initialize(config).await.unwrap();
                        connector.connect().await.unwrap();

                        // 生成批量符号
                        let symbols: Vec<String> = (0..batch_size)
                            .map(|i| format!("SYMBOL{}USDT", i))
                            .collect();

                        let sub_config = SubscriptionConfig {
                            symbols,
                            data_types: vec![DataType::Ticker],
                            batch_size: Some(batch_size),
                            priority: SubscriptionPriority::Medium,
                        };

                        black_box(connector.subscribe(sub_config).await.unwrap());
                        connector.disconnect().await.unwrap();
                    })
                })
            },
        );
    }
    group.finish();
}

/// 基准测试：连接状态查询性能
fn benchmark_status_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("status_queries", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut connector = ModernBinanceConnector::new();
                let config = ModernBinanceConfig::default();

                connector.initialize(config).await.unwrap();
                connector.connect().await.unwrap();

                // 执行多个状态查询
                black_box(connector.connection_status().await);
                black_box(connector.subscription_status().await);
                black_box(connector.metrics().await);
                black_box(connector.health_check().await.unwrap());

                connector.disconnect().await.unwrap();
            })
        })
    });
}

/// 基准测试：并发连接性能
fn benchmark_concurrent_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_connections");

    for connection_count in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*connection_count as u64));
        group.bench_with_input(
            BenchmarkId::new("connections", connection_count),
            connection_count,
            |b, &connection_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();

                        for _ in 0..connection_count {
                            let handle = tokio::spawn(async {
                                let mut connector = ModernBinanceConnector::new();
                                let config = ModernBinanceConfig::default();

                                connector.initialize(config).await.unwrap();
                                connector.connect().await.unwrap();

                                // 模拟一些工作
                                tokio::time::sleep(Duration::from_millis(10)).await;

                                connector.disconnect().await.unwrap();
                            });
                            handles.push(handle);
                        }

                        // 等待所有连接完成
                        for handle in handles {
                            black_box(handle.await.unwrap());
                        }
                    })
                })
            },
        );
    }
    group.finish();
}

/// 基准测试：内存使用情况
fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("memory_usage", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut connectors = Vec::new();

                // 创建多个连接器实例来测试内存使用
                for _ in 0..10 {
                    let mut connector = ModernBinanceConnector::new();
                    let config = ModernBinanceConfig::default();

                    connector.initialize(config).await.unwrap();
                    connector.connect().await.unwrap();

                    connectors.push(connector);
                }

                // 执行一些操作
                for connector in &mut connectors {
                    let sub_config = SubscriptionConfig {
                        symbols: vec!["BTCUSDT".to_string()],
                        data_types: vec![DataType::Ticker],
                        batch_size: Some(1),
                        priority: SubscriptionPriority::Medium,
                    };

                    connector.subscribe(sub_config).await.unwrap();
                }

                // 清理
                for mut connector in connectors {
                    black_box(connector.disconnect().await.unwrap());
                }
            })
        })
    });
}

criterion_group!(
    benches,
    benchmark_connector_initialization,
    benchmark_connection_establishment,
    benchmark_single_subscription,
    benchmark_batch_subscription,
    benchmark_status_queries,
    benchmark_concurrent_connections,
    benchmark_memory_usage
);
criterion_main!(benches);
