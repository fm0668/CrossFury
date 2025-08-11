//! Binance连接器演示程序
//! 
//! 演示如何使用Binance现货连接器连接到Binance WebSocket API
//! 并接收实时市场数据

use trifury::core::{AppState, OrderbookUpdate};
use trifury::connectors::binance::BinanceAdapter;
use trifury::connectors::binance::config::BinanceConfig;
use trifury::connectors::traits::ExchangeConnector;
use trifury::types::common::DataType;
use trifury::Exchange;
use trifury::types::ConnectionStatus;
use trifury::types::orders::{OrderSide, OrderType, TimeInForce, OrderRequest};
use trifury::types::ConnectorError;

use std::sync::Arc;
use tokio::sync::mpsc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    println!("🚀 启动Binance连接器演示程序");
    println!("{}", "=".repeat(50));
    
    // 创建应用状态
    let (orderbook_tx, mut orderbook_rx) = mpsc::channel::<OrderbookUpdate>(1000);
    let mut app_state = AppState::new();
    app_state.orderbook_queue = Some(orderbook_tx);
    let app_state = Arc::new(app_state);
    // 创建Binance配置（使用实盘）
    let config = BinanceConfig {
        api_key: None,
        secret_key: None,
        testnet: false, // 使用实盘URL
        rate_limit_per_minute: 1200,
    };
    
    println!("📋 配置信息:");
    println!("  - 交易所: Binance");
    println!("  - 市场类型: 现货");
    println!("  - 网络: 实盘");
    println!("  - 限流: {} 请求/分钟", config.rate_limit_per_minute);
    println!();
    
    // 创建Binance适配器
    println!("🔧 正在创建Binance适配器...");
    let adapter = match BinanceAdapter::new(config, app_state.clone()).await {
        Ok(adapter) => {
            println!("✅ Binance适配器创建成功");
            adapter
        },
        Err(e) => {
            eprintln!("❌ 创建Binance适配器失败: {e:?}");
            return Err(e.into());
        }
    };
    
    // 显示适配器信息
    println!("📊 适配器信息:");
    println!("  - 交易所类型: {:?}", adapter.get_exchange_type());
    println!("  - 市场类型: {:?}", adapter.get_market_type());
    let connection_status = adapter.get_connection_status().await;
    println!("  - 连接状态: {connection_status:?}");
    println!();
    
    // 先订阅市场数据（设置要订阅的流）
    println!("📈 正在设置市场数据订阅...");
    let symbols = vec![
        "BTCUSDT".to_string(),
        "ETHUSDT".to_string(),
        "ADAUSDT".to_string(),
    ];
    let data_types = vec![
        DataType::OrderBook,
        DataType::Trade,
    ];
    
    println!("  - 交易对: {symbols:?}");
    println!("  - 数据类型: {data_types:?}");
    
    match adapter.subscribe_market_data(symbols.clone(), data_types.clone()).await {
        Ok(_) => println!("✅ 市场数据订阅设置成功"),
        Err(e) => {
            eprintln!("❌ 市场数据订阅设置失败: {e:?}");
            return Err(e.into());
        }
    }
    
    // 连接WebSocket（现在会使用已订阅的流生成URL）
    println!();
    println!("🔌 正在连接Binance WebSocket...");
    match adapter.connect_websocket().await {
        Ok(_) => println!("✅ WebSocket连接成功"),
        Err(e) => {
            eprintln!("❌ WebSocket连接失败: {e:?}");
            return Err(e.into());
        }
    }
    
    // 等待连接稳定
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 检查连接状态
    let status = adapter.get_connection_status().await;
    println!("📡 当前连接状态: {status:?}");
    
    if !matches!(status, ConnectionStatus::Connected) {
        eprintln!("❌ 连接状态异常，退出程序");
        return Ok(());
    }
    
    // 启动数据接收任务
    let data_task = tokio::spawn(async move {
        let mut message_count = 0;
        let mut orderbook_count = 0;
        // let mut trade_count = 0; // 暂时不需要
        
        println!();
        println!("📊 开始接收市场数据...");
        println!("(按 Ctrl+C 停止程序)");
        println!("{}", "-".repeat(80));
        
        while let Some(orderbook) = orderbook_rx.recv().await {
            message_count += 1;
            orderbook_count += 1;
            
            let bids_count = orderbook.depth_bids.as_ref().map(|v| v.len()).unwrap_or(0);
             let asks_count = orderbook.depth_asks.as_ref().map(|v| v.len()).unwrap_or(0);
             
             println!(
                 "📖 [{}] 订单簿更新 - 交易对: {}, 买盘: {}, 卖盘: {}, 时间: {}",
                 orderbook_count,
                 orderbook.symbol,
                 bids_count,
                 asks_count,
                 orderbook.timestamp
             );
            
            // 显示最优买卖价
            println!(
                "    💰 最优买价: {}, 最优卖价: {}, 价差: {:.4}",
                orderbook.best_bid,
                orderbook.best_ask,
                orderbook.best_ask - orderbook.best_bid
            );
            
            // 每100条消息显示统计信息
            if message_count % 100 == 0 {
                println!();
                println!("📊 统计信息 (总计: {message_count} 条消息)");
                println!("  - 订单簿更新: {orderbook_count} 条");
                println!("{}", "-".repeat(80));
            }
        }
    });
    
    // 健康检查任务
    let health_adapter = adapter.clone();
    let health_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            match health_adapter.health_check().await {
                Ok(health_status) => {
                    println!("🏥 健康检查: {health_status:?}");
                    
                    // 获取连接统计
                    if let Ok(stats) = health_adapter.get_connection_stats().await {
                        println!("📈 连接统计: {stats:?}");
                    }
                },
                Err(e) => {
                    eprintln!("❌ 健康检查失败: {e:?}");
                }
            }
        }
    });
    
    // 等待用户中断
    println!();
    println!("⏳ 程序运行中，按 Ctrl+C 停止...");
    
    // 等待中断信号
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!();
            println!("🛑 收到中断信号，正在停止程序...");
        },
        _ = data_task => {
            println!("📊 数据接收任务结束");
        },
        _ = health_task => {
            println!("🏥 健康检查任务结束");
        }
    }
    
    // 取消订阅
    println!("📤 正在取消订阅...");
    if let Err(e) = adapter.unsubscribe_market_data(symbols, data_types).await {
        eprintln!("❌ 取消订阅失败: {e:?}");
    } else {
        println!("✅ 取消订阅成功");
    }
    
    // 断开连接
    println!("🔌 正在断开WebSocket连接...");
    if let Err(e) = adapter.disconnect_websocket().await {
        eprintln!("❌ 断开连接失败: {e:?}");
    } else {
        println!("✅ 连接断开成功");
    }
    
    // 最终状态检查
    let final_status = adapter.get_connection_status().await;
    println!("📡 最终连接状态: {final_status:?}");
    
    println!();
    println!("🎉 Binance连接器演示程序结束");
    println!("{}", "=".repeat(50));
    
    Ok(())
}

