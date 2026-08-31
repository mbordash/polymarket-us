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
- **Async WebSocket streaming** — Separate market-data and private account sockets, with automatic reconnect and keepalive
- **Order book & pricing data** — Get order books, best bid/offer, settlement prices
- **Builder-based configuration** — Base URLs, timeouts, custom HTTP client
- **Automatic retries** — Exponential backoff with jitter on idempotent requests, honouring `Retry-After`; `POST` is never retried, so orders can't be duplicated
- **rustls throughout** — No OpenSSL dependency

## Installation

```sh
cargo add polymarket-us tokio --features tokio/macros,tokio/rt-multi-thread
```

Or in `Cargo.toml`:

```toml
[dependencies]
polymarket-us = "0.8"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Requires Rust 1.86 or newer. TLS is provided by [rustls](https://github.com/rustls/rustls),
so no OpenSSL installation is needed. Root certificates come from the platform
verifier, which means the trust store the rest of the machine already uses — no
bundled root set to go stale.

## Migrating from 0.6 to 0.8

One breaking change: **`GetOpenOrdersResponse.orders` is now `Vec<OpenOrder>`
instead of `Vec<PlaceOrderResponse>`.**

`PlaceOrderResponse` is the acknowledgement the API returns when you *place* an
order, and it carries only an id, a status and quantities. Reusing it for
`GET /v1/orders/open` silently discarded most of the response: the market, the
side, the price and the time-in-force all arrive from the API and were being
thrown away, so callers could not tell which order they were looking at or act on
one safely.

(0.7.0 made this same change but introduced an `Amount` type duplicating the
existing `Money`. 0.8.0 removes `Amount` and uses `Money`. If you already moved
to 0.7.0, the only further change is that name.)

`OpenOrder` models the documented response instead:

```rust
pub struct OpenOrder {
    pub id: String,
    pub market_slug: String,               // "marketSlug"
    pub side: Option<OrderSideDirection>,  // ORDER_SIDE_BUY / ORDER_SIDE_SELL
    pub outcome_side: Option<OutcomeSide>, // OUTCOME_SIDE_YES / OUTCOME_SIDE_NO
    pub price: Option<Money>,
    pub quantity: f64,                     // original size, in contracts
    pub cum_quantity: f64,                 // "cumQuantity" — filled so far
    pub leaves_quantity: f64,              // "leavesQuantity" — still resting
    pub tif: Option<TimeInForce>,
    pub intent: Option<OrderIntent>,
    pub state: String,
    pub create_time: Option<String>,
    pub good_till_time: Option<String>,
}
```

### What to change

If you only read the order id, the field is renamed and nothing else moves:

```rust
// 0.6
for o in &open.orders { cancel(&o.order_id).await?; }

// 0.7
for o in &open.orders { cancel(&o.id).await?; }
```

If you read `status`, `filled_quantity` or `remaining_quantity`, they become
`state`, `cum_quantity` and `leaves_quantity`. `state` is a `String` because the
API documents the enum's name but not its values, so it is not modeled as one.

Everything else the API sends is now available rather than dropped, so most
callers can delete whatever bookkeeping they kept to work around the gap.

### Unknown values do not fail the response

`OrderSideDirection`, `OutcomeSide` and `OrderIntent` each carry an `Unknown`
variant, and every field is `#[serde(default)]`. A value this crate has not seen
before deserializes as `Unknown` rather than failing the whole response.

That is deliberate rather than merely lenient. The common use for open orders is
cancelling leftovers after a restart, and a response that fails to parse there
leaves real orders working on the venue with nothing managing them. One
unrecognized enum is much the lesser harm, and `id` — all `cancel` needs — still
arrives.

### Minimum supported Rust version

The MSRV is **1.86**, declared as `rust-version` in `Cargo.toml` and verified in
CI: every push compiles *and* runs the test suite on exactly that toolchain.

It is a tracked floor rather than a support promise. It follows what the
dependency tree requires, and **may rise in any minor release** — it is not
treated as a breaking change. Pin `polymarket-us = "=0.8.0"` if you need a
toolchain guarantee.

In practice the floor moves rarely and only for a reason. The crate uses Cargo's
MSRV-aware resolver (`resolver = "3"`), so a dependency version needing a newer
compiler is simply not selected; a routine `cargo update` cannot quietly raise
what you need to build. CI fails when a *direct* dependency gets held back that
way, and that failure is the trigger to raise the floor deliberately — to the
version actually required, not to whatever is current.

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
use polymarket_us::{PolymarketUsClient, PolymarketUsError};

