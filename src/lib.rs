//! Unofficial Rust SDK for the Polymarket US Retail API.
//!
//! The crate exposes a typed async REST client and a managed WebSocket stream.
//! Requests to authenticated endpoints are signed with Ed25519 and carry the
//! `X-PM-*` headers automatically.
//!
//! # Getting started
//!
//! Public endpoints need no credentials:
//!
//! ```no_run
//! use polymarket_us::PolymarketUsClient;
//!
//! # async fn run() -> Result<(), polymarket_us::PolymarketUsError> {
//! let client = PolymarketUsClient::builder().build()?;
//! let markets = client.markets().list().await?;
//! println!("{} markets", markets.markets.len());
//! # Ok(())
//! # }
//! ```
//!
//! Authenticated endpoints read credentials from `POLYMARKET_US_KEY_ID` and
//! `POLYMARKET_US_SECRET_KEY`:
//!
//! ```no_run
//! use polymarket_us::{PolymarketUsClient, UsAuth};
//!
//! # async fn run() -> Result<(), polymarket_us::PolymarketUsError> {
//! let client = PolymarketUsClient::builder()
//!     .auth(UsAuth::from_env()?)
//!     .build()?;
//!
//! let balances = client.account().balances().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Resources
//!
//! Endpoints are grouped behind accessors on the client: [`PolymarketUsClient::markets`],
//! [`PolymarketUsClient::events`], [`PolymarketUsClient::orders`],
//! [`PolymarketUsClient::account`], [`PolymarketUsClient::portfolio`], and
//! [`PolymarketUsClient::search`].
//!
//! # Retries
//!
//! Idempotent requests (`GET`, `DELETE`) are retried with exponential backoff and
//! jitter, honouring a server-supplied `Retry-After`. `POST` is **never** retried
//! automatically, so a submitted order cannot be duplicated by the transport
//! layer. See [`RetryConfig`].
//!
//! # Streaming
//!
//! The venue splits its WebSocket surface across two sockets, and the SDK
//! mirrors that split rather than multiplexing them:
//!
//! | Data | Endpoint | Client |
//! |---|---|---|
//! | Books, trades, best-bid/offer | `wss://api.polymarket.us/v1/ws/markets` | [`MarketStreamClient`] |
//! | Orders, positions, balances | `wss://api.polymarket.us/v1/ws/private` | [`PrivateStreamClient`] |
//!
//! Each client maintains its connection with automatic reconnect, replaying its
//! subscriptions every time. Connections that go silent are torn down after
//! [`StreamConnectConfig::idle_timeout`] so a dead socket cannot stall the
//! stream indefinitely, and a keepalive ping keeps a quiet market from tripping
//! that check.
//!
//! ```no_run
//! use polymarket_us::{MarketStreamClient, MarketSubscription};
//!
//! # async fn run() -> Result<(), polymarket_us::PolymarketUsError> {
//! let client = MarketStreamClient::new(None);
//!
//! let mut stream = client
//!     .connect(vec![MarketSubscription::market_data(["btc-100k-2025"])])
//!     .await?;
//!
//! while let Some(message) = stream.next().await {
//!     println!("{:?}", message.kind);
//! }
//! # Ok(())
//! # }
//! ```

pub mod auth;
pub mod client;
pub mod error;
pub mod resources;
pub mod retry;
pub mod stream;
pub mod types;

pub use auth::UsAuth;
pub use client::{PolymarketUsClient, PolymarketUsClientBuilder};
pub use error::PolymarketUsError;
pub use resources::{
    AccountClient, EventsClient, MarketsClient, OrdersClient, PortfolioClient, SearchClient,
};
pub use retry::RetryConfig;
pub use stream::{
    MarketStream, MarketStreamClient, MarketSubscription, PrivateStream, PrivateStreamClient,
    PrivateSubscription, ReconnectConfig, StreamConnectConfig, StreamControlEvent, StreamDataEvent,
    StreamEndpoint, StreamMessage, StreamMessageKind, Subscription, SubscriptionType,
};
pub use types::{MarketStatus, OrderAction, OrderSide, OrderType, TimeInForce};
