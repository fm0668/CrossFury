# 币安期货连接器测试指南

本目录包含了币安期货连接器的各种测试文件，用于验证实盘数据获取功能。

## 测试文件说明

### 1. 集成测试 (`integration/`)

#### `binance_futures_test.rs`
完整的集成测试套件，包含：
- 服务器时间获取测试
- 交易所信息获取测试
- 深度数据获取测试
- 资金费率获取测试
- 标记价格获取测试
- WebSocket深度数据流测试

**运行方式：**
```bash
# 运行所有集成测试
cargo test --test binance_futures_test

# 运行特定测试
cargo test --test binance_futures_test test_get_funding_rate
cargo test --test binance_futures_test test_get_depth_data
cargo test --test binance_futures_test test_websocket_depth_stream
```

### 2. 示例程序 (`../examples/`)

#### `test_binance_futures_live_data.rs`
交互式实盘数据测试工具，功能包括：
- 实盘资金费率数据测试
- 实盘深度数据测试
- WebSocket实时深度流测试
- 支持命令行参数和交互式菜单

**运行方式：**
```bash
# 交互式运行
cargo run --example test_binance_futures_live_data

# 直接运行特定测试
cargo run --example test_binance_futures_live_data funding
cargo run --example test_binance_futures_live_data depth
cargo run --example test_binance_futures_live_data websocket
cargo run --example test_binance_futures_live_data all
```

#### `quick_test.rs`
快速验证基本功能的简化测试：
- 服务器时间
- BTCUSDT资金费率
- BTCUSDT深度数据
- BTCUSDT标记价格

**运行方式：**
```bash
cargo run --example quick_test
```

## 测试配置

### 网络环境
- **默认使用实盘数据**：所有测试默认连接币安实盘API
- **无需API密钥**：公开市场数据不需要API密钥
- **测试网支持**：可以通过修改配置切换到测试网

### 测试交易对
默认测试以下交易对：
- BTCUSDT
- ETHUSDT
- ADAUSDT
- SOLUSDT

### 速率限制
- REST API：1200次/分钟
- WebSocket：无限制
- 测试程序已内置适当的延迟控制

## 预期测试结果

### 资金费率测试
- ✅ 成功获取当前资金费率（通常在-0.1%到0.1%之间）
- ✅ 显示下次结算时间（每8小时结算一次）
- ✅ 费率水平分析（极低/低/中等/高）
- ✅ 多头/空头付费方向

### 深度数据测试
- ✅ 获取指定档位的买卖盘数据
- ✅ 显示最佳买卖价格和数量
- ✅ 计算买卖价差和百分比
- ✅ 流动性分析（买盘/卖盘占优情况）

### WebSocket测试
- ✅ 成功建立WebSocket连接
- ✅ 订阅深度数据流
- ✅ 实时接收深度更新（通常每秒多次更新）
- ✅ 价格变化检测
- ✅ 连接状态监控

## 故障排除

### 常见问题

1. **网络连接失败**
   ```
   ❌ 连接失败: Connection timeout
   ```
   - 检查网络连接
   - 确认防火墙设置
   - 尝试使用VPN

2. **API限制**
   ```
   ❌ 请求失败: Rate limit exceeded
   ```
   - 等待一分钟后重试
   - 减少测试频率

3. **数据解析错误**
   ```
   ❌ 解析失败: Invalid JSON format
   ```
   - 检查API响应格式是否变化
   - 更新解析逻辑

### 调试模式

启用详细日志输出：
```bash
RUST_LOG=debug cargo run --example quick_test
RUST_LOG=info cargo test --test binance_futures_test
```

### 性能监控

测试程序会显示以下性能指标：
- WebSocket更新频率（次/秒）
- 网络延迟
- 数据处理时间
- 连接稳定性

## 扩展测试

### 添加新交易对
在测试配置中修改 `test_symbols` 列表：
```rust
test_symbols: vec![
    "BTCUSDT".to_string(),
    "ETHUSDT".to_string(),
    "NEWCOIN".to_string(), // 添加新交易对
],
```

### 自定义测试参数
```rust
LiveTestConfig {
    use_testnet: false,           // 是否使用测试网
    test_symbols: vec![...],      // 测试交易对
    depth_limit: 20,              // 深度档位数量
    ws_test_duration: 30,         // WebSocket测试时长（秒）
}
```

## 注意事项

1. **实盘数据**：测试使用真实市场数据，价格会实时变化
2. **网络依赖**：需要稳定的网络连接
3. **API稳定性**：币安API偶尔会有维护，可能影响测试
4. **数据准确性**：所有数据来自币安官方API，具有高可靠性
5. **测试频率**：避免过于频繁的测试以防触发速率限制

## 技术支持

如果遇到问题，请检查：
1. 网络连接状态
2. 币安API服务状态
3. 项目依赖是否正确安装
4. Rust版本兼容性

更多技术细节请参考项目文档和源代码注释。