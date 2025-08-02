//! Binance期货REST API客户端模块
//! 
//! 实现期货交易所的REST API调用功能

use crate::connectors::binance::futures::config::{BinanceFuturesConfig, PositionSide as ConfigPositionSide};
use crate::connectors::binance::futures::constants::*;
use crate::types::trading::{OrderSide, OrderType, TimeInForce as TradingTimeInForce, PositionSide as TradingPositionSide};
use crate::core::AppError;

// 定义Result类型别名
pub type Result<T> = std::result::Result<T, AppError>;

use reqwest::{Client, Method, Response};
use serde_json::{Value, json};
use std::collections::HashMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use log::{info, warn, error, debug};
use chrono::Utc;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

/// Binance期货REST API客户端
pub struct BinanceFuturesRestClient {
    /// HTTP客户端
    client: Client,
    /// 配置信息
    config: BinanceFuturesConfig,
    /// API基础URL
    base_url: String,
}

/// API响应结果
#[derive(Debug)]
pub struct ApiResponse<T> {
    pub data: T,
    pub rate_limit_remaining: Option<u32>,
    pub rate_limit_reset: Option<u64>,
}

/// 订单请求参数
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
    pub time_in_force: Option<TradingTimeInForce>,
    pub position_side: Option<TradingPositionSide>,
    pub stop_price: Option<f64>,
    pub close_position: Option<bool>,
    pub activation_price: Option<f64>,
    pub callback_rate: Option<f64>,
    pub working_type: Option<String>,
    pub price_protect: Option<bool>,
    pub new_client_order_id: Option<String>,
    pub reduce_only: Option<bool>,
}

/// 杠杆调整请求
#[derive(Debug, Clone)]
pub struct LeverageRequest {
    pub symbol: String,
    pub leverage: u8,
}

/// 保证金模式调整请求
#[derive(Debug, Clone)]
pub struct MarginTypeRequest {
    pub symbol: String,
    pub margin_type: MarginType,
}

/// 持仓模式调整请求
#[derive(Debug, Clone)]
pub struct PositionModeRequest {
    pub dual_side_position: bool,
}

/// 保证金类型
#[derive(Debug, Clone)]
pub enum MarginType {
    Isolated,
    Cross,
}

impl MarginType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MarginType::Isolated => "ISOLATED",
            MarginType::Cross => "CROSSED",
        }
    }
}

