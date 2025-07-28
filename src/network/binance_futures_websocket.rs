// binance_futures_websocket.rs - Binance Futures WebSocket handler
// Handles real-time perpetual contract orderbook data from Binance Futures

use crate::{AppState, OrderbookUpdate};
use crate::utils::ensure_exchange_prefix;
use crate::config::get_config;
use log::{info, error, debug};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::time::sleep;

/// Binance Futures WebSocket handler for perpetual contracts
pub async fn binance_futures_websocket_handler(
    symbols: Vec<String>,
    connection_id: usize,
    app_state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = get_config();
    let ws_url = &config.exchanges.get("BINANCE_FUTURES").unwrap().websocket_url;
    
    info!("BINANCE_FUTURES {}: Starting connection with {} symbols", connection_id, symbols.len());
    
    loop {
        match connect_async(ws_url).await {
            Ok((ws_stream, _)) => {
                info!("BINANCE_FUTURES {}: Connected successfully", connection_id);
                let (mut write, mut read) = ws_stream.split();

                // Subscribe to depth data for each symbol
                for symbol in &symbols {
                    let stream_name = format!("{}@depth20@100ms", symbol.to_lowercase());
                    let subscribe_msg = json!({
                        "method": "SUBSCRIBE",
                        "params": [stream_name],
                        "id": connection_id
                    });

                    if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                        error!("BINANCE_FUTURES {}: Failed to subscribe {}: {}", connection_id, symbol, e);
                    }
                    sleep(Duration::from_millis(10)).await;
                }
                
                // Process incoming messages
                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                process_binance_futures_message(connection_id, &app_state, &data).await;
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            let _ = write.send(Message::Pong(payload)).await;
                        }
                        Err(e) => {
                            error!("BINANCE_FUTURES {}: WebSocket error: {}", connection_id, e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("BINANCE_FUTURES {}: Connection failed: {}", connection_id, e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Process incoming messages from Binance Futures WebSocket
async fn process_binance_futures_message(
    connection_id: usize,
    app_state: &AppState,
    data: &Value,
) {
    // Handle subscription confirmation
    if let Some(result) = data.get("result") {
        if result.is_null() {
            debug!("BINANCE_FUTURES {}: Subscription confirmed", connection_id);
        }
        return;
    }
    
    // Handle stream data
    if let Some(stream) = data.get("stream").and_then(|s| s.as_str()) {
        if stream.contains("@depth20") {
            if let Some(data_obj) = data.get("data") {
                process_binance_depth_update(connection_id, app_state, data_obj).await;
            }
        }
    }
}

/// Process depth update from Binance Futures
async fn process_binance_depth_update(
    connection_id: usize,
    app_state: &AppState,
    data: &Value,
) {
    let symbol = data.get("s").and_then(|s| s.as_str()).unwrap_or("unknown");
    let prefixed_symbol = ensure_exchange_prefix(symbol, "BINANCE_FUTURES");
    
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
                    error!("BINANCE_FUTURES {}: Failed to send update: {}", connection_id, e);
                }
            }
        }
    }
}

/// Parse depth levels from Binance Futures format
fn parse_depth_levels(levels: &Vec<Value>) -> Vec<(f64, f64)> {
    levels.iter()
        .filter_map(|level| {
            if let Some(arr) = level.as_array() {
                if arr.len() >= 2 {
                    let price = arr[0].as_str()?.parse::<f64>().ok()?;
                    let quantity = arr[1].as_str()?.parse::<f64>().ok()?;
                    Some((price, quantity))
                } else { 
                    None 
                }
            } else { 
                None 
            }
        })
        .collect()
}
