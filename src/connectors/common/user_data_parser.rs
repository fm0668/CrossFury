//! 用户数据解析器模块
//!
//! 负责解析和路由用户数据事件，与市场数据完全隔离
use crate::types::common::{ExchangeType, MarketType};
use crate::types::events::{
    BalanceChangeReason, StandardizedBalanceUpdate, StandardizedOrderUpdate,
    StandardizedPositionUpdate, StandardizedTradeExecution, UserDataEvent,
};
use crate::types::trading::{OrderSide, OrderStatus, OrderType, PositionSide};
use log::{debug, error, warn};
use serde_json::Value;
use std::time::SystemTime;
use tokio::sync::mpsc;
use thiserror::Error;

/// 用户数据解析错误
#[derive(Debug, Error)]
pub enum UserDataParseError {
    #[error("JSON 解析失败: {0}")]
    JsonParseError(String),
    #[error("缺少必要字段: {0}")]
    MissingField(String),
    #[error("不支持的交易所: {0:?}")]
    UnsupportedExchange(ExchangeType),
    #[error("未知事件类型: {0}")]
    UnknownEventType(String),
    #[error("无效的枚举值: {0}")]
    InvalidEnumValue(String),
    #[error("通道发送失败")]
    ChannelSendError,
}

/// 用户数据解析器
#[derive(Debug, Clone)]
pub struct UserDataParser {
    /// 交易所类型
    exchange: ExchangeType,
    /// 市场类型
    market_type: MarketType,
    /// 用户数据事件发送器
    user_event_sender: mpsc::Sender<UserDataEvent>,
}

impl UserDataParser {
    /// 创建新的用户数据解析器
    pub fn new(
        exchange: ExchangeType,
        market_type: MarketType,
        user_event_sender: mpsc::Sender<UserDataEvent>,
    ) -> Self {
        Self {
            exchange,
            market_type,
            user_event_sender,
        }
    }

    /// 解析用户数据消息
    pub async fn parse_user_data(&self, raw_message: &str) -> Result<(), UserDataParseError> {
        debug!("解析用户数据消息: {raw_message}");

        let json_value: Value = serde_json::from_str(raw_message)
            .map_err(|e| UserDataParseError::JsonParseError(e.to_string()))?;

        // 根据交易所类型分发解析
        match self.exchange {
            ExchangeType::Binance => self.parse_binance_user_data(&json_value).await,
            _ => {
                warn!("暂不支持的交易所类型: {:?}", self.exchange);
                Err(UserDataParseError::UnsupportedExchange(self.exchange))
            }
        }
    }

    /// 解析Binance用户数据
    async fn parse_binance_user_data(&self, json: &Value) -> Result<(), UserDataParseError> {
        // 获取事件类型
        let event_type = json
            .get("e")
            .and_then(|v| v.as_str())
            .ok_or_else(|| UserDataParseError::MissingField("event_type".to_string()))?;

        debug!("Binance用户数据事件类型: {event_type}");

        match event_type {
            "executionReport" => self.parse_binance_order_update(json).await,
            "outboundAccountPosition" => self.parse_binance_balance_update(json).await,
            "ACCOUNT_UPDATE" => self.parse_binance_account_update(json).await,
            "ORDER_TRADE_UPDATE" => self.parse_binance_order_trade_update(json).await,
            _ => {
                warn!("未知的Binance用户数据事件类型: {event_type}");
                Err(UserDataParseError::UnknownEventType(event_type.to_string()))
            }
        }
    }

