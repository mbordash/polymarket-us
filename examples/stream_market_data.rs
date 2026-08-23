//! Stream live market data with automatic reconnect and dead-connection detection.
//!
//! ```sh
//! cargo run --example stream_market_data -- btc-100k-2025
//! ```
//!
//! Takes market *slugs*, not ticker symbols — the same identifiers the REST
//! market listing returns. Credentials are picked up from
//! `POLYMARKET_US_KEY_ID` / `POLYMARKET_US_SECRET_KEY` when present; the live
//! endpoint has been observed to reject an unauthenticated upgrade.

use polymarket_us::{
    MarketStreamClient, MarketSubscription, PolymarketUsError, ReconnectConfig,
    StreamConnectConfig, StreamControlEvent, StreamDataEvent, StreamMessageKind, UsAuth,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    let slugs: Vec<String> = match std::env::args().skip(1).collect::<Vec<_>>() {
        empty if empty.is_empty() => vec!["btc-100k-2025".to_string()],
        provided => provided,
    };

    let client = MarketStreamClient::new(UsAuth::from_env().ok());

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::default())
        // Tear down and reconnect if the peer stops answering for 30s. The
        // keepalive ping (on by default) is what proves it is still there when
        // the market itself has nothing to report.
        .with_idle_timeout(Some(Duration::from_secs(30)));

    let mut stream = client
        .connect_with_config(
            vec![
                MarketSubscription::market_data_lite(slugs.clone()),
                MarketSubscription::trades(slugs.clone()),
            ],
            config,
        )
        .await?;

    println!("streaming {} — press Ctrl-C to stop", slugs.join(", "));

    while let Some(message) = stream.next().await {
        match message.kind {
            StreamMessageKind::Data(StreamDataEvent::MarketDataLite(payload)) => {
                println!("bbo: {payload}");
            }
            StreamMessageKind::Data(StreamDataEvent::Trade(payload)) => {
                println!("trade: {payload}");
            }
            StreamMessageKind::Data(StreamDataEvent::Heartbeat) => {}
            StreamMessageKind::Control(StreamControlEvent::Reconnecting { attempt, delay_ms }) => {
                eprintln!("reconnecting (attempt {attempt}) in {delay_ms}ms");
            }
            StreamMessageKind::Control(StreamControlEvent::Error(err)) => {
                eprintln!("stream error: {err}");
            }
            StreamMessageKind::Control(StreamControlEvent::Closed) => break,
            other => println!("{other:?}"),
        }
    }

    Ok(())
}
