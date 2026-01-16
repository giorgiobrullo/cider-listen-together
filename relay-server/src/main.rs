//! Cider Listen Together - Dedicated Relay Server
//!
//! A libp2p relay server with a terminal dashboard.
//!
//! Usage:
//!   cargo run --release
//!   cargo run --release -- --no-dashboard  # Plain logging mode

mod dashboard;
mod metrics;
mod network;

use std::sync::Arc;
use parking_lot::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let use_dashboard = !args.contains(&"--no-dashboard".to_string());

    // Shared metrics state
    let metrics = Arc::new(RwLock::new(metrics::Metrics::new()));

    if use_dashboard {
        // Run with TUI dashboard
        dashboard::run(metrics).await
    } else {
        // Run with plain logging
        network::run_with_logging(metrics).await
    }
}
