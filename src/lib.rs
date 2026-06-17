pub mod auth;
pub mod client;
pub mod error;
pub mod resources;
pub mod stream;
pub mod types;

pub use auth::UsAuth;
pub use client::{PolymarketUsClient, PolymarketUsClientBuilder};
pub use error::PolymarketUsError;
pub use resources::{
    AccountClient, EventsClient, MarketsClient, OrdersClient, PortfolioClient, SearchClient,
};
pub use stream::{
    ManagedStream, PolymarketUsStreamClient, ReconnectConfig, StreamConnectConfig,
    StreamControlEvent, StreamDataEvent, StreamMessage, StreamMessageKind, StreamSubscription,
};
