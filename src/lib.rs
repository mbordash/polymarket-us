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
//! [`PolymarketUsStreamClient`] maintains a WebSocket with automatic reconnect,
//! re-subscribing on every reconnect. Connections that go silent are torn down
//! after [`StreamConnectConfig::idle_timeout`] so a dead socket cannot stall the
//! stream indefinitely.
//!
//! ```no_run
//! use polymarket_us::{PolymarketUsStreamClient, StreamSubscription};
//!
//! # async fn run() -> Result<(), polymarket_us::PolymarketUsError> {
//! let stream = PolymarketUsStreamClient::from_gateway_base_url(
//!     "https://gateway.polymarket.us",
//!     None,
//! );
//!
//! let mut managed = stream
//!     .connect(vec![StreamSubscription::market_data("BTC-USD")])
//!     .await?;
//!
//! while let Some(message) = managed.next().await {
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
    ManagedStream, PolymarketUsStreamClient, ReconnectConfig, StreamConnectConfig,
    StreamControlEvent, StreamDataEvent, StreamMessage, StreamMessageKind, StreamSubscription,
    SubscriptionChannel,
};
pub use types::{MarketStatus, OrderAction, OrderSide, OrderType, TimeInForce};
