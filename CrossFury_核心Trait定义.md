# CrossFury 核心 Trait 定义文档

## 概述

本文档定义了 CrossFury 重构项目中所有核心 trait，确保各模块间的接口统一性和一致性。所有重构代码必须基于这些 trait 定义进行实现。

## 1. 连接模块核心 Trait

### 1.1 ExchangeConnector
```rust
// src/connectors/traits.rs
use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::types::*;

#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    // 基础信息
    fn get_exchange_type(&self) -> ExchangeType;
    fn get_market_type(&self) -> MarketType;
    fn get_exchange_name(&self) -> &str;
    
    // WebSocket 连接管理
    async fn connect_websocket(&self) -> Result<(), ConnectorError>;
    async fn disconnect_websocket(&self) -> Result<(), ConnectorError>;
    async fn subscribe_orderbook(&self, symbol: &str) -> Result<(), ConnectorError>;
    async fn subscribe_trades(&self, symbol: &str) -> Result<(), ConnectorError>;
    async fn subscribe_user_stream(&self) -> Result<(), ConnectorError>;
    
    // 推送式数据流接口
    fn get_market_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage>;
    fn get_user_data_stream(&self) -> mpsc::UnboundedReceiver<StandardizedMessage>;
    
    // 本地缓存快照读取
    fn get_orderbook_snapshot(&self, symbol: &str) -> Option<StandardizedOrderBook>;
    fn get_recent_trades_snapshot(&self, symbol: &str, limit: usize) -> Vec<StandardizedTrade>;
    
    // 交易相关操作 (REST API)
    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse, ConnectorError>;
    async fn cancel_order(&self, order_id: &str, symbol: &str) -> Result<bool, ConnectorError>;
    async fn get_order_status(&self, order_id: &str, symbol: &str) -> Result<OrderStatus, ConnectorError>;
    async fn get_account_balance(&self) -> Result<AccountBalance, ConnectorError>;
    
    // 连接状态
    fn is_connected(&self) -> bool;
    fn is_websocket_connected(&self) -> bool;
    fn get_connection_status(&self) -> ConnectionStatus;
}
```

### 1.2 DataFlowManager
```rust
// src/connectors/data_flow_manager.rs
pub trait DataFlowManager: Send + Sync {
    // 高频数据流管理
    fn take_market_data_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<HighFrequencyData>>;
    fn subscribe_events(&self) -> broadcast::Receiver<SystemEvent>;
    fn send_market_data(&self, data: HighFrequencyData) -> Result<(), mpsc::error::SendError<HighFrequencyData>>;
    async fn send_event(&self, event: SystemEvent);
}
```

### 1.3 ConnectorManager
```rust
// src/connectors/manager.rs
#[async_trait]
pub trait ConnectorManager: Send + Sync {
    // 连接器管理
    async fn add_connector(&mut self, connector: Box<dyn ExchangeConnector>) -> Result<(), ConnectorError>;
    async fn remove_connector(&mut self, exchange: ExchangeType, market_type: MarketType) -> Result<(), ConnectorError>;
    fn get_connector(&self, exchange: ExchangeType, market_type: MarketType) -> Option<&dyn ExchangeConnector>;
    fn get_all_connectors(&self) -> Vec<&dyn ExchangeConnector>;
    
    // 批量操作
    async fn connect_all(&self) -> Result<(), ConnectorError>;
    async fn disconnect_all(&self) -> Result<(), ConnectorError>;
    async fn get_connection_status_all(&self) -> HashMap<(ExchangeType, MarketType), ConnectionStatus>;
}
```

## 2. 执行模块核心 Trait

