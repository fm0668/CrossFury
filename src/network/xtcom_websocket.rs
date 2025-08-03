// xtcom_websocket.rs - XT.COM WebSocket handler with correct protocol implementation

use crate::core::*;
use crate::utils::ensure_exchange_prefix;
use futures::{SinkExt, StreamExt};
use log::{info, error, warn, debug};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Error as WsError;

// Correct WebSocket URL for XT.COM
const XTCOM_WS_URL: &str = "wss://fstream.xt.com/ws/market";

pub async fn xtcom_websocket_handler(
    symbols: Vec<String>,
    connection_index: usize,
    app_state: AppState,
) -> Result<(), AppError> {
    let connection_id = format!("xtcom-{}", connection_index + 1);
    app_state.update_connection_timestamp(&connection_id);
    app_state.clear_reconnect_signal(&connection_id);

    let mut retry_count = 0;
    let max_retries = 15; 

    while retry_count < max_retries {
        info!("XT.COM {connection_id}: Connecting to {XTCOM_WS_URL}");
        
        // Create request with required headers
        let mut request = XTCOM_WS_URL.into_client_request()
            .map_err(|e| AppError::WebSocketError(format!("Failed to create request: {e}")))?;
        
        // Add required permessage-deflate header for XT.COM
        request.headers_mut().insert(
            "Sec-Websocket-Extensions", 
            "permessage-deflate".parse().unwrap()
        );
        request.headers_mut().insert(
            "User-Agent", 
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".parse().unwrap()
        );

        // Connect with timeout
        match timeout(Duration::from_secs(15), connect_async(request)).await {
            Ok(Ok((ws_stream, _))) => {
                info!("XT.COM {connection_id}: Connection established");
                app_state.update_connection_timestamp(&connection_id);
                let (write, mut read) = ws_stream.split();
                let write = Arc::new(Mutex::new(write));
                let write_clone = write.clone();

                // Start a ping task with CORRECT plain text ping format for XT.COM
                let ping_connection_id = connection_id.clone();
                let ping_app_state = app_state.clone();
                let ping_task = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(10)); // Every 10 seconds
                    
                    // Wait a bit before starting pings to allow connection to stabilize
                    sleep(Duration::from_secs(1)).await;
                    
                    loop {
                        interval.tick().await;
                        
                        if ping_app_state.should_reconnect(&ping_connection_id) {
                            error!("XT.COM {ping_connection_id}: Reconnection signaled, terminating ping task");
                            return;
                        }
                        
                        // Send simple text "ping" as per XT.COM documentation
                        info!("XT.COM {ping_connection_id}: Sending heartbeat ping");
                        ping_app_state.update_connection_timestamp(&ping_connection_id);
                        
                        let mut writer = write_clone.lock().await;
                        // Send plain "ping" text, not JSON
                        if let Err(e) = writer.send(Message::Text("ping".to_string())).await {
                            error!("XT.COM {ping_connection_id}: Failed to send ping: {e}");
                            break;
                        }
                        
                        // Check if connection is stale
                        let idle_time = ping_app_state.get_connection_idle_time(&ping_connection_id);
                        if idle_time > STALE_CONNECTION_TIMEOUT as u64 {
                            error!("XT.COM {ping_connection_id}: Connection stale for {idle_time}ms, terminating ping task");
                            break;
                        }
                    }
                    
                    warn!("XT.COM {ping_connection_id}: Ping task terminated");
                });

                // Subscribe to symbols in batches to avoid overwhelming the server
                let mut remaining_symbols = symbols.clone();
                let mut subscription_count = 0;

                while !remaining_symbols.is_empty() {
                    // Take 5 symbols at a time
                    let batch: Vec<String> = remaining_symbols.drain(..std::cmp::min(5, remaining_symbols.len())).collect();
                    
                    // Format symbols correctly for XT.COM (lowercase with underscore)
                    let mut params = Vec::new();
                    for symbol in &batch {
                        // Convert symbol (e.g. "BTCUSDT") to XT.COM format (e.g. "btc_usdt")
                        let xtcom_symbol = if symbol.contains('_') {
                            symbol.to_lowercase()
                        } else {
                            let mut symbol_lower = symbol.to_lowercase();
                            // Extract base/quote for common quote currencies
                            let quote_currencies = ["usdt", "usd", "btc", "eth", "usdc"];
                            
                            for &quote in &quote_currencies {
                                if symbol_lower.ends_with(quote) {
                                    let base = &symbol_lower[..symbol_lower.len() - quote.len()];
                                    symbol_lower = format!("{base}_{quote}");
                                    break;
                                }
                            }
                            symbol_lower
                        };
                        
                        // CRITICAL FIX: Use exact format from documentation: depth_update@{symbol},{interval}
                        // Only use valid intervals: 100/250/500/1000ms
                        params.push(format!("depth_update@{xtcom_symbol},100ms"));
                    }
                    
                    if !params.is_empty() {
                        // Use exact subscription format from documentation
                        let sub_req = json!({
                            "method": "subscribe",
                            "params": params,
                            "id": format!("sub_{}", chrono::Utc::now().timestamp_millis())
                        });
                        
                        info!("XT.COM {}: Subscribing batch {} with {} symbols: {}", 
                            connection_id, subscription_count + 1, params.len(), sub_req);
                        
                        let mut writer = write.lock().await;
                        if let Err(e) = writer.send(Message::Text(sub_req.to_string())).await {
                            error!("XT.COM {connection_id}: Failed to send subscription: {e}");
                            break;
                        } else {
                            info!("XT.COM {connection_id}: Subscription sent successfully");
                            subscription_count += 1;
                        }
                        
                        // Wait between subscription batches to avoid overwhelming the server
                        drop(writer); // Release lock before sleeping
                        sleep(Duration::from_millis(5)).await;
                    }
                }

                // Main event loop with timeout
                let mut consecutive_timeouts = 0;
                let mut consecutive_errors = 0;
                
                loop {
                    if app_state.should_reconnect(&connection_id) {
                        error!("XT.COM {connection_id}: Reconnection signaled, breaking main loop");
                        break;
                    }
                    
                    app_state.update_connection_timestamp(&connection_id);
                    
                    match timeout(Duration::from_secs(10), read.next()).await {
                        Ok(Some(Ok(Message::Text(text)))) => {
                             // Add this line at the beginning:
                            app_state.increment_websocket_messages(1);
                            consecutive_timeouts = 0;
                            consecutive_errors = 0;
                            
                            // Handle plain text "pong" response
                            if text == "pong" {
                                debug!("XT.COM {connection_id}: Received pong response");
                                app_state.update_connection_timestamp(&connection_id);
                                continue;
                            }
                            
                            // Process data messages
                            app_state.update_connection_timestamp(&connection_id);
                            
                            // Process depth_update messages
                            if text.contains("depth_update") {
                                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if let Some(data) = json_value.get("data") {
                                        if let Some(symbol_info) = data.get("s") {
                                            let symbol_str = symbol_info.as_str().unwrap_or("unknown").to_uppercase();
                                            // Convert from format like "eth_usdt" to "ETHUSDT"
                                            let normalized_symbol = symbol_str.replace('_', "");
                                            
                                            // Ensure we use the proper prefixed symbol
                                            let prefixed_symbol = ensure_exchange_prefix(&normalized_symbol, "XTCOM");
                                            
                                            // Check if we have asks and bids data
                                            if let (Some(asks), Some(bids)) = (data.get("a"), data.get("b")) {
                                                if let (Some(asks_array), Some(bids_array)) = (asks.as_array(), bids.as_array()) {
                                                    // Get price scale (assumes already in app_state)
                                                    let scale = app_state.price_scales.get(&normalized_symbol)
                                                        .map(|entry| *entry.value())
                                                        .unwrap_or(8); // Default to 8 if not found
                                                    
                                                    // Get best bid and ask from the arrays
                                                    let best_ask = if !asks_array.is_empty() {
                                                        asks_array[0][0].as_str().unwrap_or("0")
                                                            .parse::<f64>().unwrap_or(0.0) / 10f64.powi(scale)
                                                    } else {
                                                        0.0
                                                    };
                                                    
                                                    let best_bid = if !bids_array.is_empty() {
                                                        bids_array[0][0].as_str().unwrap_or("0")
                                                            .parse::<f64>().unwrap_or(0.0) / 10f64.powi(scale)
                                                    } else {
                                                        0.0
                                                    };
                                                    
                                                    // Extract full depth data for accurate slippage calculation
                                                    let depth_asks = asks_array.iter()
                                                        .filter_map(|level| {
                                                            if let Some(level_array) = level.as_array() {
                                                                if level_array.len() >= 2 {
                                                                    let price = level_array[0].as_str().unwrap_or("0")
                                                                        .parse::<f64>().unwrap_or(0.0) / 10f64.powi(scale);
                                                                    let quantity = level_array[1].as_str().unwrap_or("0")
                                                                        .parse::<f64>().unwrap_or(0.0);
                                                                    
                                                                    if price > 0.0 && quantity > 0.0 {
                                                                        Some((price, quantity))
                                                                    } else {
                                                                        None
                                                                    }
                                                                } else {
                                                                    None
                                                                }
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect::<Vec<(f64, f64)>>();
                                                    
                                                    let depth_bids = bids_array.iter()
                                                        .filter_map(|level| {
                                                            if let Some(level_array) = level.as_array() {
                                                                if level_array.len() >= 2 {
                                                                    let price = level_array[0].as_str().unwrap_or("0")
                                                                        .parse::<f64>().unwrap_or(0.0) / 10f64.powi(scale);
                                                                    let quantity = level_array[1].as_str().unwrap_or("0")
                                                                        .parse::<f64>().unwrap_or(0.0);
                                                                    
                                                                    if price > 0.0 && quantity > 0.0 {
                                                                        Some((price, quantity))
                                                                    } else {
                                                                        None
                                                                    }
                                                                } else {
                                                                    None
                                                                }
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect::<Vec<(f64, f64)>>();
                                                    
                                                    // Skip if prices are invalid
                                                    if best_ask <= 0.0 || best_bid <= 0.0 {
                                                        debug!("XT.COM {connection_id}: Invalid prices for {symbol_str}: ask={best_ask}, bid={best_bid}");
                                                        continue;
                                                    }
                                                    
                                                    info!("XT.COM {connection_id}: Received valid orderbook update for {prefixed_symbol}: ask={best_ask}, bid={best_bid}");
                                                    
                                                    // Update price data
                                                    let current_time = chrono::Utc::now().timestamp_millis();
                                                    if let Some(tx) = &app_state.orderbook_queue {
                                                        let update = OrderbookUpdate {
                                                            symbol: prefixed_symbol.clone(),
                                                            best_ask,
                                                            best_bid,
                                                            timestamp: current_time,
                                                            scale,
                                                            is_synthetic: false,
                                                            leg1: None,
                                                            leg2: None,
                                                            depth_asks: Some(depth_asks),
                                                            depth_bids: Some(depth_bids),
                                                        };
                                                        
                                                        if let Err(e) = tx.send(update) {
                                                            error!("XT.COM {connection_id}: Failed to send orderbook update: {e}");
                                                        } else {
                                                            debug!("XT.COM {connection_id}: Enqueued price update for {prefixed_symbol}: ask={best_ask}, bid={best_bid}");
                                                        }
                                                    } else {
                                                        app_state.price_data.insert(
                                                            prefixed_symbol,
                                                            PriceData {
                                                                best_ask,
                                                                best_bid,
                                                                timestamp: current_time,
                                                                scale,
                                                                is_synthetic: false,
                                                                leg1: None,
                                                                leg2: None,
                                                                // Just add these two lines:
                                                                depth_asks: Some(depth_asks),
                                                                depth_bids: Some(depth_bids),
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Ok(Some(Ok(msg))) => {
                            // Handle other message types
                            consecutive_timeouts = 0;
                            consecutive_errors = 0;
                            app_state.update_connection_timestamp(&connection_id);
                            
                            match msg {
                                
                                Message::Binary(data) => {
                                    debug!("XT.COM {}: Received binary data ({} bytes)", connection_id, data.len());
                                },
                                Message::Ping(data) => {
                                    debug!("XT.COM {connection_id}: Received Ping, sending Pong");
                                    let mut writer = write.lock().await;
                                    if let Err(e) = writer.send(Message::Pong(data)).await {
                                        error!("XT.COM {connection_id}: Failed to send Pong: {e}");
                                    }
                                },
                                Message::Pong(_) => {
                                    debug!("XT.COM {connection_id}: Received Pong");
                                },
                                Message::Close(frame) => {
                                    info!("XT.COM {connection_id}: Received Close frame: {frame:?}");
                                    break;
                                },
                                _ => {
                                    debug!("XT.COM {connection_id}: Received other message type");
                                }
                            }
                        },
                        Ok(Some(Err(e))) => {
                            consecutive_errors += 1;
                            
                            // Different handling based on error type
                            match &e {
                                WsError::ConnectionClosed => {
                                    error!("XT.COM {connection_id}: Connection closed");
                                    break;
                                },
                                WsError::AlreadyClosed => {
                                    error!("XT.COM {connection_id}: Connection already closed");
                                    break;
                                },
                                WsError::Protocol(_) => {
                                    error!("XT.COM {connection_id}: Protocol error: {e}");
                                    if consecutive_errors >= 2 {
                                        error!("XT.COM {connection_id}: Multiple protocol errors, reconnecting");
                                        break;
                                    }
                                },
                                _ => {
                                    error!("XT.COM {connection_id}: WebSocket error: {e}");
                                    if consecutive_errors >= 3 {
                                        error!("XT.COM {connection_id}: Too many consecutive errors, reconnecting");
                                        break;
                                    }
                                }
                            }
                        },
                        Ok(None) => {
                            info!("XT.COM {connection_id}: WebSocket stream ended");
                            break;
                        },
                        Err(_) => {
                            consecutive_timeouts += 1;
                            let idle_time = app_state.get_connection_idle_time(&connection_id);
                            warn!("XT.COM {connection_id}: Read timeout - idle for {idle_time}ms");
                            
                            // Try to send a ping to keep the connection alive
                            if consecutive_timeouts == 1 {
                                // Try to send an emergency ping
                                warn!("XT.COM {connection_id}: Sending emergency ping");
                                let mut writer = write.lock().await;
                                // Use correct plain text "ping" format
                                if let Err(e) = writer.send(Message::Text("ping".to_string())).await {
                                    error!("XT.COM {connection_id}: Failed to send emergency ping: {e}");
                                }
                            }
                            
                            if consecutive_timeouts >= 2 {
                                error!("XT.COM {connection_id}: Too many consecutive timeouts, reconnecting");
                                break;
                            }
                        }
                    }
                    
                    // Check connection staleness
                    let idle_time = app_state.get_connection_idle_time(&connection_id);
                    if idle_time > FORCE_RECONNECT_TIMEOUT as u64 {
                        error!("XT.COM {connection_id}: Connection stale ({idle_time}ms), forcing reconnect");
                        break;
                    }
                    
                    // Yield occasionally to avoid monopolizing the thread
                    if subscription_count % 10 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
                
                // Clean up the ping task
                ping_task.abort();
                error!("XT.COM {connection_id}: Session ended, reconnecting...");
            },
            Ok(Err(e)) => {
                error!("XT.COM {connection_id}: Failed to connect: {e}");
                retry_count += 1;
            },
            Err(_) => {
                error!("XT.COM {connection_id}: Connection timeout");
                retry_count += 1;
            }
        }
        
        // Incremental backoff
        let delay = f64::min(0.5 * 1.5f64.powi(retry_count), MAX_RECONNECT_DELAY);
        
        info!("XT.COM {}: Reconnecting in {:.2} seconds (attempt {}/{})", 
             connection_id, delay, retry_count + 1, max_retries);
        
        app_state.update_connection_timestamp(&connection_id);
        sleep(Duration::from_secs_f64(delay)).await;
        retry_count += 1;
    }
    
    error!("XT.COM {connection_id}: Failed to maintain connection after {max_retries} retries");
    Ok(())
}