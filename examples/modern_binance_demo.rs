//! 现代化Binance连接器演示程序
//! 
//! 演示如何使用现代化的ModernBinanceConnector
//! 展示完整的生命周期管理、订阅管理和错误处理

use std::time::Duration;
use tokio::time::sleep;
use trifury::connectors::traits::modern::*;
use trifury::connectors::traits::modern_binance::*;
use trifury::connectors::traits::subscription_manager::{SubscriptionConfig, DataType, SubscriptionPriority};
use trifury::types::events::SystemEvent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    println!("🚀 启动现代化Binance连接器演示");
    
    // 1. 创建配置
    let config = ModernBinanceConfig {
        api_key: None, // 演示模式不需要API密钥
        secret_key: None,
        testnet: true,
        websocket_url: "wss://testnet.binance.vision/ws".to_string(),
        rest_api_url: "https://testnet.binance.vision".to_string(),
        connection_timeout: Duration::from_secs(30),
        reconnect_interval: Duration::from_secs(5),
        max_reconnect_attempts: 5,
        heartbeat_interval: Duration::from_secs(30),
        batch_size: 10,
        channel_buffer_size: 1000,
    };
    
    // 2. 验证配置
    println!("📋 验证配置...");
    config.validate().map_err(|e| format!("配置验证失败: {e}"))?;
    println!("✅ 配置验证通过");
    
    // 3. 创建重连策略
    let reconnect_strategy = ReconnectStrategy::ExponentialBackoff {
        initial_interval: Duration::from_secs(1),
        max_interval: Duration::from_secs(60),
        multiplier: 2.0,
        max_attempts: Some(5),
    };
    
    // 4. 使用构建器创建连接器
    println!("🔧 创建现代化连接器...");
    let mut connector = ModernBinanceConnectorBuilder::new()
        .with_config(config)
        .with_reconnect_strategy(reconnect_strategy)
        .build()?;
    
    println!("✅ 连接器创建成功");
    println!("📊 连接器信息:");
    println!("   - 名称: {}", connector.name());
    println!("   - 交易所类型: {:?}", connector.exchange_type());
    println!("   - 市场类型: {:?}", connector.market_type());
    println!("   - 支持功能: {:?}", connector.supported_features());
    
    // 5. 初始化连接器
    println!("🔄 初始化连接器...");
    connector.initialize(connector.config().clone()).await?;
    println!("✅ 连接器初始化完成");
    
    // 6. 启动连接器
    println!("🚀 启动连接器...");
    connector.start().await?;
    println!("✅ 连接器启动成功");
    
    // 7. 检查连接状态
    let status = connector.connection_status().await;
    println!("🔗 连接状态: {status:?}");
    
    // 8. 订阅市场数据
    println!("📡 订阅市场数据...");
    let subscription_config = SubscriptionConfig {
        symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        data_types: vec![DataType::OrderBook, DataType::Trade],
        batch_size: Some(10),
        priority: SubscriptionPriority::High,
    };
    
    match connector.subscribe(subscription_config).await {
        Ok(_) => println!("✅ 订阅成功"),
        Err(e) => println!("❌ 订阅失败: {e}"),
    }
    
    // 9. 获取事件流并处理事件
    println!("📨 开始监听事件...");
    let mut event_stream = connector.event_stream();
    
    // 启动事件处理任务
    let event_handler = tokio::spawn(async move {
        let mut event_count = 0;
        while let Ok(event) = event_stream.recv().await {
            event_count += 1;
            match event {
                SystemEvent::ConnectorInitialized { connector_id, .. } => {
                    println!("🎉 连接器已初始化: {connector_id}");
                },
                SystemEvent::ConnectorConnected { connector_id, .. } => {
                    println!("🔗 连接器已连接: {connector_id}");
                },
                SystemEvent::ConnectorDisconnected { connector_id, .. } => {
                    println!("🔌 连接器已断开: {connector_id}");
                },
                SystemEvent::Subscription { exchange, market_type, symbol, subscribed, .. } => {
                    println!("📡 订阅更新 [{:?}-{:?}]: {} -> {}", exchange, market_type, symbol, if subscribed { "已订阅" } else { "已取消订阅" });
                },
                _ => {
                    if event_count <= 10 {
                        println!("📨 收到事件 #{event_count}: {event:?}");
                    }
                }
            }
            
            // 限制事件处理数量以避免输出过多
            if event_count >= 20 {
                println!("📊 已处理 {event_count} 个事件，停止显示详细信息");
                break;
            }
        }
    });
    
    // 10. 监控连接器健康状态
    let health_monitor = tokio::spawn(async move {
        for i in 1..=3 {
            sleep(Duration::from_secs(10)).await;
            
            match connector.health_check().await {
                Ok(health) => {
                    println!("🏥 健康检查 #{i}: {health:?}");
                    if !health.healthy {
                        println!("⚠️  连接器不健康: {:?}", health.errors);
                    }
                },
                Err(e) => {
                    println!("❌ 健康检查失败: {e}");
                }
            }
            
            // 获取指标
            let metrics = connector.metrics().await;
            println!("📊 连接器指标: {metrics:?}");
            
            // 获取连接质量
            match connector.connection_quality().await {
                Ok(quality) => {
                    println!("📶 连接质量: 延迟 {:.2}ms, 稳定性 {:.2}", 
                        quality.latency_ms, quality.stability_score);
                },
                Err(e) => {
                    println!("❌ 获取连接质量失败: {e}");
                }
            }
        }
        
        // 测试订阅状态
        let subscription_status = connector.subscription_status().await;
        println!("📡 订阅状态: {subscription_status:?}");
        
        // 优雅关闭
        println!("🛑 开始优雅关闭...");
        
        if let Err(e) = connector.stop().await {
            println!("❌ 停止连接器失败: {e}");
        } else {
            println!("✅ 连接器已停止");
        }
        
        if let Err(e) = connector.shutdown().await {
            println!("❌ 关闭连接器失败: {e}");
        } else {
            println!("✅ 连接器已关闭");
        }
    });
    
    // 等待任务完成
    let _ = tokio::try_join!(event_handler, health_monitor);
    
    println!("🎯 现代化Binance连接器演示完成");
    
    Ok(())
}