//! Print every frame either endpoint sends, verbatim, for one subscription.
//!
//! Useful for checking what the server actually returns for a subscription
//! type before writing code against it.
//!
//! ```sh
//! cargo run --example probe_stream_endpoint -- markets SUBSCRIPTION_TYPE_MARKET_DATA <slug>
//! cargo run --example probe_stream_endpoint -- private SUBSCRIPTION_TYPE_ORDER
//! ```
//!
//! Credentials come from `POLYMARKET_US_KEY_ID` / `POLYMARKET_US_SECRET_KEY`.
//! They are required for the private endpoint and recommended for markets,
//! which has been observed to reject an unauthenticated upgrade with 401.

use polymarket_us::{
    MarketStreamClient, MarketSubscription, PolymarketUsError, PrivateStreamClient,
    PrivateSubscription, StreamConnectConfig, UsAuth,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    let mut args = std::env::args().skip(1);
    let endpoint = args.next().unwrap_or_else(|| "markets".to_string());
    let subscription_type = args
        .next()
        .unwrap_or_else(|| "SUBSCRIPTION_TYPE_MARKET_DATA".to_string());
    let slugs: Vec<String> = args.collect();

    let auth = UsAuth::from_env().ok();
    println!("endpoint : {endpoint}");
    println!("type     : {subscription_type}");
    println!("slugs    : {slugs:?}");
    println!("auth     : {}", if auth.is_some() { "yes" } else { "no" });

    let config = StreamConnectConfig::default().with_idle_timeout(Some(Duration::from_secs(20)));

    match endpoint.as_str() {
        "markets" => {
            let subscription =
                MarketSubscription::custom(subscription_type).with_market_slugs(slugs);
            println!("frame    : {}", subscription.frame());

            let client = MarketStreamClient::new(auth);
            let mut stream = client
                .connect_with_config(vec![subscription], config)
                .await?;
            let mut seen = 0;
            while let Some(message) = stream.next().await {
                println!("{:?}", message.kind);
                seen += 1;
                if seen >= 12 {
                    break;
                }
            }
            println!("--- {seen} frame(s) ---");
        }
        "private" => {
            let subscription =
                PrivateSubscription::custom(subscription_type).with_market_slugs(slugs);
            println!("frame    : {}", subscription.frame());

            let client = PrivateStreamClient::new(UsAuth::from_env()?);
            let mut stream = client
                .connect_with_config(vec![subscription], config)
                .await?;
            let mut seen = 0;
            while let Some(message) = stream.next().await {
                println!("{:?}", message.kind);
                seen += 1;
                if seen >= 12 {
                    break;
                }
            }
            println!("--- {seen} frame(s) ---");
        }
        other => {
            eprintln!("unknown endpoint {other:?} — expected \"markets\" or \"private\"");
        }
    }

    Ok(())
}