### 2.1 OrderExecutor
```rust
// src/executors/traits.rs
#[async_trait]
pub trait OrderExecutor: Send + Sync {
    // 订单执行
    async fn execute_order(&self, order: OrderRequest) -> Result<ExecutionResult, ExecutionError>;
    async fn cancel_order(&self, order_id: &str, exchange: ExchangeType) -> Result<bool, ExecutionError>;
    async fn modify_order(&self, order_id: &str, new_params: OrderModification) -> Result<bool, ExecutionError>;
    
    // 批量操作
    async fn execute_batch_orders(&self, orders: Vec<OrderRequest>) -> Vec<Result<ExecutionResult, ExecutionError>>;
    async fn cancel_batch_orders(&self, order_ids: Vec<(String, ExchangeType)>) -> Vec<Result<bool, ExecutionError>>;
    
    // 查询操作
    async fn get_order_status(&self, order_id: &str, exchange: ExchangeType) -> Result<OrderStatus, ExecutionError>;
    async fn get_open_orders(&self, exchange: Option<ExchangeType>) -> Result<Vec<OrderStatus>, ExecutionError>;
    async fn get_execution_history(&self, filter: ExecutionFilter) -> Result<Vec<ExecutionRecord>, ExecutionError>;
    
    // 仓位管理
    async fn get_positions(&self, exchange: Option<ExchangeType>) -> Result<Vec<Position>, ExecutionError>;
    async fn get_portfolio_summary(&self) -> Result<PortfolioSummary, ExecutionError>;
}
```

### 2.2 RiskManager
```rust
// src/executors/risk_manager.rs
#[async_trait]
pub trait RiskManager: Send + Sync {
    // 风险检查
    async fn check_order_risk(&self, order: &OrderRequest) -> Result<RiskCheckResult, RiskError>;
    async fn check_position_risk(&self, symbol: &str, exchange: ExchangeType) -> Result<PositionRiskResult, RiskError>;
    async fn check_portfolio_risk(&self) -> Result<PortfolioRiskResult, RiskError>;
    
    // 风险限制
    async fn get_position_limit(&self, symbol: &str, market_type: MarketType) -> Result<PositionLimit, RiskError>;
    async fn get_order_size_limit(&self, symbol: &str, market_type: MarketType) -> Result<OrderSizeLimit, RiskError>;
    
    // 风险监控
    async fn monitor_risk_metrics(&self) -> Result<RiskMetrics, RiskError>;
    async fn handle_risk_violation(&self, violation: RiskViolation) -> Result<(), RiskError>;
}
```

### 2.3 PositionManager
```rust
// src/executors/position_manager.rs
#[async_trait]
pub trait PositionManager: Send + Sync {
    // 仓位跟踪
    async fn update_position(&mut self, execution: &ExecutionRecord) -> Result<(), PositionError>;
    async fn get_position(&self, symbol: &str, exchange: ExchangeType) -> Result<Option<Position>, PositionError>;
    async fn get_all_positions(&self) -> Result<Vec<Position>, PositionError>;
    
    // 仓位计算
    async fn calculate_unrealized_pnl(&self, symbol: &str, exchange: ExchangeType) -> Result<Decimal, PositionError>;
    async fn calculate_portfolio_value(&self) -> Result<Decimal, PositionError>;
    
    // 仓位同步
    async fn sync_positions_from_exchange(&mut self, exchange: ExchangeType) -> Result<(), PositionError>;
    async fn reconcile_positions(&mut self) -> Result<Vec<PositionDiscrepancy>, PositionError>;
}
```

### 2.4 OrderRouter
```rust
// src/executors/routing.rs
#[async_trait]
pub trait OrderRouter: Send + Sync {
    // 路由决策
    async fn route_order(&self, order: &OrderRequest) -> Result<ExchangeType, RoutingError>;
    async fn get_available_exchanges(&self, symbol: &str, market_type: MarketType) -> Result<Vec<ExchangeType>, RoutingError>;
    
    // 流动性分析
    async fn select_best_liquidity_exchange(&self, exchanges: &[ExchangeType], symbol: &str, side: OrderSide, quantity: Decimal) -> Result<ExchangeType, RoutingError>;
    async fn select_lowest_fee_exchange(&self, exchanges: &[ExchangeType], symbol: &str, market_type: MarketType) -> Result<ExchangeType, RoutingError>;
    async fn select_most_reliable_exchange(&self, exchanges: &[ExchangeType], symbol: &str) -> Result<ExchangeType, RoutingError>;
}
```

## 3. 策略模块核心 Trait

