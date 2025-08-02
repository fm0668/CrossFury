//! Binance期货消息解析模块
//! 
//! 解析WebSocket接收到的各种期货数据消息

use crate::connectors::binance::futures::websocket::{AccountEvent, FuturesBalance, FuturesPosition, FuturesOrder};
use crate::connectors::binance::futures::config::PositionSide;
use crate::types::market_data::{MarketDataEvent, DepthUpdate, TradeUpdate, KlineUpdate, TickerUpdate, MarkPriceUpdate, OpenInterestUpdate, FundingRateUpdate, PriceLevel};
use crate::types::trading::{TradeEvent, OrderUpdate as TradingOrderUpdate, TradeExecution, PositionUpdate as TradingPositionUpdate, BalanceUpdate as TradingBalanceUpdate, OrderSide, OrderType, OrderStatus, PositionSide as TradingPositionSide, TimeInForce};
use crate::core::AppError;

// 定义Result类型别名
pub type Result<T> = std::result::Result<T, AppError>;

use serde_json::{Value, from_value};
use log::{debug, warn, error};
use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;

/// Binance期货消息解析器
pub struct BinanceFuturesMessageParser;

/// 解析后的消息类型
#[derive(Debug, Clone)]
pub enum ParsedMessage {
    /// 市场数据事件
    MarketData(MarketDataEvent),
    /// 交易事件
    Trading(TradeEvent),
    /// 订阅确认
    SubscriptionConfirm,
    /// 错误消息
    Error(String),
}

impl BinanceFuturesMessageParser {
    /// 解析WebSocket消息
    pub fn parse_message(message: &str) -> Result<Vec<ParsedMessage>> {
        let value: Value = serde_json::from_str(message)
            .map_err(|e| AppError::ParseError(format!("JSON解析失败: {}", e)))?;
        
        let mut results = Vec::new();
        
        // 检查是否是订阅确认消息
        if let Some(result) = value.get("result") {
            if result.is_null() {
                debug!("收到订阅确认消息");
                return Ok(vec![ParsedMessage::SubscriptionConfirm]);
            }
        }
        
        // 检查是否是错误消息
        if let Some(error) = value.get("error") {
            let error_msg = error.get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("未知错误");
            warn!("收到错误消息: {}", error_msg);
            return Ok(vec![ParsedMessage::Error(error_msg.to_string())]);
        }
        
        // 解析数据流消息
        if let Some(stream) = value.get("stream").and_then(|s| s.as_str()) {
            if let Some(data) = value.get("data") {
                if let Ok(parsed) = Self::parse_stream_data(stream, data) {
                    results.push(parsed);
                }
            }
        }
        
        // 解析用户数据流消息
        if let Some(event_type) = value.get("e").and_then(|e| e.as_str()) {
            if let Ok(parsed) = Self::parse_user_data(event_type, &value) {
                results.push(parsed);
            }
        }
        
        Ok(results)
    }
    
    /// 解析数据流消息
    fn parse_stream_data(stream: &str, data: &Value) -> Result<ParsedMessage> {
        if stream.contains("@depth") {
            Self::parse_depth_update(data)
        } else if stream.contains("@aggTrade") {
            Self::parse_trade_update(data)
        } else if stream.contains("@kline") {
            Self::parse_kline_update(data)
        } else if stream.contains("@ticker") {
            Self::parse_ticker_update(data)
        } else if stream.contains("@markPrice") {
            Self::parse_mark_price_update(data)
        } else if stream.contains("@openInterest") {
            Self::parse_open_interest_update(data)
        } else if stream == "!markPrice@arr" {
            Self::parse_funding_rate_update(data)
        } else {
            Err(AppError::ParseError(format!("未知的数据流类型: {}", stream)))
        }
    }
    
    /// 解析深度更新
    fn parse_depth_update(data: &Value) -> Result<ParsedMessage> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let event_time = data.get("E")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少事件时间字段".to_string()))?;
        