impl BinanceFuturesRestClient {
    /// 创建新的REST API客户端
    pub fn new(config: BinanceFuturesConfig) -> Self {
        let base_url = if config.testnet {
            BINANCE_FUTURES_TESTNET_API_URL.to_string()
        } else {
            BINANCE_FUTURES_API_URL.to_string()
        };
        
        let client = Client::builder()
            .timeout(Duration::from_secs(config.rest_timeout))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            config,
            base_url,
        }
    }
    
    /// 获取服务器时间
    pub async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/fapi/v1/time", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        let data: Value = response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))?;
        
        data.get("serverTime")
            .and_then(|t| t.as_u64())
            .ok_or_else(|| AppError::ParseError("无效的服务器时间".to_string()))
    }
    
    /// 获取交易所信息
    pub async fn get_exchange_info(&self) -> Result<Value> {
        let url = format!("{}{}", self.base_url, FUTURES_EXCHANGE_INFO_PATH);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取深度数据
    pub async fn get_depth(&self, symbol: &str, limit: Option<u16>) -> Result<Value> {
        let mut url = format!("{}{}", self.base_url, FUTURES_DEPTH_PATH);
        let mut params = vec![("symbol", symbol.to_string())];
        
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        
        url.push('?');
        url.push_str(&self.build_query_string(&params));
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取24小时价格统计
    pub async fn get_24hr_ticker(&self, symbol: Option<&str>) -> Result<Value> {
        let mut url = format!("{}{}", self.base_url, FUTURES_TICKER_24HR_PATH);
        
        if let Some(symbol) = symbol {
            url.push_str(&format!("?symbol={}", symbol));
        }
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取标记价格
    pub async fn get_mark_price(&self, symbol: Option<&str>) -> Result<Value> {
        let mut url = format!("{}{}", self.base_url, FUTURES_MARK_PRICE_PATH);
        
        if let Some(symbol) = symbol {
            url.push_str(&format!("?symbol={}", symbol));
        }
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取资金费率历史
    pub async fn get_funding_rate(&self, symbol: &str, start_time: Option<u64>, end_time: Option<u64>, limit: Option<u16>) -> Result<Value> {
        let mut params = vec![("symbol", symbol.to_string())];
        
        if let Some(start_time) = start_time {
            params.push(("startTime", start_time.to_string()));
        }
        
        if let Some(end_time) = end_time {
            params.push(("endTime", end_time.to_string()));
        }
        
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        
        let url = format!("{}/fapi/v1/fundingRate?{}", self.base_url, self.build_query_string(&params));
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取持仓量
    pub async fn get_open_interest(&self, symbol: &str) -> Result<Value> {
        let url = format!("{}/fapi/v1/openInterest?symbol={}", self.base_url, symbol);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取账户信息
    pub async fn get_account_info(&self) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let mut params = vec![("timestamp", timestamp.to_string())];
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        params.push(("signature", signature));
        
        let url = format!("{}/fapi/v2/account?{}", self.base_url, self.build_query_string(&params));
        
        let response = self.send_signed_request(Method::GET, &url, None).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 获取持仓信息
    pub async fn get_position_info(&self, symbol: Option<&str>) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let mut params = vec![("timestamp", timestamp.to_string())];
        
        if let Some(symbol) = symbol {
            params.push(("symbol", symbol.to_string()));
        }
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        params.push(("signature", signature));
        
        let url = format!("{}/fapi/v2/positionRisk?{}", self.base_url, self.build_query_string(&params));
        
        let response = self.send_signed_request(Method::GET, &url, None).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 下单
    pub async fn place_order(&self, request: &OrderRequest) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let mut params = vec![
            ("symbol", request.symbol.clone()),
            ("side", request.side.to_api_string().to_string()),
            ("type", request.order_type.to_api_string().to_string()),
            ("quantity", request.quantity.to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        if let Some(price) = request.price {
            params.push(("price", price.to_string()));
        }
        
        if let Some(time_in_force) = &request.time_in_force {
            params.push(("timeInForce", time_in_force.to_api_string().to_string()));
        }
        
        if let Some(position_side) = &request.position_side {
            params.push(("positionSide", position_side.to_api_string().to_string()));
        }
        
        if let Some(stop_price) = request.stop_price {
            params.push(("stopPrice", stop_price.to_string()));
        }
        
        if let Some(close_position) = request.close_position {
            params.push(("closePosition", close_position.to_string()));
        }
        
        if let Some(reduce_only) = request.reduce_only {
            params.push(("reduceOnly", reduce_only.to_string()));
        }
        
        if let Some(new_client_order_id) = &request.new_client_order_id {
            params.push(("newClientOrderId", new_client_order_id.clone()));
        }
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        params.push(("signature", signature));
        
        let url = format!("{}/fapi/v1/order", self.base_url);
        let body = self.build_query_string(&params);
        
        let response = self.send_signed_request(Method::POST, &url, Some(body)).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 取消订单
    pub async fn cancel_order(&self, symbol: &str, order_id: Option<u64>, orig_client_order_id: Option<&str>) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let mut params = vec![
            ("symbol", symbol.to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        if let Some(order_id) = order_id {
            params.push(("orderId", order_id.to_string()));
        }
        
        if let Some(orig_client_order_id) = orig_client_order_id {
            params.push(("origClientOrderId", orig_client_order_id.to_string()));
        }
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        params.push(("signature", signature));
        
        let url = format!("{}/fapi/v1/order", self.base_url);
        let body = self.build_query_string(&params);
        
        let response = self.send_signed_request(Method::DELETE, &url, Some(body)).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 查询订单
    pub async fn query_order(&self, symbol: &str, order_id: Option<u64>, orig_client_order_id: Option<&str>) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let mut params = vec![
            ("symbol", symbol.to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        if let Some(order_id) = order_id {
            params.push(("orderId", order_id.to_string()));
        }
        
        if let Some(orig_client_order_id) = orig_client_order_id {
            params.push(("origClientOrderId", orig_client_order_id.to_string()));
        }
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        params.push(("signature", signature));
        
        let url = format!("{}/fapi/v1/order?{}", self.base_url, self.build_query_string(&params));
        
        let response = self.send_signed_request(Method::GET, &url, None).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 调整杠杆
    pub async fn change_leverage(&self, symbol: &str, leverage: u8) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let params = vec![
            ("symbol", symbol.to_string()),
            ("leverage", leverage.to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        let mut signed_params = params;
        signed_params.push(("signature", signature));
        
        let url = format!("{}/fapi/v1/leverage", self.base_url);
        let body = self.build_query_string(&signed_params);
        
        let response = self.send_signed_request(Method::POST, &url, Some(body)).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 调整保证金模式
    pub async fn change_margin_type(&self, symbol: &str, margin_type: MarginType) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let params = vec![
            ("symbol", symbol.to_string()),
            ("marginType", margin_type.as_str().to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        let mut signed_params = params;
        signed_params.push(("signature", signature));
        
        let url = format!("{}/fapi/v1/marginType", self.base_url);
        let body = self.build_query_string(&signed_params);
        
        let response = self.send_signed_request(Method::POST, &url, Some(body)).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 调整持仓模式
    pub async fn change_position_mode(&self, dual_side_position: bool) -> Result<Value> {
        let timestamp = Utc::now().timestamp_millis();
        let params = vec![
            ("dualSidePosition", dual_side_position.to_string()),
            ("timestamp", timestamp.to_string()),
        ];
        
        let query_string = self.build_query_string(&params);
        let signature = self.sign(&query_string)?;
        let mut signed_params = params;
        signed_params.push(("signature", signature));
        
        let url = format!("{}/fapi/v1/positionSide/dual", self.base_url);
        let body = self.build_query_string(&signed_params);
        
        let response = self.send_signed_request(Method::POST, &url, Some(body)).await?;
        
        response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))
    }
    
    /// 启动用户数据流
    pub async fn start_user_data_stream(&self) -> Result<String> {
        let url = format!("{}/fapi/v1/listenKey", self.base_url);
        
        let response = self.send_signed_request(Method::POST, &url, None).await?;
        
        let data: Value = response.json().await
            .map_err(|e| AppError::ParseError(format!("解析响应失败: {}", e)))?;
        
        data.get("listenKey")
            .and_then(|k| k.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::ParseError("无效的监听密钥".to_string()))
    }
    
    /// 保持用户数据流活跃
    pub async fn keepalive_user_data_stream(&self, listen_key: &str) -> Result<()> {
        let url = format!("{}/fapi/v1/listenKey", self.base_url);
        let body = format!("listenKey={}", listen_key);
        
        let _response = self.send_signed_request(Method::PUT, &url, Some(body)).await?;
        
        Ok(())
    }
    
    /// 关闭用户数据流
    pub async fn close_user_data_stream(&self, listen_key: &str) -> Result<()> {
        let url = format!("{}/fapi/v1/listenKey", self.base_url);
        let body = format!("listenKey={}", listen_key);
        
        let _response = self.send_signed_request(Method::DELETE, &url, Some(body)).await?;
        
        Ok(())
    }
    
    /// 发送签名请求
    async fn send_signed_request(&self, method: Method, url: &str, body: Option<String>) -> Result<Response> {
        let mut request = self.client.request(method, url);
        
        // 添加API密钥头
        if let Some(api_key) = &self.config.api_key {
            request = request.header("X-MBX-APIKEY", api_key);
        }
        
        // 添加请求体
        if let Some(body) = body {
            request = request.header("Content-Type", "application/x-www-form-urlencoded")
                .body(body);
        }
        
        request.send().await
            .map_err(|e| AppError::ConnectionError(format!("请求失败: {}", e)))
    }
    
    /// 构建查询字符串
    fn build_query_string(&self, params: &[(&str, String)]) -> String {
        params.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    }
    
    /// 签名
    fn sign(&self, query_string: &str) -> Result<String> {
        let secret_key = self.config.secret_key.as_ref()
            .ok_or_else(|| AppError::ConfigError("缺少密钥".to_string()))?;
        
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .map_err(|e| AppError::CryptoError(format!("HMAC初始化失败: {}", e)))?;
        
        mac.update(query_string.as_bytes());
        let result = mac.finalize();
        
        Ok(hex::encode(result.into_bytes()))
    }
}
