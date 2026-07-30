//! CLI Integration Test Crate
//!
//! Exercises every vault CLI command through the real WebSocket JSON-RPC
//! interface with all 4 plugins registered (BTC, EVM, XMR, LTC).

pub mod client;
pub mod server;