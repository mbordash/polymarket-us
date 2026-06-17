# polymarket-us

[![Crates.io](https://img.shields.io/crates/v/polymarket-us.svg)](https://crates.io/crates/polymarket-us)
[![Docs.rs](https://docs.rs/polymarket-us/badge.svg)](https://docs.rs/polymarket-us)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/mbordash/polymarket-us/actions/workflows/ci.yml/badge.svg)](https://github.com/mbordash/polymarket-us/actions/workflows/ci.yml)

Unofficial Rust SDK for the Polymarket US Retail API.

## Features

- **Resource-based API** — Organized into focused clients (`client.markets()`, `client.orders()`, `client.events()`, etc.)
- **Ed25519 request signing** — Automatic X-PM-* authentication headers
- **Typed async REST client** — Markets, events, orders, portfolio, account, and search endpoints
- **Async WebSocket streaming** — Market data and order updates with automatic reconnect
- **Order book & pricing data** — Get order books, best bid/offer, settlement prices
- **Builder-based configuration** — Base URLs, timeouts, custom HTTP client
- **Backward compatible** — All legacy methods still work (deprecated)

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
use polymarket_us::PolymarketUsClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = PolymarketUsClient::builder().build()?;

    // Health check
    let health = client.health().await?;
    println!("status: {}", health.status);

    // List markets
    let markets = client.markets().list().await?;
    println!("markets: {}", markets.markets.len());

    // Get order book for a market
    let book = client.markets().order_book("BTC-USD").await?;
    println!("bid/ask: {} orders", book.bids.len() + book.asks.len());

    Ok(())
}
```

## Resource-Based API

The SDK is organized into focused resource clients for better discoverability and maintainability:

### Markets
Market discovery, order books, and pricing data.

```rust
// List markets
let markets = client.markets().list().await?;

// List with filters
let query = [("limit", "10"), ("category", "politics")];
let page = client.markets().list_with_query(&query).await?;

// Order book and pricing
let book = client.markets().order_book("BTC-USD").await?;
let bbo = client.markets().bbo("BTC-USD").await?;           // Best bid/offer
let settlement = client.markets().settlement_price("BTC-USD").await?;
```

### Events
Event-level metadata and context.

```rust
// List all events
let events = client.events().list().await?;

// Get event by ID or slug
let event = client.events().retrieve("event-123").await?;
let event = client.events().retrieve_by_slug("2024-us-election").await?;
```

### Orders
Complete order lifecycle management. All operations are authenticated.

```rust
use polymarket_us::types;

let order_req = types::PlaceOrderRequest {
    symbol: "BTC-USD".to_string(),
    action: types::order_action::BUY.to_string(),
    outcome_side: types::outcome::LONG.to_string(),
    order_type: types::order_type::LIMIT.to_string(),
    price: types::Money { value: "0.50".to_string(), currency: "USD".to_string() },
    quantity: 100,
    tif: types::tif::GTC.to_string(),
    client_order_id: Some("my-order-1".to_string()),
    post_only: false,
    expires_at: None,
};

// Place order
let order = client.orders().create(&order_req).await?;

// Get open orders
let open = client.orders().open(None::<&()>).await?;

// Modify, cancel, preview
client.orders().modify(&order.order_id, &modify_req).await?;
client.orders().cancel(&order.order_id, &types::CancelOrderParams { quantity: None }).await?;
let estimate = client.orders().preview(&preview_req).await?;

// Close position
client.orders().close_position(&types::ClosePositionRequest {
    symbol: "BTC-USD".to_string(),
    quantity: 50,
}).await?;
```

### Account
Account balances and buying power (authenticated).

```rust
let balances = client.account().balances().await?;
for balance in balances.balances {
    println!("{}; balance={}, buying_power={}",
        balance.currency,
        balance.current_balance,
        balance.buying_power
    );
}
```

### Portfolio
Holdings and activity history (authenticated).

```rust
// Get positions
let positions = client.portfolio().positions().await?;

// Get activity with pagination
let query = [("limit", "50")];
let activities = client.portfolio().activities(&query).await?;
```

### Search
Full-text search across markets and events.

```rust
let query = [("q", "bitcoin")];
let results = client.search().search(&query).await?;

// Search specific resource
let markets = client.search().markets(&query).await?;
let events = client.search().events(&query).await?;
```

## Advanced market queries

Use `list_with_query()` for filters, cursors, and pagination:

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

    let page = client.markets().list_with_query(&query).await?;
    println!("filtered markets: {}", page.markets.len());
    Ok(())
}
```

If your account tier requires authenticated access for some filters, use `list_authenticated_with_query()`:

## Streaming market data

The SDK exposes an async WebSocket client via `client.streaming()`. It handles handshake signing, subscription frames, reconnects, and event parsing.

```rust
use polymarket_us::{PolymarketUsClient, StreamConnectConfig, StreamSubscription};

async fn watch_market(client: &PolymarketUsClient) -> anyhow::Result<()> {
    let stream_client = client.streaming();
    let config = StreamConnectConfig::default().with_responses_debounced(true);

    let mut stream = stream_client
        .connect_with_config(
            vec![StreamSubscription::market_data_lite("BTC-USD")],
            config,
        )
        .await?;

    while let Some(message) = stream.next().await {
        println!("{message:?}");
    }

    Ok(())
}
```

## Endpoint coverage

**Markets** (`client.markets()`):
- `list()` — List all markets
- `list_with_query(q)` — List markets with filters/pagination
- `list_authenticated()` — Authenticated market listing
- `list_authenticated_with_query(q)` — Authenticated with filters
- `order_book(symbol)` — Get market order book
- `bbo(symbol)` — Get best bid/offer
- `settlement_price(symbol)` — Get settlement price

**Events** (`client.events()`):
- `list()` — List all events
- `list_with_query(q)` — List events with filters
- `retrieve(id)` — Get event by ID
- `retrieve_by_slug(slug)` — Get event by slug

**Orders** (`client.orders()`):
- `create(req)` — Create order
- `place(req)` — Place order (alternative endpoint)
- `place_batch(req)` — Place multiple orders atomically
- `open(q)` — Get open orders
- `retrieve(id)` — Get order by ID
- `cancel(id, params)` — Cancel order
- `cancel_trading(id)` — Cancel via trading endpoint
- `cancel_all(params)` — Cancel all orders
- `modify(id, req)` — Modify open order
- `preview(req)` — Preview order estimate
- `close_position(req)` — Close position

**Account** (`client.account()`):
- `balances()` — Get account balances and buying power

**Portfolio** (`client.portfolio()`):
- `positions()` — Get positions
- `activities(q)` — Get activity with pagination

**Search** (`client.search()`):
- `search(q)` — Full-text search across markets/events
- `markets(q)` — Search markets
- `events(q)` — Search events

**Streaming** (`client.streaming()`):
- Async WebSocket client with automatic reconnect and subscription management

## Backward Compatibility

All legacy methods (e.g., `client.markets_list()`, `client.order_create()`) are still available but deprecated. They're aliases to the new resource-based API. Your existing code will continue to work—migrate at your own pace:

```rust
// Old style (deprecated, but still works)
#[allow(deprecated)]
let markets = client.markets_list().await?;

// New style (preferred)
let markets = client.markets().list().await?;
```

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
        Err(PolymarketUsError::RateLimited { message, retry_after }) => {
            if let Some(d) = retry_after {
                eprintln!("rate limited (retry in {}s): {message}", d.as_secs());
            } else {
                eprintln!("rate limited: {message}");
            }
        }
        Err(PolymarketUsError::Authentication(msg)) => eprintln!("auth failed: {msg}"),
        Err(e) => eprintln!("request failed: {e}"),
    }
}
```

## Retries, Correlation IDs, and Rate Limits

### Automatic Retries

`GET` and `DELETE` requests are automatically retried with exponential backoff and jitter.
`POST` requests (order creation, placement, etc.) are **never** retried automatically to
prevent duplicate submissions.

```rust
use polymarket_us::{PolymarketUsClient, RetryConfig};
use std::time::Duration;

// Default: 3 retries, 200ms initial backoff, 10s cap, 25% jitter
let client = PolymarketUsClient::builder().build()?;

// Aggressive retry for high-availability workflows
let client = PolymarketUsClient::builder()
    .retry(RetryConfig::aggressive())
    .build()?;

// Disable retries entirely
let client = PolymarketUsClient::builder()
    .retry(RetryConfig::none())
    .build()?;

// Fine-grained control
let client = PolymarketUsClient::builder()
    .retry(RetryConfig {
        max_retries: 5,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_secs(30),
        jitter_factor: 0.3,
    })
    .build()?;
```

Retries occur on:
- HTTP 429 (respects `Retry-After` header if present)
- HTTP 500, 502, 503, 504
- Transport-level errors (connection refused, timeout)

### Correlation IDs

Every request automatically includes an `X-Correlation-ID` header (`pmrs-{uuid_v4}`) for
tracing requests across your logs and Polymarket support conversations.

```rust
// Custom prefix — useful to distinguish SDK requests by service/environment
let client = PolymarketUsClient::builder()
    .correlation_id_prefix("my-service-prod")
    .build()?;
// Sends: X-Correlation-ID: my-service-prod-550e8400-e29b-41d4-a716-446655440000
```

### Rate Limit Awareness

When Polymarket returns a `429`, the `Retry-After` header is parsed and surfaced in the
`RateLimited` error variant so your application can react precisely:

```rust
match client.markets().list().await {
    Err(PolymarketUsError::RateLimited { retry_after: Some(d), .. }) => {
        println!("backing off for {}s", d.as_secs());
        tokio::time::sleep(d).await;
    }
    _ => {}
}
```

For idempotent endpoints, the SDK already honours this automatically — the `Retry-After`
duration is used directly instead of the configured backoff.

## Testing

The SDK includes comprehensive unit tests for all resource clients and type serialization/deserialization:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test module
cargo test resources::tests

# Run a single test
cargo test resources::tests::place_order_request_serializes
```

Current test coverage includes:
- ✅ Resource client creation and type checking (6 resources × 2 tests = 12 tests)
- ✅ Request/Response serialization for all order types (8+ tests)
- ✅ Type deserialization for markets, events, positions, balances (4+ tests)
- ✅ Client resource accessor availability (2 tests)
- ✅ Plus existing auth, streaming, and client tests (6 tests)

**Total: 36 tests, all passing**

## Acknowledgements

Initial implementation originated in the DRADIS project and was extracted into this crate.

- Project link: `https://github.com/mbordash/DRADIS`
- Attribution is kept for provenance and maintenance history.
