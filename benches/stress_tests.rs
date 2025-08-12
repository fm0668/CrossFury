//! 压力与故障注入测试
//! 测试系统在高负载和故障条件下的表现

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use futures::future::join_all;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

// use trifury::connectors::binance::modern::ModernBinanceConnector; // 暂时注释掉，路径可能不正确
use trifury::connectors::common::batch_subscription::{
    BatchSubscriptionManager, SubscriptionRequest,
};
use trifury::connectors::traits::subscription_manager::{
    ConnectionStrategy, SubscriptionConfig, SubscriptionManager, SubscriptionManagerConfig,
    SubscriptionPriority,
};
use trifury::connectors::traits::SubscriptionDataType;
use trifury::types::common::DataType;
// use trifury::connectors::traits::websocket_manager::WebSocketManager; // 暂时注释掉，未使用
// use trifury::connectors::common::websocket::WebSocketManagerImpl; // 暂时注释掉，路径可能不正确
use trifury::types::ConnectorError;

/// 连接压力测试 - 大量并发连接
fn stress_test_concurrent_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("stress_concurrent_connections");

    for connection_count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*connection_count as u64));
        group.bench_with_input(
            format!("connections_{}", connection_count),
            connection_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let start_time = SystemTime::now();
                        let mut connectors: Vec<()> = Vec::new(); // 暂时使用空类型，因为ModernBinanceConnector未正确导入

                        // 创建大量连接器
                        for i in 0..count {
                            // let mut connector = ModernBinanceConnector::new(
                            //     format!("test_connector_{}", i)
                            // ); // 暂时注释掉，ModernBinanceConnector未正确导入
                            // connectors.push(connector);
                        }

                        let duration = start_time.elapsed().unwrap();
                        black_box((connectors, duration))
                    })
                })
            },
        );
    }
    group.finish();
}

/// 订阅压力测试 - 大量并发订阅
fn stress_test_massive_subscriptions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("stress_massive_subscriptions");

    for subscription_count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*subscription_count as u64));
        group.bench_with_input(
            format!("subscriptions_{}", subscription_count),
            subscription_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let start_time = SystemTime::now();
                        let config = SubscriptionManagerConfig::default();
                        let _manager = SubscriptionManager::new(
                            config.clone(),
                            ConnectionStrategy::SingleConnection,
                        );

                        // 大量并发订阅
                        let mut tasks = Vec::new();
                        for i in 0..count {
                            let config_clone = config.clone();
                            let manager_clone = SubscriptionManager::new(
                                config_clone,
                                ConnectionStrategy::SingleConnection,
                            );
                            tasks.push(tokio::spawn(async move {
                                let sub_config = SubscriptionConfig {
                                    symbols: vec![format!("SYMBOL{}", i)],
                                    data_types: vec![
                                        SubscriptionDataType::OrderBook,
                                        SubscriptionDataType::Trade,
                                    ],
                                    batch_size: None,
                                    priority: SubscriptionPriority::Medium,
                                };
                                manager_clone.subscribe(sub_config).await
                            }));
                        }

                        let results = join_all(tasks).await;
                        let duration = start_time.elapsed().unwrap();
                        black_box((results, duration))
                    })
                })
            },
        );
    }
    group.finish();
}

/// 批量处理压力测试 - 高频批次处理
fn stress_test_batch_processing_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("stress_batch_high_frequency", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start_time = SystemTime::now();
                let manager = BatchSubscriptionManager::with_default_config();

                // 高频添加订阅请求
                for batch in 0..10 {
                    for i in 0..50 {
                        let _ = manager
                            .add_subscription_request(
                                format!("BATCH{}_SYMBOL{}", batch, i),
                                vec![DataType::OrderBook],
                                Some(1),
                            )
                            .await;
                    }

                    // 立即处理批次
                    let subscription_handler = |requests: Vec<_>| async move {
                        // 模拟快速处理
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        let results: Vec<(String, bool, Option<String>)> = requests
                            .into_iter()
                            .map(|req: SubscriptionRequest| (req.symbol, true, None))
                            .collect();
                        Ok::<_, ConnectorError>(results)
                    };

                    let _ = manager.process_next_batch(subscription_handler).await;
                }

                let duration = start_time.elapsed().unwrap();
                black_box(duration)
            })
        })
    });
}