### 3.1 Strategy
```rust
// src/strategies/traits.rs
#[async_trait]
pub trait Strategy: Send + Sync {
    // 策略基本信息
    fn get_strategy_name(&self) -> &str;
    fn get_strategy_version(&self) -> &str;
    fn get_description(&self) -> &str;
    fn get_supported_markets(&self) -> Vec<MarketType>;
    fn get_required_symbols(&self) -> Vec<String>;
    
    // 策略生命周期
    async fn initialize(&mut self, config: StrategyConfig) -> Result<(), StrategyError>;
    async fn start(&mut self) -> Result<(), StrategyError>;
    async fn stop(&mut self) -> Result<(), StrategyError>;
    async fn cleanup(&mut self) -> Result<(), StrategyError>;
    
    // 数据处理（响应推送式数据流）
    async fn on_market_data(&mut self, data: MarketDataEvent) -> Result<Vec<StrategySignal>, StrategyError>;
    async fn on_user_data(&mut self, data: UserDataEvent) -> Result<Vec<StrategySignal>, StrategyError>;
    async fn on_system_event(&mut self, event: SystemEvent) -> Result<Vec<StrategySignal>, StrategyError>;
    
    // 执行反馈处理
    async fn on_execution_result(&mut self, result: ExecutionResult) -> Result<Vec<StrategySignal>, StrategyError>;
    async fn on_execution_error(&mut self, error: ExecutionError) -> Result<Vec<StrategySignal>, StrategyError>;
    
    // 策略状态管理
    async fn get_strategy_state(&self) -> StrategyState;
    async fn set_strategy_parameters(&mut self, params: HashMap<String, StrategyParameter>) -> Result<(), StrategyError>;
    async fn get_strategy_metrics(&self) -> StrategyMetrics;
    
    // 风险管理
    async fn get_risk_limits(&self) -> RiskLimits;
    async fn handle_risk_event(&mut self, event: RiskEvent) -> Result<Vec<StrategySignal>, StrategyError>;
    
    // 可选方法
    async fn on_timer(&mut self, timer_id: String) -> Result<Vec<StrategySignal>, StrategyError> {
        Ok(Vec::new())
    }
    
    async fn on_custom_event(&mut self, event: CustomEvent) -> Result<Vec<StrategySignal>, StrategyError> {
        Ok(Vec::new())
    }
}
```

### 3.2 StrategyManager
```rust
// src/strategies/manager.rs
#[async_trait]
pub trait StrategyManager: Send + Sync {
    // 策略管理
    async fn register_strategy(&mut self, strategy: Box<dyn Strategy>) -> Result<String, StrategyError>;
    async fn unregister_strategy(&mut self, strategy_id: &str) -> Result<(), StrategyError>;
    async fn get_strategy(&self, strategy_id: &str) -> Option<&dyn Strategy>;
    async fn get_all_strategies(&self) -> Vec<&dyn Strategy>;
    
    // 策略控制
    async fn start_strategy(&mut self, strategy_id: &str) -> Result<(), StrategyError>;
    async fn stop_strategy(&mut self, strategy_id: &str) -> Result<(), StrategyError>;
    async fn restart_strategy(&mut self, strategy_id: &str) -> Result<(), StrategyError>;
    
    // 配置管理
    async fn update_strategy_config(&mut self, strategy_id: &str, config: StrategyConfig) -> Result<(), StrategyError>;
    async fn get_strategy_config(&self, strategy_id: &str) -> Result<StrategyConfig, StrategyError>;
    
    // 监控
    async fn get_strategy_status(&self, strategy_id: &str) -> Result<StrategyStatus, StrategyError>;
    async fn get_all_strategy_metrics(&self) -> Result<HashMap<String, StrategyMetrics>, StrategyError>;
}
```

### 3.3 EventBus
```rust
// src/strategies/event_bus.rs
#[async_trait]
pub trait EventBus: Send + Sync {
    // 事件发布
    async fn publish_market_data(&self, data: MarketDataEvent) -> Result<(), EventBusError>;
    async fn publish_user_data(&self, data: UserDataEvent) -> Result<(), EventBusError>;
    async fn publish_system_event(&self, event: SystemEvent) -> Result<(), EventBusError>;
    async fn publish_execution_result(&self, result: ExecutionResult) -> Result<(), EventBusError>;
    
    // 事件订阅
    async fn subscribe_market_data(&self, strategy_id: &str, filter: MarketDataFilter) -> Result<(), EventBusError>;
    async fn subscribe_user_data(&self, strategy_id: &str, filter: UserDataFilter) -> Result<(), EventBusError>;
    async fn subscribe_system_events(&self, strategy_id: &str, filter: SystemEventFilter) -> Result<(), EventBusError>;
    
    // 事件路由
    async fn route_strategy_signals(&self, strategy_id: &str, signals: Vec<StrategySignal>) -> Result<(), EventBusError>;
}
```

