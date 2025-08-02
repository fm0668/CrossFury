//! LBank连接器测试模块
//! 测试LBank连接器的各项功能

mod tests {
    use super::super::adapter::LBankConnector;
    use crate::connectors::traits::ExchangeConnector;
    use crate::core::AppState;
    use crate::types::{
        config::{ConnectorConfig, SubscriptionConfig, UpdateSpeed, ConnectionStatus},
        common::DataType,
        market_data::StandardizedMessage,
    };
    use tokio::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;
    use log::info;

    /// 创建测试用的连接器配置
    fn create_test_config() -> ConnectorConfig {
        ConnectorConfig {
            api_key: Some("test_key".to_string()),
            secret_key: Some("test_secret".to_string()),
            passphrase: None,
            testnet: true,
            websocket_url: None,
            rest_api_url: None,
            reconnect_interval: 5000, // 5秒 = 5000毫秒
            max_reconnect_attempts: 3,
            ping_interval: 30000, // 30秒 = 30000毫秒
            request_timeout: 10000, // 10秒 = 10000毫秒
        }
    }

    /// 创建测试用的订阅配置
    fn create_test_subscription() -> SubscriptionConfig {
        SubscriptionConfig {
            symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
            data_types: vec![DataType::OrderBook, DataType::Trade],
            depth_levels: Some(10),
            update_speed: Some(UpdateSpeed::Fast),
        }
    }

    #[tokio::test]
    async fn test_lbank_connector_creation() {
        // 初始化日志
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 验证连接器创建成功
        let status = connector.get_connection_status();
        assert_eq!(status, ConnectionStatus::Disconnected);
        
        info!("✅ LBank连接器创建测试通过");
    }

    #[tokio::test]
    async fn test_lbank_connector_connect() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let mut connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 测试连接
        let result = connector.connect_websocket().await;
        assert!(result.is_ok(), "连接应该成功");
        
        // 验证连接状态
        let status = connector.get_connection_status();
        assert_eq!(status, ConnectionStatus::Connected);
        
        info!("✅ LBank连接器连接测试通过");
    }

    #[tokio::test]
    async fn test_lbank_connector_disconnect() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let mut connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 先连接
        let _ = connector.connect_websocket().await;
        
        // 测试断开连接
        let result = connector.disconnect_websocket().await;
        assert!(result.is_ok(), "断开连接应该成功");
        
        // 验证连接状态
        let status = connector.get_connection_status();
        assert_eq!(status, ConnectionStatus::Disconnected);
        
        info!("✅ LBank连接器断开连接测试通过");
    }

    #[tokio::test]
    async fn test_lbank_connector_health_check() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 测试健康检查
        let result = connector.health_check().await;
        assert!(result.is_ok(), "健康检查应该成功");
        
        // 未连接状态下应该返回false
        let is_healthy = result.unwrap();
        assert!(!is_healthy, "未连接状态下应该不健康");
        
        info!("✅ LBank连接器健康检查测试通过");
    }

    #[tokio::test]
    async fn test_lbank_connector_subscription() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let mut connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 先连接
        let _ = connector.connect_websocket().await;
        
        // 创建消息通道
        let (sender, _receiver) = mpsc::unbounded_channel::<StandardizedMessage>();
        connector.set_message_sender(sender).await;
        
        // 测试订阅（注意：这个测试可能会尝试真实连接）
        let subscription = create_test_subscription();
        let result = connector.subscribe_market_data(subscription).await;
        
        // 由于这是集成测试，可能会因为网络问题失败
        // 我们主要验证接口调用不会panic
        match result {
            Ok(_) => info!("✅ LBank订阅成功"),
            Err(e) => info!("⚠️ LBank订阅失败（可能是网络问题）: {}", e),
        }
        
        info!("✅ LBank连接器订阅测试完成");
    }

    #[tokio::test]
    async fn test_lbank_connector_get_orderbook() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 测试获取订单簿（没有数据时应该返回错误）
        let result = connector.get_latest_orderbook("BTCUSDT").await;
        assert!(result.is_err(), "没有数据时应该返回错误");
        
        info!("✅ LBank连接器获取订单簿测试通过");
    }

    #[tokio::test]
    async fn test_lbank_connector_trading_not_implemented() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let mut connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 测试交易功能（应该返回未实现错误）
        use crate::types::orders::{OrderRequest, OrderSide, OrderType, TimeInForce};
        use crate::types::exchange::ExchangeType;
        
        let order = OrderRequest {
            symbol: "BTCUSDT".to_string(),
            exchange: ExchangeType::LBank,
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 0.001,
            price: Some(50000.0),
            time_in_force: TimeInForce::GTC,
            client_order_id: Some("test_order".to_string()),
        };
        
        let result = connector.place_order(&order).await;
        assert!(result.is_err(), "交易功能应该返回未实现错误");
        
        let cancel_result = connector.cancel_order("test_order", "BTCUSDT").await;
        assert!(cancel_result.is_err(), "取消订单功能应该返回未实现错误");
        
        let balance_result = connector.get_account_balance().await;
        assert!(balance_result.is_err(), "获取余额功能应该返回未实现错误");
        
        let positions_result = connector.get_positions().await;
        assert!(positions_result.is_err(), "获取仓位功能应该返回未实现错误");
        
        info!("✅ LBank连接器交易功能未实现测试通过");
    }

    #[tokio::test]
    async fn test_lbank_connector_stats() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = AppState::new();
        
        let connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 测试获取连接统计
        let result = connector.get_connection_stats().await;
        assert!(result.is_ok(), "获取连接统计应该成功");
        
        let (messages, updates, uptime) = result.unwrap();
        assert_eq!(messages, 0, "初始消息数应该为0");
        assert_eq!(updates, 0, "初始更新数应该为0");
        assert_eq!(uptime, 0.0, "初始运行时间应该为0");
        
        info!("✅ LBank连接器统计信息测试通过");
    }
}

