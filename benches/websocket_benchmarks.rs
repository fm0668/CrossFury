use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use futures;
use std::time::Duration;
use tokio::runtime::Runtime;
use trifury::connectors::traits::websocket_manager::{
    ConnectionStrategy, StreamType, WebSocketEvent, WebSocketManager, WebSocketManagerConfig,
};
use uuid;

/// WebSocket管理器连接建立基准测试
fn websocket_connection_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("websocket_single_connection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = WebSocketManagerConfig {
                    max_connections: 100,
                    connection_timeout: Duration::from_secs(10),
                    ..Default::default()
                };
                let manager = WebSocketManager::new(config);
                manager.start().await.unwrap();

                let connection_id = format!("test_conn_{}", uuid::Uuid::new_v4());
                let result = manager
                    .connect(
                        connection_id.clone(),
                        "wss://stream.binance.com:9443/ws/btcusdt@ticker".to_string(),
                        vec![StreamType::Ticker],
                    )
                    .await;

                black_box(result);
                manager.stop().await.unwrap();
            })
        })
    });
}

/// WebSocket管理器批量连接基准测试
fn websocket_batch_connections_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("websocket_batch_connections");

    for connection_count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*connection_count as u64));
        group.bench_with_input(
            format!("connections_{}", connection_count),
            connection_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = WebSocketManagerConfig {
                            max_connections: count as usize * 2,
                            connection_timeout: Duration::from_secs(10),
                            ..Default::default()
                        };
                        let manager = WebSocketManager::new(config);
                        manager.start().await.unwrap();

                        let mut tasks = Vec::new();
                        for i in 0..count {
                            let manager_clone = &manager;
                            let connection_id = format!("batch_conn_{}", i);
                            let url =
                                format!("wss://stream.binance.com:9443/ws/btcusdt@ticker_{}", i);

                            tasks.push(async move {
                                manager_clone
                                    .connect(connection_id, url, vec![StreamType::Ticker])
                                    .await
                            });
                        }

                        let results = futures::future::join_all(tasks).await;
                        black_box(results);

                        manager.stop().await.unwrap();
                    })
                })
            },
        );
    }
    group.finish();
}

/// WebSocket管理器消息处理基准测试
fn websocket_message_processing_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("websocket_message_processing", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = WebSocketManagerConfig {
                    max_connections: 10,
                    ..Default::default()
                };
                let manager = WebSocketManager::new(config);
                let mut event_rx = manager.subscribe_events();

                manager.start().await.unwrap();

                let connection_id = "msg_test_conn".to_string();
                manager
                    .connect(
                        connection_id.clone(),
                        "wss://stream.binance.com:9443/ws/btcusdt@ticker".to_string(),
                        vec![StreamType::Ticker],
                    )
                    .await
                    .unwrap();

                // 等待连接建立
                tokio::time::sleep(Duration::from_millis(150)).await;

                // 发送测试消息
                let test_message =
                    r#"{"stream":"btcusdt@ticker","data":{"s":"BTCUSDT","c":"50000.00"}}"#;
                let result = manager
                    .send_message(&connection_id, test_message.to_string())
                    .await;

                black_box(result);
                manager.stop().await.unwrap();
            })
        })
    });
}

