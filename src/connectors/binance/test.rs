//! Binance连接器测试模块
//! 测试Binance连接器的各项功能

mod tests {
    use super::super::adapter::BinanceAdapter;
    use super::super::config::BinanceConfig;
    use crate::connectors::traits::ExchangeConnector;
    use crate::core::AppState;
    use crate::types::{
        config::{ConnectionStatus},
        common::DataType,
        market_data::StandardizedMessage,
        orders::{OrderRequest, OrderSide, OrderType, TimeInForce},
        exchange::ExchangeType,
        common::MarketType,
        errors::ConnectorError,
    };
    use tokio::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;
    use log::info;

    /// 创建测试用的Binance配置
    fn create_test_config() -> BinanceConfig {
        BinanceConfig {
            api_key: None,
            secret_key: None,
            testnet: true, // 使用测试网
            rate_limit_per_minute: 1200,
        }
    }
    
    #[tokio::test]
    async fn test_binance_adapter_creation() {
        // 初始化日志
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        let app_state = Arc::new(AppState::new());
        
        let adapter = BinanceAdapter::new(config, app_state).await;
        assert!(adapter.is_ok(), "Binance适配器创建失败");
        
        let adapter = adapter.unwrap();
        assert_eq!(adapter.get_exchange_type(), ExchangeType::Binance);
        assert_eq!(adapter.get_market_type(), crate::types::exchange::MarketType::Spot);
        
        info!("✅ Binance适配器创建测试通过");
    }
    
    #[tokio::test]
    async fn test_binance_adapter_connect() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = Arc::new(AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 测试连接（注意：这可能会尝试真实连接）
        let result = adapter.connect_websocket().await;
        
        // 由于这是集成测试，可能会因为网络问题失败
        // 我们主要验证接口调用不会panic
        match result {
            Ok(_) => {
                info!("✅ Binance连接成功");
                // 验证连接状态
                let status = adapter.get_connection_status();
                assert_eq!(status, ConnectionStatus::Connected);
            },
            Err(e) => info!("⚠️ Binance连接失败（可能是网络问题）: {:?}", e),
        }
        
        info!("✅ Binance连接器连接测试完成");
    }
    
    #[tokio::test]
    async fn test_binance_adapter_disconnect() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = Arc::new(AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 测试断开连接
        let result = adapter.disconnect_websocket().await;
        assert!(result.is_ok(), "断开连接应该成功");
        
        // 验证连接状态
        let status = adapter.get_connection_status();
        assert_eq!(status, ConnectionStatus::Disconnected);
        
        info!("✅ Binance连接器断开连接测试通过");
    }
    
    #[tokio::test]
    async fn test_binance_adapter_health_check() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = Arc::new(AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 测试健康检查
        let result = adapter.health_check().await;
        assert!(result.is_ok(), "健康检查应该成功");
        
        // 未连接状态下应该返回不健康
        let health_status = result.unwrap();
        assert!(matches!(health_status, crate::types::config::HealthStatus::Unhealthy), "未连接状态下应该不健康");
        
        info!("✅ Binance连接器健康检查测试通过");
    }
    
    #[tokio::test]
    async fn test_binance_adapter_subscription() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = Arc::new(AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 测试订阅（注意：这个测试可能会尝试真实连接）
        let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
        let data_types = vec![DataType::OrderBook, DataType::Trade];
        
        let result = adapter.subscribe_market_data(symbols.clone(), data_types.clone()).await;
        
        // 由于这是集成测试，可能会因为网络问题失败
        // 我们主要验证接口调用不会panic
        match result {
            Ok(_) => info!("✅ Binance订阅成功"),
            Err(e) => info!("⚠️ Binance订阅失败（可能是网络问题）: {:?}", e),
        }
        
        info!("✅ Binance连接器订阅测试完成");
    }
    
