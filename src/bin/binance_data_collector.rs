//! 币安数据收集器
//! 收集现货和期货市场数据并保存为JSON格式

use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use log::{info, error, debug, warn};
use serde::Serialize;
use tokio::{
    sync::mpsc,
    time::{interval, sleep},
    select,
};

use trifury::config::get_config;
use trifury::{
    types::errors::AppError,
    connectors::{
        binance::{
            futures::{
                connector::BinanceFuturesConnector,
                config::BinanceFuturesConfig,
            },
        },
    },
    sinks::{
        DataSink,
        factory::{SinkConfig, SinkFactory},
        write_record_serialized,
    },
    types::{
        market_data::{MarketDataEvent, StandardizedTrade, PriceLevel},
        trading::TradeEvent,
    },
};

/// 订单簿数据JSON记录
#[derive(Debug, Serialize)]
struct OrderBookRecord {
    timestamp: String,
    symbol: String,
    market_type: String, // "spot" or "futures"
    bids: Vec<[f64; 2]>, // [price, quantity]
    asks: Vec<[f64; 2]>, // [price, quantity]
}

/// 交易数据JSON记录
#[derive(Debug, Serialize)]
struct TradeRecord {
    timestamp: String,
    symbol: String,
    market_type: String,
    price: f64,
    quantity: f64,
    side: String, // "buy" or "sell"
    trade_id: String,
}

/// 标记价格JSON记录
#[derive(Debug, Serialize)]
struct MarkPriceRecord {
    timestamp: String,
    symbol: String,
    mark_price: f64,
    index_price: f64,
    estimated_settle_price: f64,
    funding_rate: f64,
    next_funding_time: String,
}

/// 资金费率JSON记录
#[derive(Debug, Serialize)]
struct FundingRateRecord {
    timestamp: String,
    symbol: String,
    funding_rate: f64,
    funding_time: String,
}

/// 未平仓合约JSON记录
#[derive(Debug, Serialize)]
struct OpenInterestRecord {
    timestamp: String,
    symbol: String,
    open_interest: f64,
    open_interest_value: f64,
}

/// 本地OrderBook结构体
#[derive(Debug, Clone, Serialize)]
struct OrderBook {
    symbol: String,
    bids: Vec<PriceLevel>,
    asks: Vec<PriceLevel>,
    timestamp: i64,
}

// 旧的BatchedJsonWriter已被新的Sink抽象替代





/// 数据收集器
struct DataCollector {
    futures_symbols: Vec<String>,
    collection_duration: Duration,
    data_dir: String,
}

impl DataCollector {
    fn new() -> Self {
        Self {
            futures_symbols: vec![
                "BTCUSDT".to_string(),
                "ETHUSDT".to_string(),
                "BNBUSDT".to_string(),
                "ADAUSDT".to_string(),
                "SOLUSDT".to_string(),
            ],
            collection_duration: Duration::from_secs(0), // 0表示持续运行，不自动退出
            data_dir: "data/binance".to_string(),
        }
    }