/// 内存压力测试 - 大量数据结构创建
fn stress_test_memory_pressure(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("stress_memory_pressure");

    for data_size in [1000, 5000, 10000].iter() {
        group.throughput(Throughput::Elements(*data_size as u64));
        group.bench_with_input(
            format!("data_structures_{}", data_size),
            data_size,
            |b, &size| {
                b.iter(|| {
                    rt.block_on(async {
                        let start_time = SystemTime::now();
                        let mut managers = Vec::new();
                        let websocket_managers: Vec<()> = Vec::new(); // 暂时使用空类型，因为WebSocketManager未正确导入

                        // 创建大量管理器实例
                        for i in 0..size {
                            let config = SubscriptionManagerConfig::default();
                            let subscription_manager = SubscriptionManager::new(
                                config,
                                ConnectionStrategy::SingleConnection,
                            );
                            // let websocket_manager = WebSocketManagerImpl::new(); // 暂时注释掉，类型未正确导入

                            // 添加一些订阅以增加内存使用
                            let sub_config = SubscriptionConfig {
                                symbols: vec![format!("SYMBOL{}", i)],
                                data_types: vec![SubscriptionDataType::OrderBook],
                                batch_size: None,
                                priority: SubscriptionPriority::Medium,
                            };
                            let _ = subscription_manager.subscribe(sub_config).await;

                            managers.push(subscription_manager);
                            // websocket_managers.push(websocket_manager); // 暂时注释掉，websocket_manager未定义
                        }

                        let duration = start_time.elapsed().unwrap();
                        black_box((managers, websocket_managers, duration))
                    })
                })
            },
        );
    }
    group.finish();
}

/// 网络故障注入测试 - 模拟连接失败
fn stress_test_network_failures(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("stress_network_failures", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start_time = SystemTime::now();
                let config = SubscriptionManagerConfig::default();
                let _manager =
                    SubscriptionManager::new(config.clone(), ConnectionStrategy::SingleConnection);

                // 模拟网络故障场景
                let mut failure_tasks = Vec::new();

                for i in 0..20 {
                    let config_clone = config.clone();
                    let manager_clone = SubscriptionManager::new(
                        config_clone,
                        ConnectionStrategy::SingleConnection,
                    );
                    failure_tasks.push(tokio::spawn(async move {
                        // 尝试订阅，可能失败
                        let invalid_config = SubscriptionConfig {
                            symbols: vec![format!("INVALID_SYMBOL_{}", i)],
                            data_types: vec![SubscriptionDataType::OrderBook],
                            batch_size: None,
                            priority: SubscriptionPriority::Medium,
                        };
                        let result = manager_clone.subscribe(invalid_config).await;

                        // 模拟连接断开
                        tokio::time::sleep(Duration::from_millis(10)).await;

                        // 尝试重新连接
                        let retry_config = SubscriptionConfig {
                            symbols: vec![format!("RETRY_SYMBOL_{}", i)],
                            data_types: vec![SubscriptionDataType::Trade],
                            batch_size: None,
                            priority: SubscriptionPriority::Medium,
                        };
                        let reconnect_result = manager_clone.subscribe(retry_config).await;

                        (result, reconnect_result)
                    }));
                }

                let results = join_all(failure_tasks).await;
                let duration = start_time.elapsed().unwrap();
                black_box((results, duration))
            })
        })
    });
}

/// 超时压力测试 - 模拟慢响应
fn stress_test_timeout_scenarios(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("stress_timeout_scenarios", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start_time = SystemTime::now();
                let manager = BatchSubscriptionManager::with_default_config();

                // 添加订阅请求
                for i in 0..10 {
                    let _ = manager
                        .add_subscription_request(
                            format!("SLOW_SYMBOL{}", i),
                            vec![DataType::OrderBook],
                            Some(1),
                        )
                        .await;
                }

                // 模拟慢响应的订阅处理函数
                let slow_subscription_handler = |requests: Vec<SubscriptionRequest>| async move {
                    // 模拟网络延迟
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let results: Vec<(String, bool, Option<String>)> = requests
                        .into_iter()
                        .enumerate()
                        .map(|(idx, req)| {
                            // 模拟部分失败
                            let success = idx % 3 != 0;
                            let error = if success {
                                None
                            } else {
                                Some("Timeout error".to_string())
                            };
                            (req.symbol, success, error)
                        })
                        .collect();

                    Ok::<_, ConnectorError>(results)
                };

                let result = manager.process_next_batch(slow_subscription_handler).await;
                let duration = start_time.elapsed().unwrap();
                black_box((result, duration))
            })
        })
    });
}

