//! Stream private account events — orders, positions, and balances.
//!
//! ```sh
//! cargo run --example stream_private
//! ```
//!
//! This is a separate socket from market data (`/v1/ws/private` rather than
//! `/v1/ws/markets`), so it needs its own connection and always needs
//! credentials: `POLYMARKET_US_KEY_ID` / `POLYMARKET_US_SECRET_KEY`.

use polymarket_us::{
    PolymarketUsError, PrivateStreamClient, PrivateSubscription, StreamControlEvent,
    StreamDataEvent, StreamMessageKind, UsAuth,
};

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    let client = PrivateStreamClient::new(UsAuth::from_env()?);

    let mut stream = client
        .connect(vec![
            PrivateSubscription::orders(),
            PrivateSubscription::positions(),
            PrivateSubscription::account_balances(),
        ])
        .await?;

    println!("streaming account events — press Ctrl-C to stop");

    while let Some(message) = stream.next().await {
        match message.kind {
            StreamMessageKind::Data(StreamDataEvent::OrderSnapshot(payload))
            | StreamMessageKind::Data(StreamDataEvent::OrderUpdate(payload)) => {
                println!("order: {payload}");
            }
            StreamMessageKind::Data(StreamDataEvent::PositionSnapshot(payload))
            | StreamMessageKind::Data(StreamDataEvent::PositionUpdate(payload)) => {
                println!("position: {payload}");
            }
            StreamMessageKind::Data(StreamDataEvent::BalanceSnapshot(payload))
            | StreamMessageKind::Data(StreamDataEvent::BalanceUpdate(payload)) => {
                println!("balance: {payload}");
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
