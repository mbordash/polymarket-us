//! Stream live market data with automatic reconnect and dead-connection detection.
//!
//! ```sh
//! cargo run --example stream_market_data -- BTC-USD
//! ```

use polymarket_us::{
    PolymarketUsError, PolymarketUsStreamClient, ReconnectConfig, StreamConnectConfig,
    StreamControlEvent, StreamDataEvent, StreamMessageKind, StreamSubscription,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "BTC-USD".into());

    let stream = PolymarketUsStreamClient::from_gateway_base_url(
        "https://gateway.polymarket.us",
        None, // public channels need no auth
    );

    let config = StreamConnectConfig::default()
        .with_reconnect(ReconnectConfig::default())
        // Tear down and reconnect if the server goes quiet for 30s. The
        // heartbeat subscription below guarantees regular traffic.
        .with_idle_timeout(Some(Duration::from_secs(30)));

    let mut managed = stream
        .connect_with_config(
            vec![
                StreamSubscription::market_data_lite(&symbol),
                StreamSubscription::trades(&symbol),
                StreamSubscription::heartbeat(),
            ],
            config,
        )
        .await?;

    println!("streaming {symbol} — press Ctrl-C to stop");

    while let Some(message) = managed.next().await {
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