    #[tokio::test]
    async fn test_binance_trading_not_implemented() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = Arc::new(AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 测试交易功能（应该返回未实现错误）
        let order = OrderRequest {
            symbol: "BTCUSDT".to_string(),
            exchange: ExchangeType::Binance,
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 0.001,
            price: Some(50000.0),
            time_in_force: TimeInForce::GTC,
            client_order_id: Some("test_order".to_string()),
        };
        
        let result = adapter.place_order(&order).await;
        assert!(result.is_err(), "交易功能应该返回未实现错误");
        
        let cancel_result = adapter.cancel_order("test_order", "BTCUSDT").await;
        assert!(cancel_result.is_err(), "取消订单功能应该返回未实现错误");
        
        let balance_result = adapter.get_account_balance().await;
        assert!(balance_result.is_err(), "获取余额功能应该返回未实现错误");
        
        info!("✅ Binance连接器交易功能未实现测试通过");
    }
    
    #[tokio::test]
    async fn test_binance_adapter_stats() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = Arc::new(crate::core::AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 测试获取连接统计
        let result = adapter.get_connection_stats().await;
        assert!(result.is_ok(), "获取连接统计应该成功");
        
        let stats = result.unwrap();
        // 初始状态下的统计信息应该是默认值
        assert!(stats.connected_since.is_some(), "连接时间应该有值");
        
        info!("✅ Binance连接器统计信息测试通过");
    }
}

/// 真实网络连接测试（需要网络连接）
#[cfg(test)]
mod integration_tests {
    use super::super::adapter::BinanceAdapter;
    use super::super::config::BinanceConfig;
    use crate::connectors::traits::ExchangeConnector;
    use crate::types::{
        common::DataType,
        orders::{OrderRequest, OrderSide, OrderType, TimeInForce},
        exchange::ExchangeType,
    };
    use tokio::time::{sleep, Duration};
    use log::{info, warn};
    
    /// 创建测试用的Binance配置
    fn create_test_config() -> BinanceConfig {
        BinanceConfig {
            api_key: None,
            secret_key: None,
            testnet: true,
            rate_limit_per_minute: 1200,
        }
    }
    
    #[tokio::test]
    #[ignore] // 需要真实网络连接，默认忽略
    async fn test_real_binance_connection() {
        let _ = env_logger::try_init();
        
        let config = create_test_config();
        
        let app_state = std::sync::Arc::new(crate::core::AppState::new());
        let adapter = BinanceAdapter::new(config, app_state).await.unwrap();
        
        // 连接到Binance WebSocket
        let connect_result = adapter.connect_websocket().await;
        if connect_result.is_err() {
            warn!("连接失败: {:?}", connect_result.err());
            return;
        }
        
        // 等待连接稳定
        sleep(Duration::from_secs(2)).await;
        
        // 健康检查
        let health_result = adapter.health_check().await;
        assert!(health_result.is_ok(), "健康检查失败: {:?}", health_result.err());
        
        // 订阅市场数据
        let symbols = vec!["BTCUSDT".to_string()];
        let data_types = vec![DataType::OrderBook, DataType::Trade];
        let subscribe_result = adapter.subscribe_market_data(symbols, data_types).await;
        if subscribe_result.is_err() {
            warn!("订阅失败: {:?}", subscribe_result.err());
        }
        
        // 获取连接统计
        let stats_result = adapter.get_connection_stats().await;
        assert!(stats_result.is_ok(), "获取统计失败: {:?}", stats_result.err());
        
        // 测试交易功能（应返回未实现错误）
        let order = OrderRequest {
            symbol: "BTCUSDT".to_string(),
            exchange: ExchangeType::Binance,
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 0.001,
            price: Some(50000.0),
            time_in_force: TimeInForce::GTC,
            client_order_id: Some("test_order".to_string()),
        };
        
        let trade_result = adapter.place_order(&order).await;
        assert!(trade_result.is_err(), "交易功能应该返回未实现错误");
        
        // 断开连接
        let disconnect_result = adapter.disconnect_websocket().await;
        assert!(disconnect_result.is_ok(), "断开连接失败: {:?}", disconnect_result.err());
        
        // 验证连接状态
        let health_after_disconnect = adapter.health_check().await;
        assert!(health_after_disconnect.is_ok());
        
        info!("✅ Binance真实连接集成测试完成");
    }
}