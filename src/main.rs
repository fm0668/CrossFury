// main.rs - Entry point for cross-exchange arbitrage system with configuration support 
// and enhanced error handling

use env_logger::Env;
use log::{error, info, debug, LevelFilter};
use std::collections::HashSet;
use std::time::Duration;
use std::io::Write;

use trifury::cross_exchange::{
    buffer_cross_exchange_opportunity, 
    buffer_multi_hop_opportunity,
    find_multi_hop_arbitrage_opportunities
};
use trifury::{
    get_normalized_cross_exchange_symbols, PriceData, AppError
};
use trifury::process_mapped_cross_exchange_arbitrage_subset;
use trifury::utils::ensure_exchange_prefix;
use trifury::json_parser::init_simd_json;
// Import required components from our crate
use trifury::{
    AppState, OrderbookUpdate,
    run_clean_metrics_display,
    flush_cross_ex_buffer,
};
// 注意：原 network 模块已移除，期货 WebSocket 处理器将在重构完成后提供
// use trifury::connectors::binance::futures::BinanceFuturesConnector;
// use trifury::connectors::bybit::futures::BybitFuturesConnector;
// use trifury::connectors::okx::futures::OkxFuturesConnector;
use trifury::config::{Config, init_config, get_config};
use trifury::error_handling::{init_error_tracker, record_error};


/// Build exchange fees map from configuration
fn build_exchange_fees_from_config() -> std::collections::HashMap<trifury::exchange_types::Exchange, trifury::exchange_types::ExchangeFees> {
    let config = get_config();
    let mut fees = std::collections::HashMap::new();
    
    // For each exchange in the config, create appropriate fee structure
    for (exchange_name, exchange_config) in &config.exchanges {
        // Parse exchange from name
        if let Ok(exchange) = exchange_name.parse::<trifury::exchange_types::Exchange>() {
            fees.insert(
                exchange,
                trifury::exchange_types::ExchangeFees::new(
                    exchange,
                    exchange_config.maker_fee_pct / 100.0,
                    exchange_config.taker_fee_pct / 100.0
                )
            );
        }
    }
    
    // Ensure we have fallbacks for any missing exchanges
    fees.entry(trifury::exchange_types::Exchange::Phemex).or_insert_with(|| trifury::exchange_types::ExchangeFees::new(
                trifury::exchange_types::Exchange::Phemex,
                0.001, // 0.1% maker
                0.0006 // 0.06% taker
            ));
    
    // Add other exchanges as needed...
    
    fees
}

