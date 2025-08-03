// bybit_futures_websocket.rs - Bybit Futures WebSocket handler
// Handles real-time perpetual contract orderbook data from Bybit V5 API

use crate::{AppState, OrderbookUpdate};
use crate::utils::ensure_exchange_prefix;
use crate::config::get_config;
use log::{info, error};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::time::sleep;

/// Bybit Futures WebSocket handler for perpetual contracts
pub async fn bybit_futures_websocket_handler(
    symbols: Vec<String>,
    connection_id: usize,
    app_state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = get_config();
    let ws_url = &config.exchanges.get("BYBIT_FUTURES").unwrap().websocket_url;
    
    info!("BYBIT_FUTURES {}: Starting connection with {} symbols", connection_id, symbols.len());
    
    loop {
        match connect_async(ws_url).await {
            Ok((ws_stream, _)) => {
                info!("BYBIT_FUTURES {connection_id}: Connected successfully");
                let (mut write, mut read) = ws_stream.split();

                // Subscribe to orderbook data for all symbols
                for symbol in &symbols {
                    let subscribe_msg = json!({
                        "op": "subscribe",
                        "args": [format!("orderbook.20.{}", symbol)]
                    });

                    if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                        error!("BYBIT_FUTURES {connection_id}: Failed to subscribe {symbol}: {e}");
                    }
                    sleep(Duration::from_millis(10)).await;
                }
                
                // Process incoming messages
                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                process_bybit_futures_message(connection_id, &app_state, &data).await;
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            let _ = write.send(Message::Pong(payload)).await;
                        }
                        Err(e) => {
                            error!("BYBIT_FUTURES {connection_id}: WebSocket error: {e}");
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("BYBIT_FUTURES {connection_id}: Connection failed: {e}");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Process incoming messages from Bybit Futures WebSocket
async fn process_bybit_futures_message(
    connection_id: usize,
    app_state: &AppState,
    data: &Value,
) {
    if let Some(topic) = data.get("topic").and_then(|t| t.as_str()) {
        if topic.starts_with("orderbook.20.") {
            if let Some(data_obj) = data.get("data") {
                process_bybit_depth_update(connection_id, app_state, data_obj, topic).await;
            }
        }
    }
}

/// Process depth update from Bybit Futures
async fn process_bybit_depth_update(
    connection_id: usize,
    app_state: &AppState,
    data: &Value,
    topic: &str,
) {
    let symbol = topic.replace("orderbook.20.", "");
    let prefixed_symbol = ensure_exchange_prefix(&symbol, "BYBIT_FUTURES");

    if let (Some(bids), Some(asks)) = (
        data.get("b").and_then(|b| b.as_array()),
        data.get("a").and_then(|a| a.as_array())
    ) {
        if let (Some(best_bid), Some(best_ask)) = (
            bids.first()
                .and_then(|b| b.as_array())
                .and_then(|arr| arr[0].as_str())
                .and_then(|s| s.parse::<f64>().ok()),
            asks.first()
                .and_then(|a| a.as_array())
                .and_then(|arr| arr[0].as_str())
                .and_then(|s| s.parse::<f64>().ok())
        ) {
            let update = OrderbookUpdate {
                symbol: prefixed_symbol.clone(),
                best_bid,
                best_ask,
                timestamp: chrono::Utc::now().timestamp_millis(),
                scale: 8,
                is_synthetic: false,
                leg1: None,
                leg2: None,
                depth_asks: Some(parse_depth_levels(asks)),
                depth_bids: Some(parse_depth_levels(bids)),
            };

            if let Some(tx) = &app_state.orderbook_queue {
                if let Err(e) = tx.send(update) {
                    error!("BYBIT_FUTURES {connection_id}: Failed to send update: {e}");
                }
            }
        }
    }
}

fn parse_depth_levels(levels: &Vec<Value>) -> Vec<(f64, f64)> {
    levels.iter()
        .filter_map(|level| {
            if let Some(arr) = level.as_array() {
                if arr.len() >= 2 {
                    let price = arr[0].as_str()?.parse::<f64>().ok()?;
                    let quantity = arr[1].as_str()?.parse::<f64>().ok()?;
                    Some((price, quantity))
                } else { None }
            } else { None }
        })
        .collect()
}
