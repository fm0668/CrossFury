use crate::types::market_data::MarketDataEvent;
use crate::types::trading::TradeEvent;
use crate::core::AppError;
use crate::core::AppState;
use log::{info, error, debug, warn};
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

/// LBank WebSocket处理器
#[derive(Clone)]
pub struct LBankWebSocketHandler {
    app_state: Arc<AppState>,
    market_data_sender: tokio::sync::mpsc::Sender<MarketDataEvent>,
}

impl LBankWebSocketHandler {
    pub fn new(
        app_state: Arc<AppState>,
        market_data_sender: tokio::sync::mpsc::Sender<MarketDataEvent>,
    ) -> Self {
        Self {
            app_state,
            market_data_sender,
        }
    }

    /// 启动WebSocket连接
    pub async fn start(&self) -> Result<(), AppError> {
        info!("启动LBank WebSocket连接");
        
        let url = "wss://www.lbkex.net/ws/V2/";
        
        'reconnect: loop {
            match self.connect_and_handle(url).await {
                Ok(_) => {
                    warn!("LBank WebSocket连接正常关闭");
                    break;
                }
                Err(e) => {
                    error!("LBank WebSocket连接错误: {e}");
                    // 等待重连
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    info!("尝试重新连接LBank WebSocket");
                }
            }
        }
        
        Ok(())
    }

    async fn connect_and_handle(&self, url: &str) -> Result<(), AppError> {
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| AppError::ConnectionError(format!("连接LBank WebSocket失败: {e}")))?;
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        info!("LBank WebSocket连接已建立");
        
        // 发送订阅消息
        let subscribe_msg = r#"{"action":"subscribe","subscribe":"tick","pair":"btc_usdt"}"#;
        if let Err(e) = ws_sender.send(Message::Text(subscribe_msg.to_string())).await {
            error!("发送LBank订阅消息失败: {e}");
            return Err(AppError::ConnectionError(format!("订阅失败: {e}")));
        }
        
        // 降低日志噪声：订阅成功改为debug级别
        debug!("已订阅LBank市场数据");
        
        // 处理消息
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // 降低日志噪声：移除热路径debug日志
                    match self.parse_message(&text) {
                        Ok(events) => {
                            for event in events {
                                if let Err(e) = self.market_data_sender.send(event).await {
                                    error!("发送LBank市场数据失败: {e}");
                                    return Err(AppError::ConnectionError(format!("发送数据失败: {e}")));
                                }
                            }
                        }
                        Err(e) => {
                            // 保留错误路径的结构化上下文
                            error!("解析LBank WebSocket消息失败: {e}, 原始消息: {text}");
                        }
                    }
                }
                Ok(Message::Ping(payload)) => {
                    // 降低日志噪声：ping/pong改为debug级别
                    debug!("收到LBank ping，发送pong");
                    if let Err(e) = ws_sender.send(Message::Pong(payload)).await {
                        error!("发送LBank pong失败: {e}");
                    }
                }
                Ok(Message::Close(_)) => {
                    warn!("LBank WebSocket连接被服务器关闭");
                    break;
                }
                Ok(_) => {
                    // 降低日志噪声：移除其他消息类型的debug日志
                }
                Err(e) => {
                    // 保留错误路径的结构化上下文
                    error!("LBank WebSocket接收消息失败: {e}");
                    return Err(AppError::ConnectionError(format!("接收消息失败: {e}")));
                }
            }
        }
        
        Ok(())
    }

    fn parse_message(&self, text: &str) -> Result<Vec<MarketDataEvent>, AppError> {
        // 简化的消息解析逻辑
        // 实际实现需要根据LBank的消息格式进行解析
        debug!("解析LBank消息: {}", text.chars().take(100).collect::<String>());
        
        // 这里返回空向量，实际实现需要解析具体的市场数据
        Ok(vec![])
    }

    /// 订阅市场数据
    pub async fn subscribe(&self, symbols: Vec<String>) -> Result<(), AppError> {
        // 降低日志噪声：订阅信息改为debug级别
        debug!("订阅LBank符号: {:?}", symbols);
        
        // 在测试环境中，我们不启动真实的WebSocket连接
        // 只是模拟订阅成功
        Ok(())
    }

    /// 取消订阅
    pub async fn unsubscribe(&self, _symbols: Vec<String>) -> Result<(), AppError> {
        // 注意：现有的lbank_websocket_handler不支持动态取消订阅
        // 这里只是一个占位符实现
        debug!("LBank取消订阅请求（未实现）");
        Ok(())
    }

    /// 检查连接状态
    pub fn is_connected(&self) -> bool {
        // 通过检查连接健康状态来判断是否连接
        self.app_state.is_connection_healthy("lbank-1")
    }

    /// 获取连接统计信息
    pub fn get_stats(&self) -> (u64, u64) {
        let messages = self.app_state.websocket_messages.load(std::sync::atomic::Ordering::Relaxed);
        let updates = self.app_state.price_updates.load(std::sync::atomic::Ordering::Relaxed);
        (messages, updates)
    }
}