/// WebSocket管理器连接策略基准测试
fn websocket_connection_strategy_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("websocket_connection_strategies");

    for strategy in [
        ConnectionStrategy::Multiplexed,
        ConnectionStrategy::Independent,
        ConnectionStrategy::Hybrid,
    ]
    .iter()
    {
        group.bench_with_input(
            format!("strategy_{:?}", strategy),
            strategy,
            |b, strategy| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = WebSocketManagerConfig {
                            strategy: strategy.clone(),
                            max_connections: 50,
                            max_streams_per_connection: 100,
                            ..Default::default()
                        };
                        let manager = WebSocketManager::new(config);
                        manager.start().await.unwrap();

                        // 创建多个不同类型的数据流连接
                        let stream_types = vec![
                            StreamType::OrderBook,
                            StreamType::Trade,
                            StreamType::Ticker,
                            StreamType::Kline,
                        ];

                        let mut tasks = Vec::new();
                        for (i, stream_type) in stream_types.iter().enumerate() {
                            let manager_clone = &manager;
                            let connection_id =
                                format!("strategy_conn_{}_{}", i, format!("{:?}", stream_type));
                            let url = format!(
                                "wss://stream.binance.com:9443/ws/btcusdt@{:?}",
                                stream_type
                            );

                            tasks.push(async move {
                                manager_clone
                                    .connect(connection_id, url, vec![stream_type.clone()])
                                    .await
                            });
                        }

                        let results = futures::future::join_all(tasks).await;
                        black_box(results);

                        manager.stop().await.unwrap();
                    })
                })
            },
        );
    }
    group.finish();
}

/// WebSocket管理器质量监控基准测试
fn websocket_quality_monitoring_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("websocket_quality_monitoring", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = WebSocketManagerConfig {
                    quality_check_interval: Duration::from_millis(100),
                    max_connections: 20,
                    ..Default::default()
                };
                let manager = WebSocketManager::new(config);
                manager.start().await.unwrap();

                // 创建多个连接
                let mut connection_ids = Vec::new();
                for i in 0..5 {
                    let connection_id = format!("quality_conn_{}", i);
                    manager
                        .connect(
                            connection_id.clone(),
                            format!("wss://stream.binance.com:9443/ws/btcusdt@ticker_{}", i),
                            vec![StreamType::Ticker],
                        )
                        .await
                        .unwrap();
                    connection_ids.push(connection_id);
                }

                // 等待质量监控运行
                tokio::time::sleep(Duration::from_millis(200)).await;

                // 获取所有连接的质量信息
                let mut qualities = Vec::new();
                for connection_id in &connection_ids {
                    if let Some(quality) = manager.get_connection_quality(connection_id).await {
                        qualities.push(quality);
                    }
                }

                black_box(qualities);
                manager.stop().await.unwrap();
            })
        })
    });
}

/// WebSocket管理器并发连接基准测试
fn websocket_concurrent_operations_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("websocket_concurrent_operations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = WebSocketManagerConfig {
                    max_connections: 100,
                    connection_timeout: Duration::from_secs(5),
                    ..Default::default()
                };
                let manager = WebSocketManager::new(config);
                manager.start().await.unwrap();

                // 并发执行连接、断开、状态查询操作
                let connect_tasks: Vec<_> = (0..20)
                    .map(|i| {
                        let manager_ref = &manager;
                        async move {
                            let connection_id = format!("concurrent_conn_{}", i);
                            manager_ref
                                .connect(
                                    connection_id,
                                    format!(
                                        "wss://stream.binance.com:9443/ws/btcusdt@ticker_{}",
                                        i
                                    ),
                                    vec![StreamType::Ticker],
                                )
                                .await
                        }
                    })
                    .collect();

                let status_tasks: Vec<_> = (0..10)
                    .map(|i| {
                        let manager_ref = &manager;
                        async move {
                            let connection_id = format!("concurrent_conn_{}", i);
                            manager_ref.get_connection_quality(&connection_id).await
                        }
                    })
                    .collect();

                let (connect_results, status_results) = tokio::join!(
                    futures::future::join_all(connect_tasks),
                    futures::future::join_all(status_tasks)
                );

                black_box((connect_results, status_results));
                manager.stop().await.unwrap();
            })
        })
    });
}

criterion_group!(
    benches,
    websocket_connection_benchmark,
    websocket_batch_connections_benchmark,
    websocket_message_processing_benchmark,
    websocket_connection_strategy_benchmark,
    websocket_quality_monitoring_benchmark,
    websocket_concurrent_operations_benchmark
);
criterion_main!(benches);
