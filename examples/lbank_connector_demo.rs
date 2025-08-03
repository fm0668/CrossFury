//! LBank连接器演示程序
//! 展示如何使用重构后的LBank连接器获取市场数据

use trifury::{
    connectors::{
        traits::ExchangeConnector,
        lbank::LBankConnector,
    },
    core::AppState,
    types::{
        config::{ConnectorConfig, SubscriptionConfig, UpdateSpeed},
        common::DataType,
    },
};
use std::{sync::Arc, time::Duration};
use tokio::time::{sleep, timeout};
use log::{info, error, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    info!("🚀 启动LBank连接器演示程序");
    
    // 创建应用状态
    let app_state = Arc::new(AppState::new());
    
    // 创建连接器配置
    let config = ConnectorConfig {
        api_key: Some("".to_string()), // 演示程序不需要API密钥
        secret_key: Some("".to_string()),
        passphrase: None,
        testnet: true,
        websocket_url: Some("wss://www.lbkex.net/ws/V2/".to_string()),
        rest_api_url: Some("https://api.lbkex.com".to_string()),
        reconnect_interval: 5000,
        max_reconnect_attempts: 3,
        ping_interval: 30000,
        request_timeout: 10000,
    };
    
    // 创建LBank连接器
    let mut connector = LBankConnector::new(config, app_state);
    
    info!("📡 连接到LBank交易所...");
    
    // 连接到交易所
    match connector.connect_websocket().await {
        Ok(_) => info!("✅ 成功连接到LBank交易所"),
        Err(e) => {
            error!("❌ 连接失败: {e}");
            return Err(e.into());
        }
    }
    
    // 检查连接状态
    let status = connector.get_connection_status();
    info!("🔗 连接状态: {status:?}");
    
    // 创建订阅配置
    let subscription = SubscriptionConfig {
        symbols: vec![
            "BTCUSDT".to_string(),
            "ETHUSDT".to_string(),
            "BNBUSDT".to_string(),
        ],
        data_types: vec![DataType::OrderBook, DataType::Trade],
        depth_levels: Some(10),
        update_speed: Some(UpdateSpeed::Fast),
    };
    
    info!("📊 订阅市场数据: {:?}", subscription.symbols);
    
    // 订阅市场数据（设置超时）
    let subscribe_result = timeout(
        Duration::from_secs(30),
        connector.subscribe_market_data(subscription)
    ).await;
    
    match subscribe_result {
        Ok(Ok(_)) => info!("✅ 成功订阅市场数据"),
        Ok(Err(e)) => {
            warn!("⚠️ 订阅失败: {e}");
            // 继续运行，可能是网络问题
        },
        Err(_) => {
            warn!("⚠️ 订阅超时");
            // 继续运行
        }
    }
    
    // 等待数据接收
    info!("⏳ 等待市场数据...");
    sleep(Duration::from_secs(10)).await;
    
    // 尝试获取订单簿数据
    let symbols_to_check = vec![
        "LBANK_BTCUSDT",
        "LBANK_ETHUSDT", 
        "LBANK_BNBUSDT"
    ];
    
    for symbol in symbols_to_check {
        if let Some(orderbook) = connector.get_orderbook_snapshot(symbol) {
            info!("📈 {symbol} 订单簿数据:");
            info!("   最佳买价: {:.8}", orderbook.best_bid);
            info!("   最佳卖价: {:.8}", orderbook.best_ask);
            info!("   价差: {:.8}", orderbook.best_ask - orderbook.best_bid);
            info!("   时间戳: {}", orderbook.timestamp);
            info!("   深度数据: {} 档买单, {} 档卖单", 
                  orderbook.depth_bids.len(), 
                  orderbook.depth_asks.len());
            
            // 显示前3档深度
            if !orderbook.depth_bids.is_empty() {
                info!("   买单深度 (前3档):");
                for (i, (price, qty)) in orderbook.depth_bids.iter().take(3).enumerate() {
                    info!("     {}. {:.8} @ {:.8}", i + 1, price, qty);
                }
            }
            
            if !orderbook.depth_asks.is_empty() {
                info!("   卖单深度 (前3档):");
                for (i, (price, qty)) in orderbook.depth_asks.iter().take(3).enumerate() {
                    info!("     {}. {:.8} @ {:.8}", i + 1, price, qty);
                }
            }
        } else {
            warn!("⚠️ 无法获取 {symbol} 的订单簿数据");
        }
    }
    
    // 获取连接统计信息
    match connector.get_connection_stats().await {
        Ok((messages, updates, uptime)) => {
            info!("📊 连接统计:");
            info!("   WebSocket消息数: {messages}");
            info!("   价格更新数: {updates}");
            info!("   运行时间: {uptime:.2}秒");
        },
        Err(e) => warn!("⚠️ 无法获取连接统计: {e}"),
    }
    
    // 健康检查
    match connector.health_check().await {
        Ok(is_healthy) => {
            if is_healthy {
                info!("💚 连接健康状态: 良好");
            } else {
                warn!("💛 连接健康状态: 异常");
            }
        },
        Err(e) => error!("❌ 健康检查失败: {e}"),
    }
    
    // 测试交易功能（应该返回未实现错误）
    info!("🧪 测试交易功能（预期失败）...");
    
    use trifury::types::orders::{OrderRequest, OrderSide, OrderType, TimeInForce};
    use trifury::types::exchange::ExchangeType;
    
    let test_order = OrderRequest {
        symbol: "BTCUSDT".to_string(),
        exchange: ExchangeType::LBank,
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: 0.001,
        price: None,
        time_in_force: TimeInForce::IOC,
        client_order_id: Some("demo_order".to_string()),
    };
    
    match connector.place_order(&test_order).await {
        Ok(_) => warn!("⚠️ 意外成功: 交易功能不应该被实现"),
        Err(e) => info!("✅ 预期错误: {e}"),
    }
    
    // 断开连接
    info!("🔌 断开连接...");
    match connector.disconnect_websocket().await {
        Ok(_) => info!("✅ 成功断开连接"),
        Err(e) => error!("❌ 断开连接失败: {e}"),
    }
    
    // 最终状态检查
    let final_status = connector.get_connection_status();
    info!("🏁 最终连接状态: {final_status:?}");
    
    info!("🎉 LBank连接器演示程序完成");
    
    Ok(())
}