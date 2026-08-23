//! Probe a WebSocket endpoint and print every frame verbatim.
//!
//! Written to establish the live market-data wire format, which the library's
//! own `stream_market_data` example cannot reach: `from_gateway_base_url`
//! derives `wss://<host>/ws`, and that path 404s against the live venue.
//!
//! ```sh
//! cargo run --example probe_stream_endpoint -- \
//!     wss://api.polymarket.us/v1/ws/markets market_data <symbol>
//! ```
//!
//! Credentials are read from the environment when present
//! (`POLYMARKET_US_KEY_ID` / `POLYMARKET_US_SECRET_KEY`); the endpoint under
//! test rejects an unauthenticated upgrade with 401.

use polymarket_us::{
    PolymarketUsError, PolymarketUsStreamClient, StreamConnectConfig, StreamSubscription, UsAuth,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    let mut args = std::env::args().skip(1);
    let url = args.next().unwrap_or_else(|| {
        "wss://api.polymarket.us/v1/ws/markets".to_string()
    });
    let channel = args.next().unwrap_or_else(|| "market_data".to_string());
    let symbol = args.next().unwrap_or_default();

    // Auth is optional so the same probe can test whether a channel is public.
    let auth = UsAuth::from_env().ok();
    println!("url     : {url}");
    println!("channel : {channel}");
    println!("symbol  : {symbol}");
    println!("auth    : {}", if auth.is_some() { "yes" } else { "no" });

    let stream = PolymarketUsStreamClient::new(url, auth);

    let mut sub = StreamSubscription::new(&channel);
    if !symbol.is_empty() {
        sub.symbol = Some(symbol);
    }

    let config = StreamConnectConfig::default()
        .with_idle_timeout(Some(Duration::from_secs(20)));

    let mut managed = stream.connect_with_config(vec![sub], config).await?;

    let mut seen = 0;
    while let Some(message) = managed.next().await {
        println!("{:?}", message.kind);
        seen += 1;
        if seen >= 12 {
            break;
        }
    }
    println!("--- {seen} frame(s) ---");
    Ok(())
}