    /// 解析Binance订单更新
    async fn parse_binance_order_update(&self, json: &Value) -> Result<(), UserDataParseError> {
        let order_update = StandardizedOrderUpdate {
            order_id: self.extract_string(json, "i")?,
            client_order_id: json
                .get("c")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            symbol: self.extract_string(json, "s")?,
            side: self.parse_order_side(&self.extract_string(json, "S")?)?,
            order_type: self.parse_order_type(&self.extract_string(json, "o")?)?,
            status: self.parse_order_status(&self.extract_string(json, "X")?)?,
            quantity: self.extract_f64(json, "q")?,
            price: json
                .get("p")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok()),
            filled_quantity: self.extract_f64(json, "z")?,
            remaining_quantity: self.extract_f64(json, "q")? - self.extract_f64(json, "z")?,
            average_price: json
                .get("ap")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok()),
            position_side: json
                .get("ps")
                .and_then(|v| v.as_str())
                .and_then(|s| self.parse_position_side(s).ok()),
            reduce_only: json.get("R").and_then(|v| v.as_bool()),
            created_time: SystemTime::now(),
            updated_time: SystemTime::now(),
        };

        let event = UserDataEvent::OrderUpdate {
            exchange: self.exchange,
            market_type: self.market_type,
            order: order_update,
            timestamp: SystemTime::now(),
        };

        self.send_user_event(event).await
    }

    /// 解析Binance余额更新
    async fn parse_binance_balance_update(&self, json: &Value) -> Result<(), UserDataParseError> {
        let balances = json
            .get("B")
            .and_then(|v| v.as_array())
            .ok_or_else(|| UserDataParseError::MissingField("balances".to_string()))?;

        for balance_data in balances {
            let asset = self.extract_string(balance_data, "a")?;
            let free = self.extract_f64(balance_data, "f")?;
            let locked = self.extract_f64(balance_data, "l")?;
            let total = free + locked;

            let balance_update = StandardizedBalanceUpdate {
                asset,
                total_balance: total,
                available_balance: free,
                frozen_balance: locked,
                balance_change: 0.0,
                change_reason: BalanceChangeReason::Trade,
            };

            let event = UserDataEvent::BalanceUpdate {
                exchange: self.exchange,
                market_type: self.market_type,
                balance: balance_update,
                timestamp: SystemTime::now(),
            };

            self.send_user_event(event).await?;
        }

        Ok(())
    }

    /// 解析Binance账户更新（期货）
    async fn parse_binance_account_update(&self, json: &Value) -> Result<(), UserDataParseError> {
        let account_data = json
            .get("a")
            .ok_or_else(|| UserDataParseError::MissingField("account_data".to_string()))?;

        // 解析余额更新
        if let Some(balances) = account_data.get("B").and_then(|v| v.as_array()) {
            for balance_data in balances {
                let asset = self.extract_string(balance_data, "a")?;
                let wallet_balance = self.extract_f64(balance_data, "wb")?;
                let cross_wallet_balance = self.extract_f64(balance_data, "cw")?;
                let balance_change = self.extract_f64(balance_data, "bc")?;

                let balance_update = StandardizedBalanceUpdate {
                    asset,
                    total_balance: wallet_balance,
                    available_balance: cross_wallet_balance,
                    frozen_balance: wallet_balance - cross_wallet_balance,
                    balance_change,
                    change_reason: self.determine_balance_change_reason(json),
                };

                let event = UserDataEvent::BalanceUpdate {
                    exchange: self.exchange,
                    market_type: self.market_type,
                    balance: balance_update,
                    timestamp: SystemTime::now(),
                };

                self.send_user_event(event).await?;
            }
        }

        // 解析持仓更新
        if let Some(positions) = account_data.get("P").and_then(|v| v.as_array()) {
            for position_data in positions {
                let symbol = self.extract_string(position_data, "s")?;
                let position_amount = self.extract_f64(position_data, "pa")?;

                // 只处理有持仓的数据
                if position_amount != 0.0 {
                    let position_update = StandardizedPositionUpdate {
                        symbol,
                        position_side: self
                            .parse_position_side(&self.extract_string(position_data, "ps")?)?,
                        position_amount,
                        entry_price: self.extract_f64(position_data, "ep")?,
                        mark_price: self.extract_f64(position_data, "mp")?,
                        unrealized_pnl: self.extract_f64(position_data, "up")?,
                        realized_pnl: 0.0,
                        margin: 0.0,
                        leverage: 1.0,
                        pnl_percentage: 0.0,
                    };

                    let event = UserDataEvent::PositionUpdate {
                        exchange: self.exchange,
                        market_type: self.market_type,
                        position: position_update,
                        timestamp: SystemTime::now(),
                    };

                    self.send_user_event(event).await?;
                }
            }
        }

        Ok(())
    }

    /// 解析Binance订单交易更新（期货）
    async fn parse_binance_order_trade_update(
        &self,
        json: &Value,
    ) -> Result<(), UserDataParseError> {
        let order_data = json
            .get("o")
            .ok_or_else(|| UserDataParseError::MissingField("order_data".to_string()))?;

        // 如果是成交事件，解析成交信息
        if let Some(trade_id) = order_data.get("t").and_then(|v| v.as_str()) {
            if trade_id != "0" {
                let execution = StandardizedTradeExecution {
                    trade_id: trade_id.to_string(),
                    order_id: self.extract_string(order_data, "i")?,
                    symbol: self.extract_string(order_data, "s")?,
                    side: self.parse_order_side(&self.extract_string(order_data, "S")?)?,
                    quantity: self.extract_f64(order_data, "l")?,
                    price: self.extract_f64(order_data, "L")?,
                    commission: self.extract_f64(order_data, "n")?,
                    commission_asset: self.extract_string(order_data, "N")?,
                    is_maker: self.extract_string(order_data, "m")? == "true",
                    execution_time: SystemTime::now(),
                };

                let event = UserDataEvent::TradeExecution {
                    exchange: self.exchange,
                    market_type: self.market_type,
                    execution,
                    timestamp: SystemTime::now(),
                };

                self.send_user_event(event).await?;
            }
        }

        // 解析订单更新
        let order_update = StandardizedOrderUpdate {
            order_id: self.extract_string(order_data, "i")?,
            client_order_id: order_data
                .get("c")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            symbol: self.extract_string(order_data, "s")?,
            side: self.parse_order_side(&self.extract_string(order_data, "S")?)?,
            order_type: self.parse_order_type(&self.extract_string(order_data, "o")?)?,
            status: self.parse_order_status(&self.extract_string(order_data, "X")?)?,
            quantity: self.extract_f64(order_data, "q")?,
            price: order_data
                .get("p")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok()),
            filled_quantity: self.extract_f64(order_data, "z")?,
            remaining_quantity: self.extract_f64(order_data, "q")?
                - self.extract_f64(order_data, "z")?,
            average_price: order_data
                .get("ap")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok()),
            position_side: order_data
                .get("ps")
                .and_then(|v| v.as_str())
                .and_then(|s| self.parse_position_side(s).ok()),
            reduce_only: order_data.get("R").and_then(|v| v.as_bool()),
            created_time: SystemTime::now(),
            updated_time: SystemTime::now(),
        };

        let event = UserDataEvent::OrderUpdate {
            exchange: self.exchange,
            market_type: self.market_type,
            order: order_update,
            timestamp: SystemTime::now(),
        };

        self.send_user_event(event).await
    }

    /// 发送用户事件
    async fn send_user_event(&self, event: UserDataEvent) -> Result<(), UserDataParseError> {
        self.user_event_sender
            .send(event)
            .await
            .map_err(|_| UserDataParseError::ChannelSendError)
    }

    /// 辅助方法：提取字符串字段（支持数字转字符串）
    fn extract_string(&self, json: &Value, field: &str) -> Result<String, UserDataParseError> {
        json.get(field)
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => v.to_string().trim_matches('"').to_string(),
            })
            .ok_or_else(|| UserDataParseError::MissingField(field.to_string()))
    }

    /// 辅助方法：提取f64字段（支持数字和字符串）
    fn extract_f64(&self, json: &Value, field: &str) -> Result<f64, UserDataParseError> {
        json.get(field)
            .and_then(|v| match v {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.parse().ok(),
                _ => None,
            })
            .ok_or_else(|| UserDataParseError::MissingField(field.to_string()))
    }

    /// 解析订单方向
    fn parse_order_side(&self, side_str: &str) -> Result<OrderSide, UserDataParseError> {
        match side_str.to_uppercase().as_str() {
            "BUY" => Ok(OrderSide::Buy),
            "SELL" => Ok(OrderSide::Sell),
            _ => Err(UserDataParseError::InvalidEnumValue(format!(
                "OrderSide: {side_str}"
            ))),
        }
    }

    /// 解析订单类型
    fn parse_order_type(&self, type_str: &str) -> Result<OrderType, UserDataParseError> {
        match type_str.to_uppercase().as_str() {
            "MARKET" => Ok(OrderType::Market),
            "LIMIT" => Ok(OrderType::Limit),
            "STOP" => Ok(OrderType::Stop),
            "STOP_MARKET" => Ok(OrderType::StopMarket),
            "TAKE_PROFIT" => Ok(OrderType::TakeProfit),
            "TAKE_PROFIT_MARKET" => Ok(OrderType::TakeProfitMarket),
            "TRAILING_STOP_MARKET" => Ok(OrderType::TrailingStopMarket),
            _ => Err(UserDataParseError::InvalidEnumValue(format!(
                "OrderType: {type_str}"
            ))),
        }
    }

    /// 解析订单状态
    fn parse_order_status(&self, status_str: &str) -> Result<OrderStatus, UserDataParseError> {
        match status_str.to_uppercase().as_str() {
            "NEW" => Ok(OrderStatus::New),
            "PARTIALLY_FILLED" => Ok(OrderStatus::PartiallyFilled),
            "FILLED" => Ok(OrderStatus::Filled),
            "CANCELED" => Ok(OrderStatus::Canceled),
            "REJECTED" => Ok(OrderStatus::Rejected),
            "EXPIRED" => Ok(OrderStatus::Expired),
            _ => Err(UserDataParseError::InvalidEnumValue(format!(
                "OrderStatus: {status_str}"
            ))),
        }
    }

    /// 解析持仓方向
    fn parse_position_side(&self, side_str: &str) -> Result<PositionSide, UserDataParseError> {
        match side_str.to_uppercase().as_str() {
            "BOTH" => Ok(PositionSide::Both),
            "LONG" => Ok(PositionSide::Long),
            "SHORT" => Ok(PositionSide::Short),
            _ => Err(UserDataParseError::InvalidEnumValue(format!(
                "PositionSide: {side_str}"
            ))),
        }
    }

    /// 确定余额变化原因
    fn determine_balance_change_reason(&self, json: &Value) -> BalanceChangeReason {
        if let Some(reason) = json.get("m").and_then(|v| v.as_str()) {
            match reason {
                "ORDER" => BalanceChangeReason::Trade,
                "FUNDING_FEE" => BalanceChangeReason::FundingFee,
                "WITHDRAW" | "DEPOSIT" => BalanceChangeReason::Transfer,
                _ => BalanceChangeReason::Other(reason.to_string()),
            }
        } else {
            BalanceChangeReason::Trade
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::events::UserDataEvent;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_user_data_parser_creation() {
        let (sender, _receiver) = mpsc::channel(10);
        let parser = UserDataParser::new(
            ExchangeType::Binance,
            MarketType::Spot,
            sender,
        );
        
        assert_eq!(parser.exchange, ExchangeType::Binance);
        assert_eq!(parser.market_type, MarketType::Spot);
    }
}

/// 用户数据路由器：负责从通道中消费用户事件并进行后续处理/转发
#[derive(Debug)]
pub struct UserDataRouter {
    receiver: mpsc::Receiver<UserDataEvent>,
}

impl UserDataRouter {
    /// 创建路由器
    pub fn new(receiver: mpsc::Receiver<UserDataEvent>) -> Self {
        Self { receiver }
    }

    /// 启动路由循环（最小实现：仅记录日志，后续可扩展为转发/落盘等）
    pub async fn start_routing(&mut self) {
        use log::info;
        info!("UserDataRouter 启动");
        while let Some(event) = self.receiver.recv().await {
            // 这里可以根据事件类型进行路由到不同的处理器
            // 目前最小实现：仅打印调试日志
            debug!("UserDataRouter 接收到事件: {:?}", event);
        }
        info!("UserDataRouter 已退出（通道关闭）");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::events::UserDataEvent;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_user_data_parser_creation() {
        let (sender, _receiver) = mpsc::channel(10);
        let parser = UserDataParser::new(
            ExchangeType::Binance,
            MarketType::Spot,
            sender,
        );
        
        assert_eq!(parser.exchange, ExchangeType::Binance);
        assert_eq!(parser.market_type, MarketType::Spot);
    }
}