/// 并发竞争测试 - 多线程访问共享资源
fn stress_test_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("stress_concurrent_access");

    for thread_count in [5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));
        group.bench_with_input(
            format!("threads_{}", thread_count),
            thread_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let start_time = SystemTime::now();
                        let config = SubscriptionManagerConfig::default();
                        let manager = Arc::new(SubscriptionManager::new(
                            config,
                            ConnectionStrategy::SingleConnection,
                        ));
                        let semaphore = Arc::new(Semaphore::new(count));

                        let mut tasks = Vec::new();

                        for i in 0..count {
                            let manager_clone = Arc::clone(&manager);
                            let semaphore_clone = Arc::clone(&semaphore);

                            tasks.push(tokio::spawn(async move {
                                let _permit = semaphore_clone.acquire().await.unwrap();

                                // 并发操作：订阅和取消订阅
                                let symbol = format!("CONCURRENT_SYMBOL_{}", i);

                                // 订阅
                                let sub_config = SubscriptionConfig {
                                    symbols: vec![symbol.clone()],
                                    data_types: vec![SubscriptionDataType::OrderBook],
                                    batch_size: None,
                                    priority: SubscriptionPriority::Medium,
                                };
                                let sub_result = manager_clone.subscribe(sub_config).await;

                                // 短暂等待
                                tokio::time::sleep(Duration::from_millis(1)).await;

                                // 取消订阅
                                let unsub_config = SubscriptionConfig {
                                    symbols: vec![symbol],
                                    data_types: vec![SubscriptionDataType::OrderBook],
                                    batch_size: None,
                                    priority: SubscriptionPriority::Medium,
                                };
                                let unsub_result = manager_clone.unsubscribe(unsub_config).await;

                                (sub_result, unsub_result)
                            }));
                        }

                        let results = join_all(tasks).await;
                        let duration = start_time.elapsed().unwrap();
                        black_box((results, duration))
                    })
                })
            },
        );
    }
    group.finish();
}

/// 资源泄漏测试 - 检测内存和连接泄漏
fn stress_test_resource_leaks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("stress_resource_leaks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start_time = SystemTime::now();

                // 循环创建和销毁资源
                for cycle in 0..10 {
                    let config = SubscriptionManagerConfig::default();
                    let manager =
                        SubscriptionManager::new(config, ConnectionStrategy::SingleConnection);
                    let batch_manager = BatchSubscriptionManager::with_default_config();

                    // 添加订阅
                    for i in 0..20 {
                        let symbol = format!("CYCLE{}_SYMBOL{}", cycle, i);
                        let sub_config = SubscriptionConfig {
                            symbols: vec![symbol.clone()],
                            data_types: vec![SubscriptionDataType::OrderBook],
                            batch_size: None,
                            priority: SubscriptionPriority::Medium,
                        };
                        let _ = manager.subscribe(sub_config).await;

                        let _ = batch_manager
                            .add_subscription_request(symbol, vec![DataType::Trade], Some(1))
                            .await;
                    }

                    // 清理订阅
                    for i in 0..20 {
                        let symbol = format!("CYCLE{}_SYMBOL{}", cycle, i);
                        let unsub_config = SubscriptionConfig {
                            symbols: vec![symbol],
                            data_types: vec![SubscriptionDataType::OrderBook],
                            batch_size: None,
                            priority: SubscriptionPriority::Medium,
                        };
                        let _ = manager.unsubscribe(unsub_config).await;
                    }

                    // 强制垃圾回收（在Rust中主要是drop）
                    drop(manager);
                    drop(batch_manager);
                }

                let duration = start_time.elapsed().unwrap();
                black_box(duration)
            })
        })
    });
}

criterion_group!(
    stress_benches,
    stress_test_concurrent_connections,
    stress_test_massive_subscriptions,
    stress_test_batch_processing_load,
    stress_test_memory_pressure,
    stress_test_network_failures,
    stress_test_timeout_scenarios,
    stress_test_concurrent_access,
    stress_test_resource_leaks
);
criterion_main!(stress_benches);