#[tokio::main]
async fn main() -> Result<(), PolymarketUsError> {
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
    action: types::OrderAction::Buy,
    outcome_side: types::OrderSide::Long,
    order_type: types::OrderType::Limit,
    price: types::Money { value: "0.50".to_string(), currency: "USD".to_string() },
    quantity: 100,
    tif: types::TimeInForce::GoodTillCancel,
    client_order_id: Some("my-order-1".to_string()),
    post_only: false,
    expires_at: None,
};

// Place order
let order = client.orders().create(&order_req).await?;

// Get open orders. Each entry carries the market, side, price and quantities —
// see the migration note below if you are coming from 0.6 or 0.7.
let open = client.orders().open(None::<&()>).await?;
for order in &open.orders {
    println!(
        "{} {} {:?} {} @ {}",
        order.id,
        order.market_slug,
        order.outcome_side,
        order.leaves_quantity,
        order.price.as_ref().map(|p| p.value.as_str()).unwrap_or("-"),
    );
}

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
use polymarket_us::{PolymarketUsClient, PolymarketUsError};
use serde::Serialize;

#[derive(Serialize)]
struct MarketsQuery<'a> {
    category: Option<&'a str>,
    limit: Option<u32>,
    cursor: Option<&'a str>,
}

async fn load_filtered_markets(client: &PolymarketUsClient) -> Result<(), PolymarketUsError> {
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

If your account tier requires authenticated access for some filters, use
`list_authenticated_with_query()`, which takes the same query argument.

## Streaming

The venue splits its WebSocket surface across two sockets on the **API** host
(not the gateway host used for public REST traffic), and the SDK mirrors that
split rather than multiplexing them:

| Data | Endpoint | Client | Subscriptions |
|---|---|---|---|
| Books, trades, best-bid/offer | `wss://api.polymarket.us/v1/ws/markets` | `MarketStreamClient` | `MarketSubscription` |
| Orders, positions, balances | `wss://api.polymarket.us/v1/ws/private` | `PrivateStreamClient` | `PrivateSubscription` |

Because the two subscription families are distinct types, subscribing to an
order feed on the market socket is a compile error rather than a server
rejection.

### Wire format

A subscription serializes to the server's `subscribe` envelope:

```json
{
  "subscribe": {
    "requestId": "md-sub-1",
    "subscriptionType": "SUBSCRIPTION_TYPE_MARKET_DATA",
    "marketSlugs": ["btc-100k-2025"]
  }
}
```

and unsubscribing echoes the same `requestId`:

```json
{ "unsubscribe": { "requestId": "md-sub-1" } }
```

`MarketSubscription::frame()` / `PrivateSubscription::frame()` return exactly
what will be sent, which is the quickest way to check a subscription against the
docs. Note that the endpoint rejects a frame it cannot parse — including one
carrying a field it does not define — so the SDK sends only the three documented
fields. `insert_extra` adds more when the docs call for it.

Subscription types map to `SubscriptionType`, whose wire form is the
fully-qualified enum name:

| Constructor | `subscriptionType` |
|---|---|
| `MarketSubscription::market_data(slugs)` | `SUBSCRIPTION_TYPE_MARKET_DATA` |
| `MarketSubscription::market_data_lite(slugs)` | `SUBSCRIPTION_TYPE_MARKET_DATA_LITE` |
| `MarketSubscription::trades(slugs)` | `SUBSCRIPTION_TYPE_TRADE` |
| `PrivateSubscription::orders()` | `SUBSCRIPTION_TYPE_ORDER` |
| `PrivateSubscription::positions()` | `SUBSCRIPTION_TYPE_POSITION` |
| `PrivateSubscription::account_balances()` | `SUBSCRIPTION_TYPE_ACCOUNT_BALANCE` |

A type the SDK does not model yet can still be used via `custom()`, which sends
the string verbatim.

### Market data

```rust
use polymarket_us::{
    MarketSubscription, PolymarketUsClient, PolymarketUsError, StreamDataEvent,
    StreamMessageKind,
};

async fn watch_market(client: &PolymarketUsClient) -> Result<(), PolymarketUsError> {
    let mut stream = client
        .market_stream()
        .connect(vec![
            MarketSubscription::market_data_lite(["btc-100k-2025"]),
            MarketSubscription::trades(["btc-100k-2025"]),
        ])
        .await?;

    // Add and remove subscriptions at runtime.
    let extra = MarketSubscription::market_data(["eth-10k-2025"]);
    let request_id = extra.request_id().to_string();
    stream.subscribe(extra).await?;
    stream.unsubscribe(&request_id).await?;

    while let Some(message) = stream.next().await {
        match message.kind {
            StreamMessageKind::Data(StreamDataEvent::Trade(payload)) => {
                println!("trade: {payload}");
            }
            StreamMessageKind::Data(StreamDataEvent::MarketDataLite(payload)) => {
                println!("bbo: {payload}");
            }
            _ => {}
        }
    }

    Ok(())
}
```

### Private account events

`private_stream()` fails with `MissingAuth` if the client has no credentials,
since the endpoint rejects an unauthenticated upgrade.

```rust
use polymarket_us::{PolymarketUsClient, PolymarketUsError, PrivateSubscription};

async fn watch_account(client: &PolymarketUsClient) -> Result<(), PolymarketUsError> {
    let mut stream = client
        .private_stream()?
        .connect(vec![
            PrivateSubscription::orders(),
            PrivateSubscription::positions(),
            PrivateSubscription::account_balances(),
        ])
        .await?;

    while let Some(message) = stream.next().await {
        println!("{:?}", message.kind);
    }

    Ok(())
}
```

### Staying connected

Both streams reconnect automatically and replay their subscriptions each time.

A connection that goes quiet for `StreamConnectConfig::idle_timeout` (60s by
default) is torn down and reconnected. This matters because a TCP connection can
die without a FIN or RST — common behind NAT and load balancers — in which case
the socket never reports an error and the stream would otherwise wait forever.

Feeding that check is `keepalive_interval` (20s by default): the SDK pings the
server, and the pong counts as traffic. That distinction is what keeps a market
with nothing to report from being mistaken for a dead socket. Pass `None` to
either to switch it off.

```rust
use std::time::Duration;
use polymarket_us::StreamConnectConfig;

let config = StreamConnectConfig::default()
    .with_idle_timeout(Some(Duration::from_secs(30)))
    .with_keepalive_interval(Some(Duration::from_secs(10)));
```

Inbound events arrive as `StreamMessageKind::Data(StreamDataEvent::…)`:
`MarketData`, `MarketDataLite`, `OrderBookDelta`, `Trade`, `OrderSnapshot`,
`OrderUpdate`, `PositionSnapshot`, `PositionUpdate`, `BalanceSnapshot`,
`BalanceUpdate`, `Heartbeat`, and `Other` for anything not yet modelled.
`StreamMessage::request_id` carries the `requestId` the server echoed, matching
the subscription that produced it.

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

**Streaming** (`client.market_stream()` / `client.private_stream()`):
- Two endpoint-specific clients mirroring the server's `/v1/ws/markets` and `/v1/ws/private` split
- Typed subscription types via `SubscriptionType`, with `custom()` for unmodelled ones
- Dynamic `subscribe(...)` / `unsubscribe(...)` by `requestId`
- Automatic reconnect with subscription replay, keepalive pings, and idle-connection teardown

## Migrating to 0.5

**The streaming API in 0.4 did not work against the live venue.** It sent a flat
frame — `{"channel": "market_data", "trackingId": ..., "symbol": ...}` — that the
server never parsed as a subscribe request, so every subscription came back as
`{"error":"invalid_message"}`. It also derived its URL from the gateway host,
which does not serve WebSockets. 0.5 replaces that layer; there is no compatible
upgrade path, but the mapping is mechanical.

**One client became two**, matching the server's own split:

```rust
// 0.4 — one client, one socket, derived from the gateway host
let stream = client.streaming();

// 0.5 — wss://api.polymarket.us/v1/ws/markets
let markets = client.market_stream();
// 0.5 — wss://api.polymarket.us/v1/ws/private
let private = client.private_stream()?;
```

**`StreamSubscription` became `MarketSubscription` and `PrivateSubscription`**,
and takes market slugs rather than a symbol:

```rust
// 0.4
StreamSubscription::market_data("BTC-USD")
StreamSubscription::trades("BTC-USD")
StreamSubscription::order_snapshot("BTC-USD")
StreamSubscription::order_update()
StreamSubscription::position_snapshot()   // and position_update()
StreamSubscription::balance_snapshot()    // and balance_update()

// 0.5
MarketSubscription::market_data(["btc-100k-2025"])
MarketSubscription::trades(["btc-100k-2025"])
PrivateSubscription::orders()
PrivateSubscription::positions()
PrivateSubscription::account_balances()
```

The snapshot/update split is gone from the *subscribe* side — the server has one
subscription type per family — but survives on the receive side, where
`StreamDataEvent::OrderSnapshot` and `OrderUpdate` are still distinct.

Everything else that changed:

- **`SubscriptionChannel` became `SubscriptionType`**, and its wire form is the
  fully-qualified enum name (`SUBSCRIPTION_TYPE_TRADE`, not `trade`). Unmodelled
  types go through `MarketSubscription::custom` / `PrivateSubscription::custom`.
- **`tracking_id` became `request_id`** throughout, matching the wire field.
  `StreamMessage::tracking_id` is now `StreamMessage::request_id`, and
  `unsubscribe` takes the `requestId` the subscription was created with.
- **`ManagedStream` became `MarketStream` and `PrivateStream`.** Each accepts
  only its own endpoint's subscription type, so the two sockets cannot be
  crossed.
- **`StreamSubscription::heartbeat()` is gone.** There is no heartbeat
  subscription type; keeping the connection warm is now the SDK's job, via
  `StreamConnectConfig::keepalive_interval`. If you subscribed to the heartbeat
  purely to feed `idle_timeout`, drop the subscription — that is handled.
- **`responses_debounced` is gone** from both `StreamSubscription` and
  `StreamConnectConfig`. It is not part of the documented subscribe object, and
  sending an undefined field risks the same `invalid_message` rejection.
- **`StreamConnectConfig::tracking_id` became `session_id`**, and is explicitly
  local — it identifies the connection in control events and is never sent.
- **`PolymarketUsStreamClient::from_gateway_base_url` is gone.** The sockets are
  not on the gateway host. `MarketStreamClient::with_base_url` and
  `PrivateStreamClient::with_base_url` take an explicit override for staging or
  local servers.

## Migrating to 0.4

The flat legacy methods deprecated in 0.3.0 have been removed. Each maps to a
resource client:

```rust
// Removed in 0.4
let markets = client.markets_list().await?;
let balances = client.account_balances().await?;
let order = client.place_order(&req).await?;

// Use instead
let markets = client.markets().list().await?;
let balances = client.account().balances().await?;
let order = client.orders().place(&req).await?;
```

The general rule: `client.<resource>_<verb>()` becomes `client.<resource>().<verb>()`.

Three other breaking changes:

- **`UsAuth` returns `PolymarketUsError`, not `anyhow::Error`.** `UsAuth::from_env()`
  and `UsAuth::from_parts()` now return `Result<UsAuth, PolymarketUsError>`, so
  credential failures can be matched on like every other SDK error. Bad Base64 or a
  wrong key length surfaces as `PolymarketUsError::InvalidCredentials`. If you were
  using `?` inside an `anyhow::Result` function, no change is needed.
- **`UsMarket::market_sides` is `Vec<MarketSide>`**, previously `Vec<serde_json::Value>`.
  Unmodelled keys are preserved in `MarketSide::extra`.
- **`League` and `Team` were removed.** They were unreachable placeholders that no
  endpoint returned; they will return with the endpoints that populate them.

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

For idempotent endpoints, the SDK already honors this automatically — the `Retry-After`
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
- ✅ Request/Response serialization for all order types (typed enums + wire compatibility)
- ✅ Type deserialization for markets, events, positions, balances
- ✅ Streaming wire format, endpoint routing, event parsing, and keepalive/idle behaviour
- ✅ Gateway quirks in market deserialization (double-encoded `outcomes` / `outcomePrices`)
- ✅ Retry/backoff policy tests and builder configuration tests

**Total: 93 tests plus 6 doc tests, all passing**

## Acknowledgements

Initial implementation originated in the DRADIS project and was extracted into this crate.

- Project link: `https://github.com/mbordash/DRADIS`
- Attribution is kept for provenance and maintenance history.
