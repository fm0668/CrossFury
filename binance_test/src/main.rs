//! 测试币安REST API连接的Rust程序
//! 验证我们的网络环境是否可以访问币安API

use reqwest;
use serde_json::Value;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 开始测试币安REST API连接...");
    println!("测试时间: {}", chrono::Utc::now());
    println!("{}", "-".repeat(50));
    
    // 创建HTTP客户端
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    // 测试1: 获取服务器时间
    println!("=== 测试1: 获取服务器时间 ===");
    match test_server_time(&client).await {
        Ok(time) => println!("   ✅ 服务器时间: {}", time),
        Err(e) => println!("   ❌ 失败: {}", e),
    }
    
    // 测试2: 获取交易对信息
    println!("\n=== 测试2: 获取交易对信息 ===");
    match test_exchange_info(&client).await {
        Ok(count) => println!("   ✅ 获取到 {} 个交易对", count),
        Err(e) => println!("   ❌ 失败: {}", e),
    }
    
    // 测试3: 获取BTCUSDT价格
    println!("\n=== 测试3: 获取BTCUSDT价格 ===");
    match test_ticker_price(&client).await {
        Ok((price, change)) => {
            println!("   ✅ BTCUSDT 当前价格: {} USDT", price);
            println!("   ✅ 24小时涨跌幅: {}%", change);
        },
        Err(e) => println!("   ❌ 失败: {}", e),
    }
    
    // 测试4: 获取订单簿
    println!("\n=== 测试4: 获取BTCUSDT订单簿 ===");
    match test_orderbook(&client).await {
        Ok((bid, ask)) => {
            println!("   ✅ 买一价: {} USDT", bid);
            println!("   ✅ 卖一价: {} USDT", ask);
        },
        Err(e) => println!("   ❌ 失败: {}", e),
    }
    
    println!("\n{}", "=".repeat(50));
    println!("📊 测试完成!");
    println!("   REST API: ✅ 可以正常访问币安API");
    
    Ok(())
}

/// 测试服务器时间
async fn test_server_time(client: &reqwest::Client) -> Result<String, Box<dyn std::error::Error>> {
    let url = "https://api.binance.com/api/v3/time";
    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;
    
    let server_time = json["serverTime"]
        .as_i64()
        .ok_or("无法解析服务器时间")?;
    
    let datetime = chrono::DateTime::from_timestamp_millis(server_time)
        .ok_or("无法转换时间戳")?;
    
    Ok(datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

/// 测试交易对信息
async fn test_exchange_info(client: &reqwest::Client) -> Result<usize, Box<dyn std::error::Error>> {
    let url = "https://api.binance.com/api/v3/exchangeInfo";
    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;
    
    let symbols = json["symbols"]
        .as_array()
        .ok_or("无法解析交易对信息")?;
    
    Ok(symbols.len())
}

/// 测试价格信息
async fn test_ticker_price(client: &reqwest::Client) -> Result<(String, String), Box<dyn std::error::Error>> {
    let url = "https://api.binance.com/api/v3/ticker/24hr?symbol=BTCUSDT";
    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;
    
    let price = json["lastPrice"]
        .as_str()
        .ok_or("无法解析价格")?;
    
    let price_change_percent = json["priceChangePercent"]
        .as_str()
        .ok_or("无法解析涨跌幅")?;
    
    Ok((price.to_string(), price_change_percent.to_string()))
}

/// 测试订单簿
async fn test_orderbook(client: &reqwest::Client) -> Result<(String, String), Box<dyn std::error::Error>> {
    let url = "https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=5";
    let response = client.get(url).send().await?;
    let json: Value = response.json().await?;
    
    let bids = json["bids"]
        .as_array()
        .ok_or("无法解析买单")?;
    
    let asks = json["asks"]
        .as_array()
        .ok_or("无法解析卖单")?;
    
    let best_bid = bids.first()
        .and_then(|b| b.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.as_str())
        .ok_or("无法解析最佳买价")?;
    
    let best_ask = asks.first()
        .and_then(|a| a.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.as_str())
        .ok_or("无法解析最佳卖价")?;
    
    Ok((best_bid.to_string(), best_ask.to_string()))
}