## 4. 测试模块核心 Trait

### 4.1 MockConnector
```rust
// src/testing/mock_connector.rs
#[async_trait]
pub trait MockConnector: ExchangeConnector {
    // 模拟控制
    async fn set_mock_config(&mut self, config: MockConnectorConfig) -> Result<(), MockError>;
    async fn simulate_connection_error(&mut self, error_type: ConnectionErrorType) -> Result<(), MockError>;
    async fn simulate_latency(&mut self, latency: Duration) -> Result<(), MockError>;
    
    // 市场数据模拟
    async fn inject_market_data(&mut self, data: StandardizedMessage) -> Result<(), MockError>;
    async fn set_orderbook(&mut self, symbol: &str, orderbook: StandardizedOrderBook) -> Result<(), MockError>;
    async fn add_trade(&mut self, symbol: &str, trade: StandardizedTrade) -> Result<(), MockError>;
    
    // 账户模拟
    async fn set_account_balance(&mut self, asset: &str, balance: Decimal) -> Result<(), MockError>;
    async fn simulate_order_execution(&mut self, order_id: &str, execution: ExecutionResult) -> Result<(), MockError>;
    
    // 测试辅助
    async fn reset_state(&mut self) -> Result<(), MockError>;
    async fn get_mock_metrics(&self) -> MockMetrics;
}
```

### 4.2 TestFramework
```rust
// src/testing/framework.rs
#[async_trait]
pub trait TestFramework: Send + Sync {
    // 测试环境管理
    async fn setup_test_environment(&mut self, config: TestConfig) -> Result<(), TestError>;
    async fn teardown_test_environment(&mut self) -> Result<(), TestError>;
    
    // 测试数据管理
    async fn load_test_data(&mut self, data_set: &str) -> Result<(), TestError>;
    async fn generate_test_data(&mut self, scenario: TestScenario) -> Result<(), TestError>;
    
    // 测试执行
    async fn run_unit_tests(&self, module: &str) -> Result<TestResults, TestError>;
    async fn run_integration_tests(&self, scenario: &str) -> Result<TestResults, TestError>;
    async fn run_performance_tests(&self, benchmark: &str) -> Result<PerformanceResults, TestError>;
    
    // 回测
    async fn run_backtest(&self, strategy: &str, data_range: DateRange) -> Result<BacktestResults, TestError>;
}
```

### 4.3 MarketSimulator
```rust
// src/testing/market_simulator.rs
#[async_trait]
pub trait MarketSimulator: Send + Sync {
    // 市场模拟控制
    async fn start_simulation(&mut self) -> Result<(), SimulationError>;
    async fn stop_simulation(&mut self) -> Result<(), SimulationError>;
    async fn pause_simulation(&mut self) -> Result<(), SimulationError>;
    async fn resume_simulation(&mut self) -> Result<(), SimulationError>;
    
    // 价格模拟
    async fn set_price_model(&mut self, symbol: &str, model: PriceModel) -> Result<(), SimulationError>;
    async fn inject_price_shock(&mut self, symbol: &str, shock: PriceShock) -> Result<(), SimulationError>;
    
    // 流动性模拟
    async fn set_liquidity_model(&mut self, symbol: &str, model: LiquidityModel) -> Result<(), SimulationError>;
    async fn simulate_market_impact(&mut self, order: &OrderRequest) -> Result<MarketImpact, SimulationError>;
    
    // 事件模拟
    async fn schedule_market_event(&mut self, event: MarketEvent, time: DateTime<Utc>) -> Result<(), SimulationError>;
    async fn trigger_system_event(&mut self, event: SystemEvent) -> Result<(), SimulationError>;
}
```

## 5. 通用工具 Trait

