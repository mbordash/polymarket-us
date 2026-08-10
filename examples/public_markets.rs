//! Read public market data — no credentials required.
//!
//! ```sh
//! cargo run --example public_markets
//! ```

use polymarket_us::{PolymarketUsClient, PolymarketUsError};

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    let client = PolymarketUsClient::builder().build()?;

    let health = client.health().await?;
    println!("gateway status: {}", health.status);

    let markets = client.markets().list().await?;
    println!("fetched {} markets", markets.markets.len());

    for market in markets.markets.iter().take(5) {
        println!(
            "  {:<40} {:?} ({} sides)",
            market.question,
            market.parsed_status(),
            market.market_sides.len()
        );
    }

    // Order book for the first market, if there is one.
    if let Some(first) = markets.markets.first() {
        let book = client.markets().order_book(&first.slug).await?;
        println!(
            "book for {}: {} bids / {} asks",
            first.slug,
            book.bids.len(),
            book.asks.len()
        );
    }

    Ok(())
}