/// 演示交易功能（仅显示错误，不执行真实交易）
#[allow(dead_code)]
async fn demo_trading_features(adapter: &BinanceAdapter) {
    println!();
    println!("💼 演示交易功能（仅测试，不执行真实交易）");
    println!("{}", "-".repeat(50));
    
    // 测试下单功能
    let order_request = OrderRequest {
        symbol: "BTCUSDT".to_string(),
        exchange: Exchange::Binance.into(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        quantity: 0.001,
        price: Some(30000.0),
        time_in_force: Some(TimeInForce::GTC),
        client_order_id: Some("demo_order_001".to_string()),
        reduce_only: None,
        close_position: None,
        position_side: None,
    };
    
    println!("📝 测试下单功能...");
    match adapter.place_order(&order_request).await {
        Ok(response) => {
            println!("✅ 下单成功: {response:?}");
        },
        Err(ConnectorError::TradingNotImplemented) => {
            println!("ℹ️  交易功能未实现（符合预期）");
        },
        Err(e) => {
            println!("❌ 下单失败: {e:?}");
        }
    }
    
    // 测试取消订单功能
    println!("🚫 测试取消订单功能...");
    match adapter.cancel_order("demo_order_001", "BTCUSDT").await {
        Ok(response) => {
            println!("✅ 取消订单成功: {response:?}");
        },
        Err(ConnectorError::TradingNotImplemented) => {
            println!("ℹ️  交易功能未实现（符合预期）");
        },
        Err(e) => {
            println!("❌ 取消订单失败: {e:?}");
        }
    }
    
    // 测试账户信息功能
    println!("👤 测试账户信息功能...");
    match adapter.get_account_balance().await {
        Ok(account) => {
            println!("✅ 获取账户信息成功: {account:?}");
        },
        Err(ConnectorError::TradingNotImplemented) => {
            println!("ℹ️  交易功能未实现（符合预期）");
        },
        Err(e) => {
            println!("❌ 获取账户信息失败: {e:?}");
        }
    }
    
    println!("{}", "-".repeat(50));
}