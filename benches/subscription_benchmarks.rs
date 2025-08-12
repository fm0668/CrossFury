use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use futures;
use std::time::Duration;
use tokio::runtime::Runtime;
use trifury::connectors::traits::subscription_manager::{
    ConnectionStrategy, DataType, SubscriptionConfig, SubscriptionManager,
    SubscriptionManagerConfig, SubscriptionPriority,
};

/// 订阅管理器初始化基准测试
fn benchmark_subscription_manager_init(c: &mut Criterion) {
    c.bench_function("subscription_manager_init", |b| {
        b.iter(|| {
            let config = SubscriptionManagerConfig::default();
            let manager = SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);
            black_box(manager)
        })
    });
}

/// 单个订阅操作基准测试
fn benchmark_single_subscription(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("single_subscription", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = SubscriptionManagerConfig::default();
                let manager =
                    SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);

                let sub_config = SubscriptionConfig {
                    symbols: vec!["BTCUSDT".to_string()],
                    data_types: vec![DataType::Ticker],
                    batch_size: None,
                    priority: SubscriptionPriority::Medium,
                };

                let result = manager.subscribe(sub_config).await;
                black_box(result)
            })
        })
    });
}

/// 批量订阅操作基准测试
fn benchmark_batch_subscription(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_subscription");

    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            format!("batch_size_{}", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = SubscriptionManagerConfig::default();
                        let manager =
                            SubscriptionManager::new(config, ConnectionStrategy::GroupByDataType);

                        let symbols: Vec<String> =
                            (0..size).map(|i| format!("SYMBOL{}USDT", i)).collect();

                        let sub_config = SubscriptionConfig {
                            symbols,
                            data_types: vec![DataType::Ticker, DataType::OrderBook],
                            batch_size: Some(size),
                            priority: SubscriptionPriority::Medium,
                        };

                        let result = manager.subscribe(sub_config).await;
                        black_box(result)
                    })
                })
            },
        );
    }
    group.finish();
}

/// 订阅状态查询基准测试
fn benchmark_subscription_status_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("subscription_status_query", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = SubscriptionManagerConfig::default();
                let manager =
                    SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);

                // 先添加一些订阅
                let sub_config = SubscriptionConfig {
                    symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
                    data_types: vec![DataType::Ticker, DataType::OrderBook],
                    batch_size: None,
                    priority: SubscriptionPriority::Medium,
                };
                let _ = manager.subscribe(sub_config).await;

                // 查询状态
                let subscriptions = manager.get_subscriptions().await;
                let connections = manager.get_connections().await;
                black_box((subscriptions, connections))
            })
        })
    });
}

/// 取消订阅操作基准测试
fn benchmark_unsubscription(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("unsubscription", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = SubscriptionManagerConfig::default();
                let manager =
                    SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);

                // 先添加订阅
                let sub_config = SubscriptionConfig {
                    symbols: vec!["BTCUSDT".to_string()],
                    data_types: vec![DataType::Ticker],
                    batch_size: None,
                    priority: SubscriptionPriority::Medium,
                };
                let _ = manager.subscribe(sub_config.clone()).await;

                // 取消订阅
                let result = manager.unsubscribe(sub_config).await;
                black_box(result)
            })
        })
    });
}

/// 健康检查基准测试
fn benchmark_health_check(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("health_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = SubscriptionManagerConfig::default();
                let manager =
                    SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);

                // 添加一些订阅以模拟真实场景
                let sub_config = SubscriptionConfig {
                    symbols: vec![
                        "BTCUSDT".to_string(),
                        "ETHUSDT".to_string(),
                        "ADAUSDT".to_string(),
                    ],
                    data_types: vec![DataType::Ticker, DataType::OrderBook, DataType::Trade],
                    batch_size: None,
                    priority: SubscriptionPriority::Medium,
                };
                let _ = manager.subscribe(sub_config).await;

                // 执行健康检查
                let health = manager.health_check().await;
                black_box(health)
            })
        })
    });
}

/// 连接策略性能对比基准测试
fn benchmark_connection_strategies(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("connection_strategies");

    let strategies = vec![
        ("single_connection", ConnectionStrategy::SingleConnection),
        ("group_by_data_type", ConnectionStrategy::GroupByDataType),
        (
            "group_by_market_type",
            ConnectionStrategy::GroupByMarketType,
        ),
        ("load_balanced", ConnectionStrategy::LoadBalanced),
    ];

    for (name, strategy) in strategies {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let config = SubscriptionManagerConfig::default();
                    let manager = SubscriptionManager::new(config, strategy.clone());

                    let sub_config = SubscriptionConfig {
                        symbols: vec![
                            "BTCUSDT".to_string(),
                            "ETHUSDT".to_string(),
                            "ADAUSDT".to_string(),
                            "BNBUSDT".to_string(),
                        ],
                        data_types: vec![DataType::Ticker, DataType::OrderBook],
                        batch_size: None,
                        priority: SubscriptionPriority::Medium,
                    };

                    let result = manager.subscribe(sub_config).await;
                    black_box(result)
                })
            })
        });
    }
    group.finish();
}

/// 并发订阅操作基准测试
fn benchmark_concurrent_subscriptions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("concurrent_subscriptions", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = SubscriptionManagerConfig::default();
                let manager = std::sync::Arc::new(SubscriptionManager::new(
                    config,
                    ConnectionStrategy::LoadBalanced,
                ));

                let mut handles = Vec::new();

                // 创建10个并发订阅任务
                for i in 0..10 {
                    let manager_clone = manager.clone();
                    let handle = tokio::spawn(async move {
                        let sub_config = SubscriptionConfig {
                            symbols: vec![format!("SYMBOL{}USDT", i)],
                            data_types: vec![DataType::Ticker],
                            batch_size: None,
                            priority: SubscriptionPriority::Medium,
                        };
                        manager_clone.subscribe(sub_config).await
                    });
                    handles.push(handle);
                }

                // 等待所有任务完成
                let results = futures::future::join_all(handles).await;
                black_box(results)
            })
        })
    });
}

/// 内存使用基准测试
fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("memory_usage", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = SubscriptionManagerConfig::default();
                let manager =
                    SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);

                // 添加大量订阅以测试内存使用
                let symbols: Vec<String> = (0..1000).map(|i| format!("SYMBOL{}USDT", i)).collect();

                let sub_config = SubscriptionConfig {
                    symbols,
                    data_types: vec![DataType::Ticker, DataType::OrderBook, DataType::Trade],
                    batch_size: Some(100),
                    priority: SubscriptionPriority::Medium,
                };

                let result = manager.subscribe(sub_config).await;

                // 检查内存使用情况
                let subscriptions = manager.get_subscriptions().await;
                let connections = manager.get_connections().await;

                black_box((result, subscriptions.len(), connections.len()))
            })
        })
    });
}

criterion_group!(
    benches,
    benchmark_subscription_manager_init,
    benchmark_single_subscription,
    benchmark_batch_subscription,
    benchmark_subscription_status_query,
    benchmark_unsubscription,
    benchmark_health_check,
    benchmark_connection_strategies,
    benchmark_concurrent_subscriptions,
    benchmark_memory_usage
);
criterion_main!(benches);