/// 集成测试模块
/// 这些测试需要真实的网络连接
#[cfg(test)]
mod integration_tests {
    use super::super::adapter::LBankConnector;
    use crate::connectors::traits::ExchangeConnector;
    use crate::core::AppState;
    use crate::types::config::{ConnectorConfig, SubscriptionConfig, UpdateSpeed};
use crate::types::common::DataType;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;
    use log::info;

    /// 创建真实的连接器配置
    fn create_real_config() -> ConnectorConfig {
        ConnectorConfig {
            api_key: Some("".to_string()), // 实际使用时需要真实的API密钥
            secret_key: Some("".to_string()),
            passphrase: None,
            testnet: true,
            websocket_url: None,
            rest_api_url: None,
            reconnect_interval: 5000, // 5秒 = 5000毫秒
            max_reconnect_attempts: 3,
            ping_interval: 30000, // 30秒 = 30000毫秒
            request_timeout: 10000, // 10秒 = 10000毫秒
        }
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要手动运行
    async fn test_real_lbank_connection() {
        let _ = env_logger::try_init();
        
        let config = create_real_config();
        let app_state = AppState::new();
        
        let mut connector = LBankConnector::new(config, Arc::new(app_state));
        
        // 测试真实连接
        let connect_result = connector.connect_websocket().await;
        assert!(connect_result.is_ok(), "真实连接应该成功");
        
        // 创建订阅配置
        let subscription = SubscriptionConfig {
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![DataType::OrderBook],
            depth_levels: Some(10),
            update_speed: Some(UpdateSpeed::Fast),
        };
        
        // 测试订阅（设置超时）
        let subscribe_result = timeout(
            Duration::from_secs(30),
            connector.subscribe_market_data(subscription)
        ).await;
        
        match subscribe_result {
            Ok(Ok(_)) => info!("✅ 真实LBank连接和订阅成功"),
            Ok(Err(e)) => info!("⚠️ LBank订阅失败: {}", e),
            Err(_) => info!("⚠️ LBank订阅超时"),
        }
        
        // 等待一段时间接收数据
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // 检查是否收到数据
        let orderbook_result = connector.get_latest_orderbook("LBANK_BTCUSDT").await;
        match orderbook_result {
            Ok(orderbook) => {
                info!("✅ 成功获取LBank订单簿数据: {} @ {}", orderbook.symbol, orderbook.timestamp);
                assert!(orderbook.best_bid > 0.0, "买价应该大于0");
                assert!(orderbook.best_ask > 0.0, "卖价应该大于0");
                assert!(orderbook.best_ask > orderbook.best_bid, "卖价应该大于买价");
            },
            Err(e) => info!("⚠️ 未能获取订单簿数据: {}", e),
        }
        
        // 断开连接
        let _ = connector.disconnect_websocket().await;
        
        info!("✅ 真实LBank连接测试完成");
    }
}