/// Simple function to distribute symbols across connections
fn simple_distribute_symbols(symbols: Vec<String>, max_connections: usize) -> Vec<Vec<String>> {
    if symbols.is_empty() || max_connections == 0 {
        return vec![];
    }

    let chunk_size = symbols.len().div_ceil(max_connections); // Ceiling division
    symbols.chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Load configuration from file
    match init_config("config.toml").await {
        Ok(_) => info!("Configuration loaded successfully"),
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            eprintln!("Falling back to default configuration");
            
            // Create a basic default config
            let config = Config::default();
            Config::global().set(config).expect("Failed to set global config");
        }
    }
    
    // Initialize the error tracker
    init_error_tracker();
    
    // Initialize SIMD JSON if enabled in config
    if get_config().features.enable_simd_json {
        init_simd_json();
    }
    
    // Configure logging based on configuration
    env_logger::Builder::from_env(Env::default().default_filter_or(&get_config().general.log_level))
        // Add specific modules you want at info level
        .filter_module("trifury::terminal_log", LevelFilter::Info)
        // Reduce logging levels for hot path modules
        .filter_module("trifury::message_processor", LevelFilter::Warn)
        .filter_module("trifury::xtcom_websocket", LevelFilter::Warn)
        .filter_module("trifury::lbank_websocket", LevelFilter::Warn)
        .filter_module("trifury::websocket", LevelFilter::Warn)
        // 注意：原 network 模块日志过滤器已移除
        // .filter_module("trifury::connectors::tapbit", LevelFilter::Warn)
        // .filter_module("trifury::connectors::hbit", LevelFilter::Warn)
        // .filter_module("trifury::connectors::batonex", LevelFilter::Warn)
        // .filter_module("trifury::connectors::coincatch", LevelFilter::Warn)
        .format(|buf, record| {
            // Use a more minimal format for regular logs to prevent terminal spam
            if record.level() <= log::Level::Info {
                writeln!(
                    buf,
                    "[{}] {}",
                    record.level(),
                    record.args()
                )
            } else {
                writeln!(
                    buf,
                    "[{} {}:{}] {}",
                    record.level(),
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.args()
                )
            }
        })
        .filter_module("tokio_tungstenite", LevelFilter::Warn)
        .filter_module("tungstenite", LevelFilter::Warn)
        .filter_module("tokio", LevelFilter::Warn)
        .filter_module("tracing", LevelFilter::Warn)
        .filter_module("reqwest", LevelFilter::Warn)
        .format_timestamp_millis()
        .format_module_path(false) // Disable module path for less overhead
        .init();

    info!("Starting TriFury Cross-Exchange Arbitrage Scanner");

    // Initialize the shared application state
    let mut app_state = AppState::new();

    // Set up message queue for orderbook updates
    let (orderbook_tx, orderbook_rx) = tokio::sync::mpsc::unbounded_channel::<OrderbookUpdate>();
    app_state.orderbook_queue = Some(orderbook_tx);
    
    // Get the number of cores for optimal distribution
    let num_cores = num_cpus::get();
    let num_worker_threads = num_cores.min(get_config().general.worker_threads); 
    
    info!("Starting {num_worker_threads} parallel orderbook processor workers");
    
    // Implement a fast path for single worker or multiple workers based on core count
    if num_worker_threads == 1 {
        // For single worker case, skip the broadcast channel entirely
        let orderbook_state = app_state.clone();
        
        tokio::spawn(async move {
            info!("Starting direct orderbook processor (single worker mode)");
            
            let mut rx = orderbook_rx;
            let mut processed_count = 0;
            let mut batch = Vec::with_capacity(1000); // Increased batch size for better throughput
            
            loop {
                match rx.recv().await {
                    Some(update) => {
                        // Add to batch
                        batch.push(update);
                        
                        // Process batch when it reaches sufficient size or when no more messages
                        if batch.len() >= 1000 || rx.try_recv().is_err() {
                            for update in &batch {
                                // Process each update
                                let symbol = &update.symbol;
                                let prefixed_symbol = if !symbol.contains(':') {
                                    let default_exchange = "UNKNOWN".to_string();
                                    ensure_exchange_prefix(symbol, &default_exchange)
                                } else {
                                    symbol.clone()
                                };
                                
                                // Update price data
                                orderbook_state.price_data.insert(
                                    prefixed_symbol,
                                    PriceData {
                                        best_ask: update.best_ask,
                                        best_bid: update.best_bid,
                                        timestamp: update.timestamp,
                                        scale: update.scale,
                                        is_synthetic: update.is_synthetic,
                                        leg1: update.leg1.clone(),
                                        leg2: update.leg2.clone(),
                                        depth_asks: update.depth_asks.clone(),
                                        depth_bids: update.depth_bids.clone(),
                                    },
                                );
                            }
                            
                            // Increment the price update counter once for the whole batch
                            orderbook_state.increment_price_updates(batch.len() as u64);
                            processed_count += batch.len();
                            
                            // Clear batch but keep capacity
                            batch.clear();
                            
                            // Yield less frequently
                            if processed_count % 10000 == 0 {
                                debug!("Processed {processed_count} orderbook updates (single worker mode)");
                                tokio::task::yield_now().await;
                            }
                        }
                    },
                    None => {
                        error!("Orderbook processor channel closed");
                        break;
                    }
                }
            }
        });
    } else {
        // For multi-worker case, use broadcast channel with increased capacity
        let (worker_tx, _) = tokio::sync::broadcast::channel::<OrderbookUpdate>(20000); // Increased to 20K

        // Spawn a task to forward messages from the unbounded channel to the broadcast channel
        let worker_tx_clone = worker_tx.clone();
        tokio::spawn(async move {
            let mut rx = orderbook_rx;
            while let Some(update) = rx.recv().await {
                if let Err(e) = worker_tx_clone.send(update) {
                    error!("Worker channel closed, stopping forwarder: {e}");
                    break;
                }
            }
            info!("Orderbook forwarding task ended");
        });

        // Spawn multiple worker tasks to process orderbook updates
        for worker_id in 0..num_worker_threads {
            let orderbook_state = app_state.clone();
            let mut worker_rx = worker_tx.subscribe();  // Each worker gets a new subscription
            
            tokio::spawn(async move {
                info!("Starting orderbook processor worker {worker_id}");
                
                let mut processed_count = 0;
                let mut batch = Vec::with_capacity(1000); // Increased batch size
                
                while let Ok(update) = worker_rx.recv().await {
                    // Add to batch
                    batch.push(update);
                    
                    // Process batch when it reaches sufficient size or when queue is empty
                    if batch.len() >= 1000 || worker_rx.is_empty() {
                        for update in &batch {
                            // Ensure the symbol has an exchange prefix
                            let symbol = &update.symbol;
                            let prefixed_symbol = if !symbol.contains(':') {
                                let default_exchange = "UNKNOWN".to_string();
                                ensure_exchange_prefix(symbol, &default_exchange)
                            } else {
                                symbol.clone()
                            };
                            
                            // Update price data in a standardized format with exchange prefix
                            orderbook_state.price_data.insert(
                                prefixed_symbol,
                                PriceData {
                                    best_ask: update.best_ask,
                                    best_bid: update.best_bid,
                                    timestamp: update.timestamp,
                                    scale: update.scale,
                                    is_synthetic: update.is_synthetic,
                                    leg1: update.leg1.clone(),
                                    leg2: update.leg2.clone(),
                                    depth_asks: update.depth_asks.clone(),
                                    depth_bids: update.depth_bids.clone(),
                                },
                            );
                        }
                        
                        // Increment the price update counter once for the whole batch
                        orderbook_state.increment_price_updates(batch.len() as u64);
                        processed_count += batch.len();
                        
                        // Clear batch but keep capacity
                        batch.clear();
                        
                        // Yield less frequently
                        if processed_count % 20000 == 0 {
                            debug!("Worker {worker_id}: processed {processed_count} updates");
                            tokio::task::yield_now().await;
                        }
                    }
                }
                
                error!("Orderbook processor worker {worker_id} channel closed unexpectedly");
            });
        }
    }

    // Build exchange fees table
    let exchange_fees = build_exchange_fees_from_config();

    // 注意：产品数据获取已移除，等待重构完成
    /*
    // Get perpetual products
    let perpetual_products = match get_perpetual_products(&app_state).await {
        Ok(products) => products,
        Err(e) => {
            error!("Failed to get perpetual product data: {e}");
            record_error(trifury::exchange_types::Exchange::Phemex, None, &e);
            return Err(e);
        }
    };
    
    if perpetual_products.is_empty() {
        error!("Failed to get perpetual product data. Check API endpoint and connection.");
        return Err(AppError::Other("Failed to get perpetual product data".to_string()));
    }

    // Generate symbols lists for each exchange
    let mut phemex_symbols = Vec::new();
    let mut lbank_symbols = Vec::new();
    let mut xtcom_symbols = Vec::new();
    let mut tapbit_symbols = Vec::new();
    let mut hbit_symbols = Vec::new();
    let mut batonex_symbols = Vec::new();
    let mut coincatch_symbols = Vec::new();
    
    // Extract symbols from perpetual products for each exchange
    for symbol in perpetual_products.keys() {
        // Add to Phemex symbols (original exchange)
        phemex_symbols.push((symbol.clone(), "perpetual".to_string()));
        
        // For other exchanges, we'll use same symbol list but with exchange prefix
        lbank_symbols.push(symbol.clone());
        xtcom_symbols.push(symbol.clone());
        tapbit_symbols.push(symbol.clone());
        hbit_symbols.push(symbol.clone());
        batonex_symbols.push(symbol.clone());
        coincatch_symbols.push(symbol.clone());
    }

    // Log how many products we're tracking
    info!("Tracking {} perpetual products on Phemex", phemex_symbols.len());
    info!("Tracking {} perpetual products on lbank", lbank_symbols.len());
    info!("Tracking {} perpetual products on XT.COM", xtcom_symbols.len());
    info!("Tracking {} perpetual products on TapBit", tapbit_symbols.len());
    info!("Tracking {} perpetual products on Hbit", hbit_symbols.len());
    info!("Tracking {} perpetual products on Batonex", batonex_symbols.len());
    info!("Tracking {} perpetual products on CoinCatch", coincatch_symbols.len());
          
    // Get max connections from config
    let max_ws_connections = get_config().connection.max_ws_connections;
    
    // Distribute original Phemex symbols across more connections
    let phemex_symbol_chunks = distribute_symbols(phemex_symbols, (max_ws_connections * 3) / 10).await;
    
    // Distribute other exchanges' symbols with fewer symbols per connection
    let lbank_chunks = distribute_even_chunks(&lbank_symbols, max_ws_connections / 10, 10);  // Max 10 per connection
    let xtcom_chunks = distribute_even_chunks(&xtcom_symbols, max_ws_connections / 10, 10);  // Max 10 per connection
    let tapbit_chunks = distribute_even_chunks(&tapbit_symbols, max_ws_connections / 10, 10); // Max 10 per connection
    let hbit_chunks = distribute_even_chunks(&hbit_symbols, max_ws_connections / 10, 10); // Max 10 per connection
    let batonex_chunks = distribute_even_chunks(&batonex_symbols, max_ws_connections / 10, 10); // Max 10 per connection
    let coincatch_chunks = distribute_even_chunks(&coincatch_symbols, max_ws_connections / 10, 10); // Max 10 per connection
    */
    
    info!("WebSocket连接器已暂时禁用，等待重构完成");

    // Mark initialization as complete
    *app_state.is_initializing.write().await = false;

    // Optimize the Tokio Runtime for the system's available resources
    let websocket_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(get_config().general.worker_threads)
        .thread_name("wss-worker")
        .thread_stack_size(2 * 1024 * 1024)  // 2MB stack size
        .enable_all()
        .build()
        .unwrap();

    // Create a dedicated scanner runtime for parallel processing
    let scanner_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(get_config().general.scanner_threads)
        .thread_name("scanner-worker")
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()
        .unwrap();

    // Track WebSocket handles
    let websocket_runtime_handle = websocket_runtime.handle().clone();
    let scanner_handle = scanner_runtime.handle().clone();
    let mut websocket_tasks = Vec::new();
    let mut scanner_tasks = Vec::new();

    // Launch Phemex WebSocket connections
    // 注意：WebSocket处理器已移除，等待重构完成
    /*
    info!("Starting Phemex WebSocket connections");
    for (i, chunk) in phemex_symbol_chunks.into_iter().enumerate() {
        let state_clone = app_state.clone();
        
        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = websocket_handler(chunk, i, state_clone).await {
                error!("Phemex WebSocket handler conn-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::Phemex, Some(&format!("conn-{}", i + 1)), &e);
            }
        });
        
        websocket_tasks.push(task);
    }
    */
    
    // 注意：所有WebSocket处理器已移除，等待重构完成
    /*
    // Launch LBANK WebSocket connections
    info!("Starting LBANK WebSocket connections");
    for (i, chunk) in lbank_chunks.into_iter().enumerate() {
        let state_clone = app_state.clone();
        
        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = lbank_websocket_handler(chunk, i, state_clone).await {
                error!("LBank WebSocket handler lbank-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::LBank, Some(&format!("lbank-{}", i + 1)), &e);
            }
        });
        
        websocket_tasks.push(task);
    }
    
    // Launch XT.COM WebSocket connections
    info!("Starting XT.COM WebSocket connections");
    for (i, chunk) in xtcom_chunks.into_iter().enumerate() {
        let state_clone = app_state.clone();
        
        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = xtcom_websocket_handler(chunk, i, state_clone).await {
                error!("XT.COM WebSocket handler xtcom-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::XtCom, Some(&format!("xtcom-{}", i + 1)), &e);
            }
        });
        
        websocket_tasks.push(task);
    }

    // Launch TapBit WebSocket connections 
    info!("Starting TapBit WebSocket connections");  
    for (i, chunk) in tapbit_chunks.into_iter().enumerate() {  
        let state_clone = app_state.clone();
        
        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = tapbit_websocket_handler(chunk, i, state_clone).await {
                error!("TapBit WebSocket handler tapbit-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::TapBit, Some(&format!("tapbit-{}", i + 1)), &e);
            }
        });
        
        websocket_tasks.push(task);
    }

    // Launch Hbit WebSocket connections
    info!("Starting Hbit WebSocket connections");
    for (i, chunk) in hbit_chunks.into_iter().enumerate() {
        let state_clone = app_state.clone();
        
        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = hbit_websocket_handler(chunk, i, state_clone).await {
                error!("Hbit WebSocket handler hbit-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::Hbit, Some(&format!("hbit-{}", i + 1)), &e);
            }
        });
        
        websocket_tasks.push(task);
    }

    // Launch Batonex WebSocket connections
    info!("Starting Batonex WebSocket connections");
    for (i, chunk) in batonex_chunks.into_iter().enumerate() {
        let state_clone = app_state.clone();
        
        // Continuing from Batonex WebSocket connections setup...
        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = batonex_websocket_handler(chunk, i, state_clone).await {
                error!("Batonex WebSocket handler batonex-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::Batonex, Some(&format!("batonex-{}", i + 1)), &e);
            }
        });
        
        websocket_tasks.push(task);
    }

    // Launch Coin Catch WebSocket connections
    info!("Starting Coin Catch WebSocket connections");
    for (i, chunk) in coincatch_chunks.into_iter().enumerate() {
        let state_clone = app_state.clone();

        let task = websocket_runtime_handle.spawn(async move {
            if let Err(e) = coincatch_websocket_handler(chunk, i, state_clone).await {
                error!("Coin Catch WebSocket handler coincatch-{} error: {}", i + 1, e);
                record_error(trifury::exchange_types::Exchange::CoinCatch, Some(&format!("coincatch-{}", i + 1)), &e);
            }
        });

        websocket_tasks.push(task);
    }
    */

    // 注意：期货 WebSocket 连接已暂时禁用，等待重构完成
    // TODO: 使用重构后的连接器系统重新实现期货连接
    /*
    // Start futures WebSocket connections
    info!("Starting futures WebSocket connections...");

    // Get futures symbols from exchanges
    let futures_symbols = match get_futures_symbols().await {
        Ok(symbols) => symbols,
        Err(e) => {
            error!("Failed to get futures symbols: {e}");
            std::collections::HashMap::new()
        }
    };

    // Launch Binance Futures WebSocket connections
    if let Some(binance_symbols) = futures_symbols.get("BINANCE_FUTURES") {
        let symbols_chunks = simple_distribute_symbols(binance_symbols.clone(), 3);
        for (i, chunk) in symbols_chunks.into_iter().enumerate() {
            let app_state_clone = app_state.clone();
            let task = websocket_runtime_handle.spawn(async move {
                if let Err(e) = binance_futures_websocket_handler(chunk, i + 1, app_state_clone).await {
                    error!("BINANCE_FUTURES connection {} failed: {}", i + 1, e);
                }
            });
            websocket_tasks.push(task);
        }
    }

    // Launch Bybit Futures WebSocket connections
    if let Some(bybit_symbols) = futures_symbols.get("BYBIT_FUTURES") {
        let symbols_chunks = simple_distribute_symbols(bybit_symbols.clone(), 3);
        for (i, chunk) in symbols_chunks.into_iter().enumerate() {
            let app_state_clone = app_state.clone();
            let task = websocket_runtime_handle.spawn(async move {
                if let Err(e) = bybit_futures_websocket_handler(chunk, i + 1, app_state_clone).await {
                    error!("BYBIT_FUTURES connection {} failed: {}", i + 1, e);
                }
            });
            websocket_tasks.push(task);
        }
    }

    // Launch OKX Futures WebSocket connections
    if let Some(okx_symbols) = futures_symbols.get("OKX_FUTURES") {
        let symbols_chunks = simple_distribute_symbols(okx_symbols.clone(), 3);
        for (i, chunk) in symbols_chunks.into_iter().enumerate() {
            let app_state_clone = app_state.clone();
            let task = websocket_runtime_handle.spawn(async move {
                if let Err(e) = okx_futures_websocket_handler(chunk, i + 1, app_state_clone).await {
                    error!("OKX_FUTURES connection {} failed: {}", i + 1, e);
                }
            });
            websocket_tasks.push(task);
        }
    }
    */
        
    // Start a connection health manager
    // 注意：连接健康管理器已移除，等待重构完成
    /*
    let health_state = app_state.clone();
    let health_task = websocket_runtime_handle.spawn(async move {
        run_connection_health_manager(health_state).await;
    });
    websocket_tasks.push(health_task);
    */

    // Allow time for connections to initialize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start cross-exchange arbitrage scanning task
    let cross_exchange_state = app_state.clone();
    let exchange_fees_clone = exchange_fees.clone();

    info!("STARTING CROSS-EXCHANGE ARBITRAGE SCANNER");

    // Initialize cross-exchange counter
    app_state.increment_cross_exchange_checks(0);

    // Add a CSV flush task for cross-exchange opportunities
    let flush_interval_secs = get_config().general.csv_flush_interval_secs;
    let flush_task = tokio::spawn(async move {
        info!("Starting arbitrage opportunity flush task");
        let mut interval = tokio::time::interval(Duration::from_secs(flush_interval_secs));
        
        loop {
            interval.tick().await;
            
            // Flush standard cross-exchange opportunities
            if let Err(e) = flush_cross_ex_buffer("cross_exchange_arb.csv").await {
                error!("Error flushing cross-exchange buffer: {e}");
            }
            
            // Flush multi-hop opportunities if feature is enabled
            if get_config().features.enable_multi_hop_arbitrage {
                // Multi-hop buffer flushing disabled for now
                // if let Err(e) = flush_multi_hop_buffer("multi_hop_arb.csv").await {
                //     error!("Error flushing multi-hop buffer: {}", e);
                // }
            }
            
            tokio::task::yield_now().await;
        }
    });
    websocket_tasks.push(flush_task);

    // Launch multiple scanner tasks for parallel processing
    let total_scanners = get_config().general.scanner_threads;  // Get from config

    for i in 0..total_scanners {
        let state_clone = cross_exchange_state.clone();
        let fees_clone = exchange_fees_clone.clone();
        
        let scanner_task = scanner_handle.spawn(async move {
            // Scanner-specific configuration
            let mut interval = tokio::time::interval(Duration::from_micros(1));
            let mut scan_counter = 0;
            let mut last_symbol_update = std::time::Instant::now();
            let mut cached_symbols = HashSet::with_capacity(1000);
            
            // Allow time for connections to initialize
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            loop {
                interval.tick().await;
                scan_counter += 1;
                
                // Refresh symbol cache periodically
                if cached_symbols.is_empty() || last_symbol_update.elapsed() > Duration::from_millis(500) {
                    // Get all symbols that exist on multiple exchanges
                    let all_symbols = get_normalized_cross_exchange_symbols(&state_clone);
                    
                    // Convert to a vector to make it easier to split
                    let all_symbols_vec: Vec<String> = all_symbols.into_iter().collect();
                    
                    // Each scanner takes a different subset
                    if !all_symbols_vec.is_empty() {
                        let chunk_size = all_symbols_vec.len().div_ceil(total_scanners);
                        let start = i * chunk_size;
                        let end = std::cmp::min(start + chunk_size, all_symbols_vec.len());
                        
                        if start < all_symbols_vec.len() {
                            cached_symbols = all_symbols_vec[start..end].iter().cloned().collect();
                        }
                    }
                    
                    last_symbol_update = std::time::Instant::now();
                    
                    // Always increment the scan counter
                    state_clone.increment_cross_exchange_checks(1);
                }
                
                // Skip if no symbols to process
                if cached_symbols.is_empty() {
                    continue;
                }
                
                // Process our subset of symbols
                let opportunities = process_mapped_cross_exchange_arbitrage_subset(
                    &state_clone,
                    &fees_clone,
                    &cached_symbols
                ).await;
                
                // Handle profitable opportunities
                if !opportunities.is_empty() {
                    // Only count truly profitable opportunities (above threshold)
                    let min_profit = get_config().arbitrage.min_profit_threshold_pct;
                    let profitable_count = opportunities.iter()
                        .filter(|opp| opp.net_profit_pct >= min_profit)
                        .count();
                    
                    // Increment the profitable opportunities counter
                    if profitable_count > 0 {
                        state_clone.increment_profitable_opportunities(profitable_count as u64);
                        
                        // Only log if we found significant opportunities
                        info!("Scanner {i}: Found {profitable_count} cross-exchange opportunities above {min_profit:.2}% threshold");
                    }
                    
                    // Log top 3 opportunities
                    for (j, opportunity) in opportunities.iter()
                        .filter(|opp| opp.net_profit_pct >= min_profit)
                        .take(3).enumerate() 
                    {
                        info!(
                            "  Scanner {} - #{}: {} from {} (${:.2}) -> {} (${:.2}): +{:.4}% (net: {:.4}%)",
                            i,
                            j + 1,
                            opportunity.symbol,
                            opportunity.buy_exchange,
                            opportunity.buy_price,
                            opportunity.sell_exchange,
                            opportunity.sell_price,
                            opportunity.profit_pct,
                            opportunity.net_profit_pct
                        );
                        
                        // Buffer the opportunity for CSV logging
                        let _ = buffer_cross_exchange_opportunity(opportunity.clone()).await;
                    }
                }
                
                // Process multi-hop arbitrage if enabled
                if get_config().features.enable_multi_hop_arbitrage && scan_counter % 1000 == 0 {
                    let multi_hop_opps = find_multi_hop_arbitrage_opportunities(
                        &state_clone, 
                        &fees_clone
                    );
                    
                    // Handle profitable multi-hop opportunities
                    if !multi_hop_opps.is_empty() {
                        let min_profit = get_config().arbitrage.min_profit_threshold_pct;
                        let profitable_count = multi_hop_opps.iter()
                            .filter(|opp| opp.net_profit_pct >= min_profit)
                            .count();
                        
                        if profitable_count > 0 {
                            info!("Scanner {i}: Found {profitable_count} multi-hop arbitrage opportunities above {min_profit:.2}% threshold");
                            
                            // Log top 3 multi-hop opportunities
                            for (j, opportunity) in multi_hop_opps.iter()
                                .filter(|opp| opp.net_profit_pct >= min_profit)
                                .take(3).enumerate()
                            {
                                let path_str = opportunity.symbol_path.join("→");
                                
                                info!(
                                    "  Scanner {} - Multi-hop #{}: Path {} with {} hops: +{:.4}% (net: {:.4}%)",
                                    i, j + 1, path_str, opportunity.symbol_path.len() - 1,
                                    opportunity.total_profit_pct, opportunity.net_profit_pct
                                );
                                
                                // Buffer the opportunity for CSV logging
                                let _ = buffer_multi_hop_opportunity(opportunity.clone()).await;
                            }
                        }
                    }
                }
                
                // Yield less frequently for better performance
                if scan_counter % 100000 == 0 {
                    tokio::task::yield_now().await;
                }
            }
        });
        
        scanner_tasks.push(scanner_task);
    }

    // Start clean metrics display
    let metrics_state = app_state.clone();
    let metrics_display = tokio::spawn(async move {
        info!("Starting metrics display");
        run_clean_metrics_display(metrics_state).await;
    });

    // Wait for metrics display to finish
    if let Err(e) = metrics_display.await {
        error!("Error in metrics display thread: {e}");
    }

    // Cleanly shut down scanner runtime
    info!("Shutting down scanner runtime...");
    for task in scanner_tasks {
        task.abort();
    }
    scanner_runtime.shutdown_timeout(Duration::from_secs(5));

    // Cleanly shut down WebSocket runtime
    info!("Shutting down WebSocket runtime...");
    for task in websocket_tasks {
        task.abort();
    }
    websocket_runtime.shutdown_timeout(Duration::from_secs(5));

    info!("Application shutting down.");
    Ok(())
    }

    /// Helper function to evenly distribute symbols across connections with maximum per chunk
    fn distribute_even_chunks(symbols: &[String], num_chunks: usize, max_per_chunk: usize) -> Vec<Vec<String>> {
        if symbols.is_empty() || num_chunks == 0 {
            return Vec::new();
        }
        
        // Calculate optimal chunk size with maximum limit
        let symbols_per_chunk = symbols.len().div_ceil(num_chunks).min(max_per_chunk);
        
        // If we need more chunks due to max_per_chunk, recalculate
        let actual_chunks_needed = symbols.len().div_ceil(symbols_per_chunk);
        
        let mut result = Vec::new();
        
        for chunk_idx in 0..actual_chunks_needed {
            let start = chunk_idx * symbols_per_chunk;
            let end = std::cmp::min(start + symbols_per_chunk, symbols.len());
            
            if start < symbols.len() {
                let chunk = symbols[start..end].to_vec();
                result.push(chunk);
            }
        }
        
        result
    }