        let first_update_id = data.get("U")
            .and_then(|id| id.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少首次更新ID字段".to_string()))?;
        
        let final_update_id = data.get("u")
            .and_then(|id| id.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少最终更新ID字段".to_string()))?;
        
        // 解析买单
        let bids = data.get("b")
            .and_then(|b| b.as_array())
            .ok_or_else(|| AppError::ParseError("缺少买单数据".to_string()))?
            .iter()
            .filter_map(|bid| {
                if let Some(bid_array) = bid.as_array() {
                    if bid_array.len() >= 2 {
                        let price = bid_array[0].as_str()?.parse::<f64>().ok()?;
                        let quantity = bid_array[1].as_str()?.parse::<f64>().ok()?;
                        Some(PriceLevel { price, quantity })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        
        // 解析卖单
        let asks = data.get("a")
            .and_then(|a| a.as_array())
            .ok_or_else(|| AppError::ParseError("缺少卖单数据".to_string()))?
            .iter()
            .filter_map(|ask| {
                if let Some(ask_array) = ask.as_array() {
                    if ask_array.len() >= 2 {
                        let price = ask_array[0].as_str()?.parse::<f64>().ok()?;
                        let quantity = ask_array[1].as_str()?.parse::<f64>().ok()?;
                        Some(PriceLevel { price, quantity })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        
        let depth_update = DepthUpdate {
            symbol: symbol.to_string(),
            first_update_id,
            final_update_id,
            event_time,
            depth_bids: bids.clone(),
            depth_asks: asks.clone(),
            best_bid_price: bids.first().map(|b| b.price).unwrap_or(0.0),
            best_ask_price: asks.first().map(|a| a.price).unwrap_or(0.0),
        };
        
        Ok(ParsedMessage::MarketData(MarketDataEvent::DepthUpdate(depth_update)))
    }
    
    /// 解析交易更新
    fn parse_trade_update(data: &Value) -> Result<ParsedMessage> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let trade_id = data.get("a")
            .and_then(|id| id.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少交易ID字段".to_string()))?;
        
        let price = data.get("p")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少价格字段".to_string()))?;
        
        let quantity = data.get("q")
            .and_then(|q| q.as_str())
            .and_then(|q| q.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少数量字段".to_string()))?;
        
        let timestamp = data.get("T")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少时间戳字段".to_string()))?;
        
        let is_buyer_maker = data.get("m")
            .and_then(|m| m.as_bool())
            .unwrap_or(false);
        
        let trade_update = TradeUpdate {
            symbol: symbol.to_string(),
            trade_id,
            price,
            quantity,
            timestamp,
            is_buyer_maker,
        };
        
        Ok(ParsedMessage::MarketData(MarketDataEvent::TradeUpdate(trade_update)))
    }
    
    /// 解析K线更新
    fn parse_kline_update(data: &Value) -> Result<ParsedMessage> {
        let kline_data = data.get("k")
            .ok_or_else(|| AppError::ParseError("缺少K线数据".to_string()))?;
        
        let symbol = kline_data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let open_time = kline_data.get("t")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少开盘时间字段".to_string()))?;
        
        let close_time = kline_data.get("T")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少收盘时间字段".to_string()))?;
        
        let interval = kline_data.get("i")
            .and_then(|i| i.as_str())
            .ok_or_else(|| AppError::ParseError("缺少时间间隔字段".to_string()))?;
        
        let open_price = kline_data.get("o")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少开盘价字段".to_string()))?;
        
        let high_price = kline_data.get("h")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少最高价字段".to_string()))?;
        
        let low_price = kline_data.get("l")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少最低价字段".to_string()))?;
        
        let close_price = kline_data.get("c")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少收盘价字段".to_string()))?;
        
        let volume = kline_data.get("v")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少成交量字段".to_string()))?;
        
        let quote_volume = kline_data.get("q")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少成交额字段".to_string()))?;
        
        let is_closed = kline_data.get("x")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        
        let kline_update = KlineUpdate {
            symbol: symbol.to_string(),
            open_time,
            close_time,
            interval: interval.to_string(),
            open_price,
            high_price,
            low_price,
            close_price,
            volume,
            quote_volume,
            is_closed,
        };
        
        Ok(ParsedMessage::MarketData(MarketDataEvent::KlineUpdate(kline_update)))
    }
    
    /// 解析24小时价格统计更新
    fn parse_ticker_update(data: &Value) -> Result<ParsedMessage> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let price_change = data.get("p")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少价格变化字段".to_string()))?;
        
        let price_change_percent = data.get("P")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少价格变化百分比字段".to_string()))?;
        
        let last_price = data.get("c")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少最新价格字段".to_string()))?;
        
        let volume = data.get("v")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少成交量字段".to_string()))?;
        
        let quote_volume = data.get("q")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少成交额字段".to_string()))?;
        
        let high_price = data.get("h")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少最高价字段".to_string()))?;
        
        let low_price = data.get("l")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少最低价字段".to_string()))?;
        
        let open_price = data.get("o")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少开盘价字段".to_string()))?;
        
        let ticker_update = TickerUpdate {
            symbol: symbol.to_string(),
            price_change,
            price_change_percent,
            last_price,
            volume,
            quote_volume,
            high_price,
            low_price,
            open_price,
        };
        
        Ok(ParsedMessage::MarketData(MarketDataEvent::TickerUpdate(ticker_update)))
    }
    
    /// 解析标记价格更新
    fn parse_mark_price_update(data: &Value) -> Result<ParsedMessage> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let mark_price = data.get("p")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少标记价格字段".to_string()))?;
        
        let index_price = data.get("i")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let funding_rate = data.get("r")
            .and_then(|r| r.as_str())
            .and_then(|r| r.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let next_funding_time = data.get("T")
            .and_then(|t| t.as_i64())
            .unwrap_or(0);
        
        let mark_price_update = MarkPriceUpdate {
            symbol: symbol.to_string(),
            mark_price,
            index_price,
            funding_rate,
            next_funding_time,
        };
        
        Ok(ParsedMessage::MarketData(MarketDataEvent::MarkPriceUpdate(mark_price_update)))
    }
    
    /// 解析持仓量更新
    fn parse_open_interest_update(data: &Value) -> Result<ParsedMessage> {
        let symbol = data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let open_interest = data.get("o")
            .and_then(|o| o.as_str())
            .and_then(|o| o.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少持仓量字段".to_string()))?;
        
        let timestamp = data.get("E")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少时间戳字段".to_string()))?;
        
        let open_interest_update = OpenInterestUpdate {
            symbol: symbol.to_string(),
            open_interest,
            timestamp,
        };
        
        Ok(ParsedMessage::MarketData(MarketDataEvent::OpenInterestUpdate(open_interest_update)))
    }
    
    /// 解析资金费率更新
    fn parse_funding_rate_update(data: &Value) -> Result<ParsedMessage> {
        if let Some(array) = data.as_array() {
            if let Some(first_item) = array.first() {
                let symbol = first_item.get("s")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
                
                let funding_rate = first_item.get("r")
                    .and_then(|r| r.as_str())
                    .and_then(|r| r.parse::<f64>().ok())
                    .ok_or_else(|| AppError::ParseError("缺少资金费率字段".to_string()))?;
                
                let funding_time = first_item.get("T")
                    .and_then(|t| t.as_i64())
                    .ok_or_else(|| AppError::ParseError("缺少资金费率时间字段".to_string()))?;
                
                let funding_rate_update = FundingRateUpdate {
                    symbol: symbol.to_string(),
                    funding_rate,
                    funding_time,
                };
                
                return Ok(ParsedMessage::MarketData(MarketDataEvent::FundingRateUpdate(funding_rate_update)));
            }
        }
        
        Err(AppError::ParseError("无效的资金费率数据格式".to_string()))
    }
    
    /// 解析用户数据
    fn parse_user_data(event_type: &str, data: &Value) -> Result<ParsedMessage> {
        match event_type {
            "ACCOUNT_UPDATE" => Self::parse_account_update(data),
            "ORDER_TRADE_UPDATE" => Self::parse_order_update(data),
            "MARGIN_CALL" => Self::parse_margin_call(data),
            "ACCOUNT_CONFIG_UPDATE" => Self::parse_account_config_update(data),
            _ => Err(AppError::ParseError(format!("未知的用户数据事件类型: {}", event_type))),
        }
    }
    
    /// 解析账户更新
    fn parse_account_update(data: &Value) -> Result<ParsedMessage> {
        let event_time = data.get("E")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少事件时间字段".to_string()))?;
        
        let transaction_time = data.get("T")
            .and_then(|t| t.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少交易时间字段".to_string()))?;
        
        let account_data = data.get("a")
            .ok_or_else(|| AppError::ParseError("缺少账户数据字段".to_string()))?;
        
        // 解析余额更新
        let mut balance_updates = Vec::new();
        if let Some(balances) = account_data.get("B").and_then(|b| b.as_array()) {
            for balance in balances {
                if let (Some(asset), Some(wallet_balance), Some(cross_wallet_balance)) = (
                    balance.get("a").and_then(|a| a.as_str()),
                    balance.get("wb").and_then(|wb| wb.as_str()).and_then(|wb| wb.parse::<f64>().ok()),
                    balance.get("cw").and_then(|cw| cw.as_str()).and_then(|cw| cw.parse::<f64>().ok()),
                ) {
                    let balance_change = balance.get("bc").and_then(|bc| bc.as_str()).and_then(|bc| bc.parse::<f64>().ok()).unwrap_or(0.0);
                    
                    balance_updates.push(TradingBalanceUpdate {
                        asset: asset.to_string(),
                        wallet_balance,
                        cross_wallet_balance,
                        balance_change,
                        timestamp: event_time as u64,
                    });
                }
            }
        }
        
        // 解析持仓更新
        let mut position_updates = Vec::new();
        if let Some(positions) = account_data.get("P").and_then(|p| p.as_array()) {
            for position in positions {
                if let (Some(symbol), Some(position_amount), Some(entry_price), Some(unrealized_pnl)) = (
                    position.get("s").and_then(|s| s.as_str()),
                    position.get("pa").and_then(|pa| pa.as_str()).and_then(|pa| pa.parse::<f64>().ok()),
                    position.get("ep").and_then(|ep| ep.as_str()).and_then(|ep| ep.parse::<f64>().ok()),
                    position.get("up").and_then(|up| up.as_str()).and_then(|up| up.parse::<f64>().ok()),
                ) {
                    let position_side = position.get("ps").and_then(|ps| ps.as_str()).unwrap_or("BOTH");
                    let margin_type = position.get("mt").and_then(|mt| mt.as_str()).unwrap_or("cross");
                    
                    position_updates.push(TradingPositionUpdate {
                        symbol: symbol.to_string(),
                        position_side: match position_side {
                            "LONG" => TradingPositionSide::Long,
                            "SHORT" => TradingPositionSide::Short,
                            _ => TradingPositionSide::Both,
                        },
                        position_amount,
                        entry_price,
                        mark_price: 0.0, // 标记价格需要从其他地方获取
                        unrealized_pnl,
                        percentage: 0.0, // 百分比需要计算
                        timestamp: event_time as u64,
                    });
                }
            }
        }
        
        if !balance_updates.is_empty() {
            for balance_update in balance_updates {
                return Ok(ParsedMessage::Trading(TradeEvent::BalanceUpdate(balance_update)));
            }
        }
        
        if !position_updates.is_empty() {
            for position_update in position_updates {
                return Ok(ParsedMessage::Trading(TradeEvent::PositionUpdate(position_update)));
            }
        }
        
        Err(AppError::ParseError("无效的账户更新数据".to_string()))
    }
    
    /// 解析订单更新
    fn parse_order_update(data: &Value) -> Result<ParsedMessage> {
        let order_data = data.get("o")
            .ok_or_else(|| AppError::ParseError("缺少订单数据字段".to_string()))?;
        
        let symbol = order_data.get("s")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少交易对字段".to_string()))?;
        
        let order_id = order_data.get("i")
            .and_then(|i| i.as_i64())
            .ok_or_else(|| AppError::ParseError("缺少订单ID字段".to_string()))?;
        
        let client_order_id = order_data.get("c")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        
        let side = order_data.get("S")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::ParseError("缺少订单方向字段".to_string()))?;
        
        let order_type = order_data.get("o")
            .and_then(|o| o.as_str())
            .ok_or_else(|| AppError::ParseError("缺少订单类型字段".to_string()))?;
        
        let quantity = order_data.get("q")
            .and_then(|q| q.as_str())
            .and_then(|q| q.parse::<f64>().ok())
            .ok_or_else(|| AppError::ParseError("缺少订单数量字段".to_string()))?;
        
        let price = order_data.get("p")
            .and_then(|p| p.as_str())
            .and_then(|p| p.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let status = order_data.get("X")
            .and_then(|x| x.as_str())
            .ok_or_else(|| AppError::ParseError("缺少订单状态字段".to_string()))?;
        
        let executed_quantity = order_data.get("z")
            .and_then(|z| z.as_str())
            .and_then(|z| z.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let last_executed_quantity = order_data.get("l")
            .and_then(|l| l.as_str())
            .and_then(|l| l.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let last_executed_price = order_data.get("L")
            .and_then(|l| l.as_str())
            .and_then(|l| l.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let commission = order_data.get("n")
            .and_then(|n| n.as_str())
            .and_then(|n| n.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let commission_asset = order_data.get("N")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        
        let trade_id = order_data.get("t")
            .and_then(|t| t.as_i64())
            .unwrap_or(0);
        
        let order_update = TradingOrderUpdate {
            symbol: symbol.to_string(),
            order_id: order_id.to_string(),
            client_order_id: client_order_id.to_string(),
            side: match side {
                "BUY" => OrderSide::Buy,
                "SELL" => OrderSide::Sell,
                _ => OrderSide::Buy,
            },
            order_type: match order_type {
                "LIMIT" => OrderType::Limit,
                "MARKET" => OrderType::Market,
                "STOP" => OrderType::Stop,
                "STOP_MARKET" => OrderType::StopMarket,
                "TAKE_PROFIT" => OrderType::TakeProfit,
                "TAKE_PROFIT_MARKET" => OrderType::TakeProfitMarket,
                _ => OrderType::Limit,
            },
            status: match status {
                "NEW" => OrderStatus::New,
                "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
                "FILLED" => OrderStatus::Filled,
                "CANCELED" => OrderStatus::Canceled,
                "REJECTED" => OrderStatus::Rejected,
                "EXPIRED" => OrderStatus::Expired,
                _ => OrderStatus::New,
            },
            quantity,
            price,
            executed_quantity,
            executed_price: last_executed_price,
            timestamp: data.get("E").and_then(|e| e.as_i64()).unwrap_or(0) as u64,
            time_in_force: TimeInForce::GTC, // 默认值，需要从订单数据中获取
            reduce_only: false, // 默认值，需要从订单数据中获取
            close_position: false, // 默认值，需要从订单数据中获取
        };
        
        // 如果有成交，创建成交事件
        if last_executed_quantity > 0.0 {
            let trade_execution = TradeExecution {
                symbol: symbol.to_string(),
                trade_id: trade_id.to_string(),
                order_id: order_id.to_string(),
                side: order_update.side.clone(),
                quantity: last_executed_quantity,
                price: last_executed_price,
                commission,
                commission_asset: commission_asset.to_string(),
                timestamp: order_update.timestamp as u64,
                is_maker: false, // 需要从订单数据中获取
            };
            
            return Ok(ParsedMessage::Trading(TradeEvent::TradeExecution(trade_execution)));
        }
        
        Ok(ParsedMessage::Trading(TradeEvent::OrderUpdate(order_update)))
    }
    
    /// 解析保证金追缴
    fn parse_margin_call(data: &Value) -> Result<ParsedMessage> {
        // 这里可以根据需要实现保证金追缴的解析逻辑
        warn!("收到保证金追缴通知: {:?}", data);
        Ok(ParsedMessage::Error("保证金追缴".to_string()))
    }
    
    /// 解析账户配置更新
    fn parse_account_config_update(data: &Value) -> Result<ParsedMessage> {
        // 这里可以根据需要实现账户配置更新的解析逻辑
        debug!("收到账户配置更新: {:?}", data);
        Ok(ParsedMessage::SubscriptionConfirm)
    }
}
