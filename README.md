# polymarket-us

Unofficial Rust SDK for the Polymarket US Retail API.

## Features

- Ed25519 request signing for `X-PM-*` auth headers.
- Typed async REST client for markets, orders, portfolio, and account endpoints.
- Builder-based configuration for base URLs, timeouts, and custom `reqwest::Client`.
- Error mapping for common HTTP status classes.

## Installation

This crate is currently easiest to consume from source or git:

```toml
[dependencies]
polymarket-us = { path = "../polymarket-us" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via git:

```toml
[dependencies]
polymarket-us = { git = "https://github.com/mbordash/DRADIS", package = "polymarket-us" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Authentication

Authenticated endpoints require:

- `POLYMARKET_US_KEY_ID`
- `POLYMARKET_US_SECRET_KEY`

`POLYMARKET_US_SECRET_KEY` must be Base64 that decodes to either:

- 64 bytes (keypair format, first 32 bytes are used as signing seed), or
- 32 bytes (raw Ed25519 seed).

Example:

```bash
export POLYMARKET_US_KEY_ID="your-key-id"
export POLYMARKET_US_SECRET_KEY="your-base64-secret"
```

## Quick start

```rust
use polymarket_us::{PolymarketUsClient, UsAuth};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = UsAuth::from_env()?;
    let client = PolymarketUsClient::builder().auth(auth).build()?;

    let health = client.health().await?;
    println!("gateway status: {}", health.status);

    let markets = client.markets_list().await?;
    println!("markets returned: {}", markets.markets.len());
    Ok(())
}
```

## Advanced market queries

Use `markets_list()` for the default market feed. Use `markets_list_with_query(...)` when you need filters, cursors, or pagination parameters.

```rust
use polymarket_us::PolymarketUsClient;
use serde::Serialize;

#[derive(Serialize)]
struct MarketsQuery<'a> {
    category: Option<&'a str>,
    limit: Option<u32>,
    cursor: Option<&'a str>,
}

async fn load_filtered_markets(client: &PolymarketUsClient) -> anyhow::Result<()> {
    let query = MarketsQuery {
        category: Some("politics"),
        limit: Some(25),
        cursor: None,
    };

    let page = client.markets_list_with_query(Some(&query)).await?;
    println!("filtered markets: {}", page.markets.len());
    Ok(())
}
```

If your account tier requires authenticated market access for some filters, use `markets_list_authenticated_with_query(...)` with the same query struct.

## Endpoint coverage

Public:

- `health`
- `markets_list`
- `markets_list_authenticated`
- `markets_list_with_query`
- `markets_list_authenticated_with_query`

Account / portfolio:

- `account_balances`
- `portfolio_positions`
- `portfolio_activities`

Trading / orders:

- `place_order`
- `place_batched_orders`
- `cancel_trading_order`
- `orders_create`
- `orders_open`
- `order_retrieve`
- `order_cancel`
- `order_modify`
- `orders_cancel_all`
- `order_preview`
- `order_close_position`

## Configuration

```rust
use polymarket_us::{PolymarketUsClient, UsAuth};
use std::time::Duration;

fn build_client(auth: UsAuth) -> Result<PolymarketUsClient, polymarket_us::PolymarketUsError> {
    PolymarketUsClient::builder()
        .auth(auth)
        .gateway_base_url("https://gateway.polymarket.us")
        .api_base_url("https://api.polymarket.us")
        .timeout(Duration::from_secs(30))
        .build()
}
```

## Error handling

```rust
use polymarket_us::{PolymarketUsClient, PolymarketUsError};

async fn check_health(client: &PolymarketUsClient) {
    match client.health().await {
        Ok(h) => println!("ok: {}", h.status),
        Err(PolymarketUsError::RateLimited(msg)) => eprintln!("rate limited: {msg}"),
        Err(PolymarketUsError::Authentication(msg)) => eprintln!("auth failed: {msg}"),
        Err(e) => eprintln!("request failed: {e}"),
    }
}
```

## Roadmap

- [ ] WebSocket support for real-time market data subscriptions.
- [ ] WebSocket user streams for order/fill updates.
- [ ] Reconnect and backoff helpers for long-running stream consumers.

## Acknowledgements

Initial implementation originated in the DRADIS project and was extracted into this crate.

- Project link: `https://github.com/mbordash/DRADIS`
- Attribution is kept for provenance and maintenance history.
