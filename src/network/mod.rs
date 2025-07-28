// network/mod.rs - Main module declaration file
// Declare submodules
pub mod api_client;
pub mod websocket;
pub mod message_processor;
pub mod connection_manager;
pub mod lbank_websocket;
pub mod xtcom_websocket;
pub mod tapbit_websocket;
pub mod hbit_websocket;
pub mod batonex_websocket;
pub mod coincatch_websocket;
// Futures WebSocket handlers
pub mod binance_futures_websocket;
pub mod bybit_futures_websocket;
pub mod okx_futures_websocket;