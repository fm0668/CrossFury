# 币安API密钥配置指南

## 概述
本文档用于配置币安API密钥，以便进行完整的交易功能测试。

## 重要说明
- **深度数据（订单簿）**: 不需要API密钥，属于公开市场数据
- **交易功能**: 需要API密钥和密钥
- **账户信息**: 需要API密钥和密钥

## API密钥获取步骤

1. 登录币安官网 (https://www.binance.com)
2. 进入账户管理 -> API管理
3. 创建新的API密钥
4. 设置权限（建议仅开启"读取"和"现货交易"权限）
5. 记录API Key和Secret Key

## 配置方式

### 方式一：环境变量（推荐）
```bash
# Windows PowerShell
$env:BINANCE_API_KEY="your_api_key_here"
$env:BINANCE_SECRET_KEY="your_secret_key_here"

# Linux/Mac
export BINANCE_API_KEY="your_api_key_here"
export BINANCE_SECRET_KEY="your_secret_key_here"
```

### 方式二：配置文件
创建 `api_keys.toml` 文件：
```toml
[binance]
api_key = "your_api_key_here"
secret_key = "your_secret_key_here"
testnet = false
rate_limit_per_minute = 1200
```

### 方式三：直接在测试代码中配置
```rust
let config = BinanceConfig {
    api_key: Some("your_api_key_here".to_string()),
    secret_key: Some("your_secret_key_here".to_string()),
    testnet: false,
    rate_limit_per_minute: 1200,
};
```

## 安全注意事项

⚠️ **重要安全提醒**：
- 永远不要将API密钥提交到版本控制系统
- 使用环境变量或配置文件存储密钥
- 定期轮换API密钥
- 仅授予必要的权限
- 在测试环境中优先使用测试网络

## 测试网络配置

币安测试网络配置：
```rust
let config = BinanceConfig {
    api_key: Some("testnet_api_key".to_string()),
    secret_key: Some("testnet_secret_key".to_string()),
    testnet: true,  // 使用测试网络
    rate_limit_per_minute: 1200,
};
```

测试网络地址：
- REST API: https://testnet.binance.vision
- WebSocket: wss://testnet.binance.vision/ws

## 下一步

请提供你的币安API密钥信息，我将帮你配置并测试：

1. **API Key**: `请在此处粘贴你的API密钥`
2. **Secret Key**: `请在此处粘贴你的Secret密钥`
3. **是否使用测试网络**: `是/否`

配置完成后，我们可以测试：
- ✅ 深度数据订阅（无需密钥）
- ✅ 账户余额查询（需要密钥）
- ✅ 订单下单测试（需要密钥，建议测试网络）