### 5.1 ConfigManager
```rust
// src/config/manager.rs
#[async_trait]
pub trait ConfigManager: Send + Sync {
    // 配置加载
    async fn load_config(&mut self, config_path: &str) -> Result<(), ConfigError>;
    async fn reload_config(&mut self) -> Result<(), ConfigError>;
    
    // 配置访问
    fn get_connector_config(&self, exchange: ExchangeType) -> Result<ConnectorConfig, ConfigError>;
    fn get_strategy_config(&self, strategy_name: &str) -> Result<StrategyConfig, ConfigError>;
    fn get_risk_config(&self) -> Result<RiskConfig, ConfigError>;
    
    // 配置更新
    async fn update_config(&mut self, section: &str, config: serde_json::Value) -> Result<(), ConfigError>;
    async fn save_config(&self, config_path: &str) -> Result<(), ConfigError>;
}
```

### 5.2 Logger
```rust
// src/logging/logger.rs
pub trait Logger: Send + Sync {
    // 日志记录
    fn log_info(&self, message: &str, context: Option<LogContext>);
    fn log_warn(&self, message: &str, context: Option<LogContext>);
    fn log_error(&self, message: &str, context: Option<LogContext>);
    fn log_debug(&self, message: &str, context: Option<LogContext>);
    
    // 结构化日志
    fn log_trade(&self, trade: &TradeLog);
    fn log_order(&self, order: &OrderLog);
    fn log_performance(&self, metrics: &PerformanceLog);
    
    // 日志配置
    fn set_log_level(&mut self, level: LogLevel);
    fn add_log_target(&mut self, target: LogTarget);
}
```

### 5.3 MetricsCollector
```rust
// src/metrics/collector.rs
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    // 指标收集
    async fn collect_connector_metrics(&self, exchange: ExchangeType) -> Result<ConnectorMetrics, MetricsError>;
    async fn collect_executor_metrics(&self) -> Result<ExecutorMetrics, MetricsError>;
    async fn collect_strategy_metrics(&self, strategy_id: &str) -> Result<StrategyMetrics, MetricsError>;
    
    // 指标聚合
    async fn aggregate_system_metrics(&self) -> Result<SystemMetrics, MetricsError>;
    async fn calculate_performance_metrics(&self, time_range: TimeRange) -> Result<PerformanceMetrics, MetricsError>;
    
    // 指标导出
    async fn export_metrics(&self, format: MetricsFormat, output: &str) -> Result<(), MetricsError>;
}
```

## 6. Trait 使用规范

### 6.1 实现规范
1. **所有连接器必须实现 `ExchangeConnector` trait**
2. **所有执行器必须实现 `OrderExecutor` trait**
3. **所有策略必须实现 `Strategy` trait**
4. **所有测试连接器必须实现 `MockConnector` trait**

### 6.2 命名规范
1. **Trait 名称使用 PascalCase**
2. **方法名称使用 snake_case**
3. **异步方法必须使用 `async fn`**
4. **错误类型必须实现 `std::error::Error`**

### 6.3 错误处理规范
1. **每个模块定义自己的错误类型**
2. **错误类型必须实现 `Send + Sync + 'static`**
3. **使用 `Result<T, E>` 返回可能失败的操作**
4. **提供详细的错误信息和上下文**

### 6.4 异步规范
1. **所有 I/O 操作必须是异步的**
2. **使用 `#[async_trait]` 宏定义异步 trait**
3. **避免在异步函数中使用阻塞操作**
4. **合理使用 `tokio::spawn` 进行并发处理**

## 7. 重构实施顺序

### 阶段一：基础 Trait 定义
1. 创建 `src/types/` 模块，定义所有数据类型
2. 创建 `src/connectors/traits.rs`，定义连接器 trait
3. 创建 `src/executors/traits.rs`，定义执行器 trait
4. 创建 `src/strategies/traits.rs`，定义策略 trait

### 阶段二：核心模块实现
1. 实现基础的 `MockConnector`
2. 实现 `DataFlowManager`
3. 实现 `OrderExecutor` 基础版本
4. 实现 `StrategyManager` 基础版本

### 阶段三：连接器迁移
1. 将现有连接器适配到新的 trait
2. 逐个交易所进行迁移和测试
3. 保持旧代码作为备份

### 阶段四：执行器和策略迁移
1. 迁移现有的执行逻辑
2. 实现新的策略框架
3. 集成测试和性能优化

### 阶段五：测试和优化
1. 完善测试框架
2. 性能基准测试
3. 文档完善和代码清理

---

**注意：本文档是重构的核心指导文档，所有重构代码必须严格遵循这些 trait 定义。任何对 trait 的修改都需要经过充分讨论和测试。**