    /// 创建Sink实例
    fn create_sink(&self, filename: &str) -> Result<Box<dyn DataSink>, AppError> {
        let path = Path::new(&self.data_dir).join(filename);
        
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::ConfigError(format!("创建目录失败: {e}")))?;
        }
        
        let cfg = get_config().data_collector.clone();
        let sink_config = SinkConfig::File {
            data_dir: path.parent().unwrap_or_else(|| std::path::Path::new("./data")).to_string_lossy().to_string(),
            batch_size: Some(cfg.batch_write_buffer_size),
            flush_interval_ms: Some(cfg.flush_interval_ms),
            rotation_size_bytes: None,
            enable_compression: Some(false),
        };
        
        SinkFactory::create_sink(sink_config)
    }

    /// 收集期货数据
    async fn collect_futures_data(&self) -> Result<(), AppError> {
        info!("开始收集期货市场数据...");

        // 创建Sink实例
        let mut orderbook_sink = self.create_sink("futures/orderbook_5level.jsonl")?;
        let mut trade_sink = self.create_sink("futures/trades.jsonl")?;
        let mut mark_price_sink = self.create_sink("futures/mark_price.jsonl")?;
        let mut funding_rate_sink = self.create_sink("futures/funding_rate.jsonl")?;
        let mut open_interest_sink = self.create_sink("futures/open_interest.jsonl")?;
        
        // 初始化所有Sink
        orderbook_sink.initialize().await?;
        trade_sink.initialize().await?;
        mark_price_sink.initialize().await?;
        funding_rate_sink.initialize().await?;
        open_interest_sink.initialize().await?;

        // 创建通道
        let cfg = get_config().data_collector.clone();
        let (market_tx, mut market_rx) = mpsc::channel::<MarketDataEvent>(cfg.market_data_channel_buffer);
        let (trade_tx, mut trade_rx) = mpsc::channel::<TradeEvent>(cfg.trade_event_channel_buffer);
        let mut flush_timer = interval(Duration::from_millis(cfg.flush_interval_ms));

        // 创建期货连接器配置（实盘环境）
        let futures_config = BinanceFuturesConfig::builder()
            .testnet(false)  // 实盘环境
            .build();

        let mut futures_connector = BinanceFuturesConnector::new(futures_config);

        // 设置数据发送器
        futures_connector.set_market_data_sender(market_tx);
        futures_connector.set_trade_event_sender(trade_tx);

        // 连接并订阅数据
        futures_connector.connect().await
            .map_err(|e| AppError::ConfigError(format!("期货连接器连接失败: {e:?}")))?;

        // 订阅各种数据类型
        for symbol in &self.futures_symbols {
            // 订阅订单簿和交易数据
            if let Err(e) = futures_connector.subscribe_symbol_data(symbol).await {
                warn!("订阅 {symbol} 数据失败: {e:?}");
            }
            
            // 订阅未平仓合约数据
            if let Err(e) = futures_connector.subscribe_open_interest(symbol).await {
                warn!("订阅 {symbol} 未平仓合约数据失败: {e:?}");
            }
        }
        
        // 订阅资金费率（全局订阅）
        if let Err(e) = futures_connector.subscribe_funding_rates().await {
            warn!("订阅资金费率失败: {e:?}");
        }

        info!("期货连接器初始化并订阅成功");

        let start_time = Instant::now();
        let mut data_counts = HashMap::new();

        // 数据收集循环
        loop {
            select! {
                Some(market_event) = market_rx.recv() => {
                    debug!("收到市场数据事件: {:?}", std::mem::discriminant(&market_event));
                    match market_event {
                        MarketDataEvent::DepthUpdate(depth_update) => {
                            // 将DepthUpdate转换为OrderBook格式
                            let orderbook = OrderBook {
                                symbol: depth_update.symbol.clone(),
                                bids: depth_update.depth_bids.iter().map(|level| PriceLevel {
                                    price: level.price,
                                    quantity: level.quantity,
                                }).collect(),
                                asks: depth_update.depth_asks.iter().map(|level| PriceLevel {
                                    price: level.price,
                                    quantity: level.quantity,
                                }).collect(),
                                timestamp: depth_update.event_time,
                            };
                            let timestamp = chrono::DateTime::from_timestamp(depth_update.event_time / 1000, 0)
                                .unwrap_or_else(Utc::now);
                            let record = self.create_orderbook_record(&depth_update.symbol, &orderbook, timestamp, "futures");
                            if let Err(e) = write_record_serialized(orderbook_sink.as_mut(), &record).await {
                                error!("写入期货订单簿数据失败: {e}");
                            } else {
                                *data_counts.entry("orderbook").or_insert(0) += 1;
                                debug!("成功写入订单簿数据: {} (总计: {})", depth_update.symbol, data_counts.get("orderbook").unwrap_or(&0));
                                // 立即刷新到磁盘
                                if let Err(e) = orderbook_sink.flush().await {
                                    error!("刷新订单簿文件失败: {e}");
                                }
                            }
                        }
                        MarketDataEvent::MarkPriceUpdate(mark_price_update) => {
                            let timestamp = chrono::DateTime::from_timestamp(mark_price_update.next_funding_time / 1000, 0)
                                .unwrap_or_else(Utc::now);
                            let record = MarkPriceRecord {
                                timestamp: timestamp.to_rfc3339(),
                                symbol: mark_price_update.symbol,
                                mark_price: mark_price_update.mark_price,
                                index_price: mark_price_update.index_price,
                                estimated_settle_price: 0.0, // 币安API可能不提供
                                funding_rate: mark_price_update.funding_rate,
                                next_funding_time: chrono::DateTime::from_timestamp(mark_price_update.next_funding_time / 1000, 0)
                                    .unwrap_or_else(Utc::now).to_rfc3339(),
                            };
                            if let Err(e) = write_record_serialized(mark_price_sink.as_mut(), &record).await {
                                error!("写入标记价格数据失败: {e}");
                            } else {
                                *data_counts.entry("mark_price").or_insert(0) += 1;
                                debug!("成功写入标记价格数据: {} mark_price={}", record.symbol, record.mark_price);
                            }
                        }
                        MarketDataEvent::FundingRateUpdate(funding_rate_update) => {
                            let timestamp = chrono::DateTime::from_timestamp(funding_rate_update.funding_time / 1000, 0)
                                .unwrap_or_else(Utc::now);
                            let record = FundingRateRecord {
                                timestamp: timestamp.to_rfc3339(),
                                symbol: funding_rate_update.symbol,
                                funding_rate: funding_rate_update.funding_rate,
                                funding_time: timestamp.to_rfc3339(),
                            };
                            if let Err(e) = write_record_serialized(funding_rate_sink.as_mut(), &record).await {
                                error!("写入资金费率数据失败: {e}");
                            } else {
                                *data_counts.entry("funding_rate").or_insert(0) += 1;
                                debug!("成功写入资金费率数据: {} funding_rate={}", record.symbol, record.funding_rate);
                            }
                        }
                        MarketDataEvent::OpenInterestUpdate(open_interest_update) => {
                            let timestamp = chrono::DateTime::from_timestamp(open_interest_update.timestamp / 1000, 0)
                                .unwrap_or_else(Utc::now);
                            let record = OpenInterestRecord {
                                timestamp: timestamp.to_rfc3339(),
                                symbol: open_interest_update.symbol,
                                open_interest: open_interest_update.open_interest,
                                open_interest_value: 0.0, // 如果API不提供则设为0
                            };
                            if let Err(e) = write_record_serialized(open_interest_sink.as_mut(), &record).await {
                                error!("写入未平仓合约数据失败: {e}");
                            } else {
                                *data_counts.entry("open_interest").or_insert(0) += 1;
                                debug!("成功写入未平仓合约数据: {} open_interest={}", record.symbol, record.open_interest);
                            }
                        }
                        _ => {}
                    }
                }
                Some(trade_event) = trade_rx.recv() => {
                    if let TradeEvent::TradeExecution(trade_execution) = trade_event {
                        let trade = StandardizedTrade {
                            symbol: trade_execution.symbol.clone(),
                            exchange: trifury::types::exchange::ExchangeType::Binance,
                            price: trade_execution.price,
                            quantity: trade_execution.quantity,
                            side: match trade_execution.side {
                                trifury::types::trading::OrderSide::Buy => trifury::types::market_data::TradeSide::Buy,
                                trifury::types::trading::OrderSide::Sell => trifury::types::market_data::TradeSide::Sell,
                            },
                            timestamp: trade_execution.timestamp as i64,
                            trade_id: trade_execution.trade_id.clone(),
                        };
                        let timestamp = chrono::DateTime::from_timestamp(trade_execution.timestamp as i64 / 1000, 0)
                            .unwrap_or_else(Utc::now);
                        let record = self.create_trade_record(&trade_execution.symbol, &trade, timestamp, "futures");
                        if let Err(e) = write_record_serialized(trade_sink.as_mut(), &record).await {
                            error!("写入期货交易数据失败: {e}");
                        } else {
                            *data_counts.entry("trade").or_insert(0) += 1;
                            debug!("成功写入交易数据: {} {}@{} side={}", record.symbol, record.price, record.quantity, record.side);
                        }
                    }
                }
                _ = flush_timer.tick() => {
                    if let Err(e) = orderbook_sink.flush().await { warn!("定时刷新订单簿Sink失败: {e}"); }
                    if let Err(e) = trade_sink.flush().await { warn!("定时刷新交易Sink失败: {e}"); }
                    if let Err(e) = mark_price_sink.flush().await { warn!("定时刷新标记价格Sink失败: {e}"); }
                    if let Err(e) = funding_rate_sink.flush().await { warn!("定时刷新资金费率Sink失败: {e}"); }
                    if let Err(e) = open_interest_sink.flush().await { warn!("定时刷新未平仓Sink失败: {e}"); }
                }
                _ = sleep(Duration::from_millis(100)) => {
                    // 如果collection_duration为0，则持续运行；否则检查时间限制
                    if self.collection_duration.as_secs() > 0 && start_time.elapsed() >= self.collection_duration {
                        break;
                    }
                }
            }
        }

        // 刷新并关闭所有Sink
        orderbook_sink.flush().await.map_err(|e| AppError::ConfigError(format!("刷新期货订单簿Sink失败: {e}")))?;
        trade_sink.flush().await.map_err(|e| AppError::ConfigError(format!("刷新期货交易Sink失败: {e}")))?;
        mark_price_sink.flush().await.map_err(|e| AppError::ConfigError(format!("刷新标记价格Sink失败: {e}")))?;
        funding_rate_sink.flush().await.map_err(|e| AppError::ConfigError(format!("刷新资金费率Sink失败: {e}")))?;
        open_interest_sink.flush().await.map_err(|e| AppError::ConfigError(format!("刷新未平仓合约Sink失败: {e}")))?;
        
        // 关闭所有Sink
        orderbook_sink.close().await.map_err(|e| AppError::ConfigError(format!("关闭期货订单簿Sink失败: {e}")))?;
        trade_sink.close().await.map_err(|e| AppError::ConfigError(format!("关闭期货交易Sink失败: {e}")))?;
        mark_price_sink.close().await.map_err(|e| AppError::ConfigError(format!("关闭标记价格Sink失败: {e}")))?;
        funding_rate_sink.close().await.map_err(|e| AppError::ConfigError(format!("关闭资金费率Sink失败: {e}")))?;
        open_interest_sink.close().await.map_err(|e| AppError::ConfigError(format!("关闭未平仓合约Sink失败: {e}")))?;

        // 断开连接
        if let Err(e) = futures_connector.disconnect().await {
            warn!("期货连接器断开连接失败: {e:?}");
        }

        info!("期货数据收集完成: {data_counts:?}");
        Ok(())
    }

    /// 创建订单簿记录
    fn create_orderbook_record(&self, symbol: &str, orderbook: &OrderBook, timestamp: DateTime<Utc>, market_type: &str) -> OrderBookRecord {
        // 转换买盘数据（取前20档）
        let bids: Vec<[f64; 2]> = orderbook.bids.iter()
            .take(20)
            .map(|level| [level.price, level.quantity])
            .collect();
        
        // 转换卖盘数据（取前20档）
        let asks: Vec<[f64; 2]> = orderbook.asks.iter()
            .take(20)
            .map(|level| [level.price, level.quantity])
            .collect();
        
        OrderBookRecord {
            timestamp: timestamp.to_rfc3339(),
            symbol: symbol.to_string(),
            market_type: market_type.to_string(),
            bids,
            asks,
        }
    }

    /// 创建交易记录
    fn create_trade_record(&self, symbol: &str, trade: &StandardizedTrade, timestamp: DateTime<Utc>, market_type: &str) -> TradeRecord {
        TradeRecord {
            timestamp: timestamp.to_rfc3339(),
            symbol: symbol.to_string(),
            market_type: market_type.to_string(),
            price: trade.price,
            quantity: trade.quantity,
            side: match trade.side {
                trifury::types::market_data::TradeSide::Buy => "buy".to_string(),
                trifury::types::market_data::TradeSide::Sell => "sell".to_string(),
            },
            trade_id: trade.trade_id.clone(),
        }
    }

    /// 运行数据收集
    async fn run(&self) -> Result<(), AppError> {
        info!("=== 币安数据收集器启动 ===");
        if self.collection_duration.as_secs() == 0 {
            info!("收集模式: 持续运行（不自动退出）");
        } else {
            info!("收集时长: {} 秒", self.collection_duration.as_secs());
        }
        info!("期货交易对: {:?}", self.futures_symbols);
        info!("数据保存目录: {}", self.data_dir);

        // 只收集期货数据
        let futures_result = self.collect_futures_data().await;

        if let Err(e) = futures_result {
            error!("期货数据收集失败: {e:?}");
        }

        info!("=== 数据收集完成 ===");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .init();

    let collector = DataCollector::new();
    collector.run().await
}