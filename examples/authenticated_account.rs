//! Read balances and positions using signed requests.
//!
//! Requires credentials in the environment:
//!
//! ```sh
//! export POLYMARKET_US_KEY_ID="your-key-id"
//! export POLYMARKET_US_SECRET_KEY="your-base64-secret"
//! cargo run --example authenticated_account
//! ```

use polymarket_us::{PolymarketUsClient, PolymarketUsError, RetryConfig, UsAuth};

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
    // Credential problems surface as a typed error rather than a string.
    let auth = match UsAuth::from_env() {
        Ok(auth) => auth,
        Err(PolymarketUsError::InvalidCredentials(reason)) => {
            eprintln!("credentials unusable: {reason}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };

    let client = PolymarketUsClient::builder()
        .auth(auth)
        .retry(RetryConfig::aggressive())
        .correlation_id_prefix("example")
        .build()?;

    let balances = client.account().balances().await?;
    for balance in &balances.balances {
        println!(
            "{}: balance {:.2}, buying power {:.2}",
            balance.currency, balance.current_balance, balance.buying_power
        );
    }

    let positions = client.portfolio().positions().await?;
    println!("{} open positions", positions.positions.len());
    for (symbol, position) in positions.positions.iter().take(10) {
        println!(
            "  {symbol}: qty {} @ {}",
            position.quantity, position.avg_entry_price
        );
    }

    Ok(())
}
