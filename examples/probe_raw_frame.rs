//! Send an arbitrary JSON frame to a WebSocket endpoint and print the replies.
//!
//! Exists to test a frame shape empirically. The typed clients cannot do this:
//! `MarketSubscription` and `PrivateSubscription` fix the frame shape, so a
//! rejected subscription gives no way to try a different one without editing
//! the library.
//!
//! ```sh
//! cargo run --example probe_raw_frame -- \
//!     wss://api.polymarket.us/v1/ws/markets \
//!     '{"subscribe":{"requestId":"md-1","subscriptionType":"SUBSCRIPTION_TYPE_MARKET_DATA","marketSlugs":["btc-100k-2025"]}}'
//! ```
//!
//! Signs the upgrade with `POLYMARKET_US_KEY_ID` / `POLYMARKET_US_SECRET_KEY`
//! when present — the market endpoint rejects an unauthenticated upgrade.

use futures_util::{SinkExt, StreamExt};
use polymarket_us::UsAuth;
use std::time::Duration;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: probe_raw_frame <wss-url> <json-frame>");
    let frame = args.next().expect("usage: probe_raw_frame <wss-url> <json-frame>");

    let mut request = url.clone().into_client_request()?;
    if let Ok(auth) = UsAuth::from_env() {
        let path = url.split_once("://")
            .and_then(|(_, rest)| rest.find('/').map(|i| rest[i..].to_string()))
            .unwrap_or_else(|| "/".to_string());
        for (name, value) in auth.signed_headers("GET", &path) {
            request.headers_mut().insert(
                tokio_tungstenite::tungstenite::http::header::HeaderName::from_bytes(name.as_bytes())?,
                value.parse()?,
            );
        }
        println!("auth  : signed");
    } else {
        println!("auth  : none");
    }

    println!("url   : {url}");
    println!("frame : {frame}");

    let (stream, _) = tokio_tungstenite::connect_async(request).await?;
    let (mut write, mut read) = stream.split();
    write.send(Message::Text(frame.into())).await?;
    println!("--- replies ---");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(12);
    let mut n = 0;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() || n >= 5 {
            break;
        }
        match tokio::time::timeout(remaining, read.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let s = t.to_string();
                println!("{}", s.chars().take(4000).collect::<String>());
                n += 1;
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => { println!("stream error: {e}"); break; }
            Ok(None) => { println!("closed"); break; }
            Err(_) => { println!("(timeout — no further frames)"); break; }
        }
    }
    Ok(())
}
