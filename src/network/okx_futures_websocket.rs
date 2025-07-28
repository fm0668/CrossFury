// okx_futures_websocket.rs - OKX Futures WebSocket handler
// Handles real-time perpetual contract orderbook data from OKX V5 API

use crate::{AppState, OrderbookUpdate};
use crate::utils::ensure_exchange_prefix;
use crate::config::get_config;
use log::{info, error, debug};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::time::sleep;

/// OKX Futures WebSocket handler for perpetual contracts
pub async fn okx_futures_websocket_handler(
    symbols: Vec<String>,
    connection_id: usize,
    app_state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = get_config();
    let ws_url = &config.exchanges.get("OKX_FUTURES").unwrap().websocket_url;
    
    info!("OKX_FUTURES {}: Starting connection with {} symbols", connection_id, symbols.len());
    
    loop {
        match connect_async(ws_url).await {
            Ok((ws_stream, _)) => {
                info!("OKX_FUTURES {}: Connected successfully", connection_id);
                let (mut write, mut read) = ws_stream.split();

                // Subscribe to orderbook data for all symbols
                let mut args = Vec::new();
                for symbol in &symbols {
                    args.push(json!({
                        "channel": "books",
                        "instId": symbol
                    }));
                }

                let subscribe_msg = json!({
                    "op": "subscribe",
                    "args": args
                });

                if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                    error!("OKX_FUTURES {}: Failed to subscribe: {}", connection_id, e);
                }
                
                // Process incoming messages
                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                process_okx_futures_message(connection_id, &app_state, &data).await;
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            let _ = write.send(Message::Pong(payload)).await;
                        }
                        Err(e) => {
                            error!("OKX_FUTURES {}: WebSocket error: {}", connection_id, e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("OKX_FUTURES {}: Connection failed: {}", connection_id, e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Process incoming messages from OKX Futures WebSocket
async fn process_okx_futures_message(
    connection_id: usize,
    app_state: &AppState,
    data: &Value,
) {
    // Handle subscription confirmation
    if let Some(event) = data.get("event").and_then(|e| e.as_str()) {
        if event == "subscribe" {
            debug!("OKX_FUTURES {}: Subscription confirmed", connection_id);
        } else if event == "error" {
            if let Some(msg) = data.get("msg") {
                error!("OKX_FUTURES {}: Error: {}", connection_id, msg);
            }
        }
        return;
    }
    
    // Handle orderbook data
    if let Some(data_array) = data.get("data").and_then(|d| d.as_array()) {
        for item in data_array {
            if let Some(inst_id) = item.get("instId").and_then(|i| i.as_str()) {
                process_okx_depth_update(connection_id, app_state, item, inst_id).await;
            }
        }
    }
}

/// Process depth update from OKX Futures
async fn process_okx_depth_update(
    connection_id: usize,
    app_state: &AppState,
    data: &Value,
    symbol: &str,
) {
    let prefixed_symbol = ensure_exchange_prefix(symbol, "OKX_FUTURES");

    if let (Some(bids), Some(asks)) = (
        data.get("bids").and_then(|b| b.as_array()),
        data.get("asks").and_then(|a| a.as_array())
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
                    error!("OKX_FUTURES {}: Failed to send update: {}", connection_id, e);
                }
            }
        }
    }
}

fn parse_depth_levels(levels: &Vec<Value>) -> Vec<(f64, f64)> {
    levels.iter()
        .filter_map(|level| {
            if let Some(arr) = level.as_array() {
                if arr.len() >= 4 {
                    let price = arr[0].as_str()?.parse::<f64>().ok()?;
                    let quantity = arr[1].as_str()?.parse::<f64>().ok()?;
                    Some((price, quantity))
                } else { None }
            } else { None }
        })
        .collect()
}
