use crate::auth::{unix_timestamp_millis, UsAuth};
use crate::error::PolymarketUsError;
use futures_util::{SinkExt, StreamExt};
use http::HeaderValue;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Host serving both WebSocket endpoints. Note this is the API host, not the
/// gateway host used for public REST traffic.
const DEFAULT_STREAM_HOST: &str = "wss://api.polymarket.us";

type WebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

// ---------------------------------------------------------------------------
// Endpoints
// ---------------------------------------------------------------------------

/// The two WebSocket surfaces exposed by the venue.
///
/// They are separate sockets on separate paths, so a single connection can
/// never carry both market data and private account events. The SDK mirrors
/// that split: see [`MarketStreamClient`] and [`PrivateStreamClient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamEndpoint {
    /// Order books, trades, and best-bid/offer. Path `/v1/ws/markets`.
    Markets,
    /// Orders, positions, and account balances. Path `/v1/ws/private`.
    Private,
}

impl StreamEndpoint {
    /// The endpoint's path on the API host.
    pub fn path(self) -> &'static str {
        match self {
            Self::Markets => "/v1/ws/markets",
            Self::Private => "/v1/ws/private",
        }
    }

    /// The full default URL for this endpoint.
    pub fn default_url(self) -> String {
        format!("{DEFAULT_STREAM_HOST}{}", self.path())
    }
}

impl std::fmt::Display for StreamEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Markets => "markets",
            Self::Private => "private",
        })
    }
}

// ---------------------------------------------------------------------------
// Subscription types
// ---------------------------------------------------------------------------

/// A value of the server's `subscriptionType` enum.
///
/// The wire format is the fully-qualified enum name — `SUBSCRIPTION_TYPE_TRADE`,
/// not `trade`. Use [`SubscriptionType::as_wire`] to see the exact string that
/// goes on the socket.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubscriptionType {
    /// Full order-book depth. Markets endpoint.
    MarketData,
    /// Best-bid/offer only. Markets endpoint.
    MarketDataLite,
    /// Trade executions. Markets endpoint.
    Trade,
    /// Order lifecycle events. Private endpoint.
    Order,
    /// Position changes. Private endpoint.
    Position,
    /// Account balance changes. Private endpoint.
    AccountBalance,
    /// A subscription type this SDK does not model yet.
    ///
    /// The contained string is sent verbatim, so it must already be in wire
    /// form (`SUBSCRIPTION_TYPE_...`). This exists so a newly documented type
    /// can be used without waiting for a crate release.
    Other(String),
}

impl SubscriptionType {
    /// The exact string sent as `subscriptionType`.
    pub fn as_wire(&self) -> &str {
        match self {
            Self::MarketData => "SUBSCRIPTION_TYPE_MARKET_DATA",
            Self::MarketDataLite => "SUBSCRIPTION_TYPE_MARKET_DATA_LITE",
            Self::Trade => "SUBSCRIPTION_TYPE_TRADE",
            Self::Order => "SUBSCRIPTION_TYPE_ORDER",
            Self::Position => "SUBSCRIPTION_TYPE_POSITION",
            Self::AccountBalance => "SUBSCRIPTION_TYPE_ACCOUNT_BALANCE",
            Self::Other(raw) => raw,
        }
    }

    /// Parse a wire string back into a variant, falling back to
    /// [`SubscriptionType::Other`] for anything unrecognised.
    pub fn from_wire(raw: &str) -> Self {
        match raw {
            "SUBSCRIPTION_TYPE_MARKET_DATA" => Self::MarketData,
            "SUBSCRIPTION_TYPE_MARKET_DATA_LITE" => Self::MarketDataLite,
            "SUBSCRIPTION_TYPE_TRADE" => Self::Trade,
            "SUBSCRIPTION_TYPE_ORDER" => Self::Order,
            "SUBSCRIPTION_TYPE_POSITION" => Self::Position,
            "SUBSCRIPTION_TYPE_ACCOUNT_BALANCE" => Self::AccountBalance,
            other => Self::Other(other.to_string()),
        }
    }

    /// Which socket carries this type, or `None` for an unmodelled type.
    pub fn endpoint(&self) -> Option<StreamEndpoint> {
        match self {
            Self::MarketData | Self::MarketDataLite | Self::Trade => Some(StreamEndpoint::Markets),
            Self::Order | Self::Position | Self::AccountBalance => Some(StreamEndpoint::Private),
            Self::Other(_) => None,
        }
    }

    /// Whether this type needs at least one entry in `marketSlugs`.
    fn requires_market_slugs(&self) -> bool {
        matches!(self, Self::MarketData | Self::MarketDataLite | Self::Trade)
    }
}

impl std::fmt::Display for SubscriptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_wire())
    }
}

impl Serialize for SubscriptionType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_wire())
    }
}

impl<'de> Deserialize<'de> for SubscriptionType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::from_wire(&raw))
    }
}

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

/// The body of a `subscribe` request — serialized as the *inner* object, since
/// the envelope is added when the frame is sent.
///
/// Only fields the server documents are emitted. `SUBSCRIPTION_TYPE_*` is
/// protobuf-style naming and such servers commonly reject unknown fields
/// outright, so nothing speculative is added here; `extra` carries only what a
/// caller asked for explicitly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    /// Correlates the server's acknowledgement, and identifies the
    /// subscription when unsubscribing.
    pub request_id: String,
    pub subscription_type: SubscriptionType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub market_slugs: Vec<String>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

impl Subscription {
    fn new(subscription_type: SubscriptionType) -> Self {
        Self {
            request_id: next_request_id("sub"),
            subscription_type,
            market_slugs: Vec::new(),
            extra: Map::new(),
        }
    }

    /// The frame as it goes on the socket, envelope included.
    fn frame(&self) -> Value {
        serde_json::json!({ "subscribe": self })
    }

    fn validate(&self, endpoint: StreamEndpoint) -> Result<(), PolymarketUsError> {
        if let Some(required) = self.subscription_type.endpoint() {
            if required != endpoint {
                return Err(PolymarketUsError::InvalidStreamConfig(format!(
                    "{} is served by the {required} endpoint, not {endpoint}",
                    self.subscription_type
                )));
            }
        }

        if self.subscription_type.requires_market_slugs() && self.market_slugs.is_empty() {
            return Err(PolymarketUsError::InvalidStreamConfig(format!(
                "{} requires at least one market slug",
                self.subscription_type
            )));
        }

        Ok(())
    }
}

/// Shared builders for both subscription newtypes.
macro_rules! subscription_accessors {
    ($ty:ident) => {
        impl $ty {
            /// The `requestId` sent with this subscription. Pass it to
            /// `unsubscribe` to cancel.
            pub fn request_id(&self) -> &str {
                &self.0.request_id
            }

            pub fn subscription_type(&self) -> &SubscriptionType {
                &self.0.subscription_type
            }

            pub fn market_slugs(&self) -> &[String] {
                &self.0.market_slugs
            }

            /// Override the generated `requestId`.
            pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
                self.0.request_id = request_id.into();
                self
            }

            /// Replace the market slugs this subscription covers.
            pub fn with_market_slugs<I, S>(mut self, slugs: I) -> Self
            where
                I: IntoIterator<Item = S>,
                S: Into<String>,
            {
                self.0.market_slugs = slugs.into_iter().map(Into::into).collect();
                self
            }

            /// Add one more market slug.
            pub fn add_market_slug(mut self, slug: impl Into<String>) -> Self {
                self.0.market_slugs.push(slug.into());
                self
            }

            /// Add an arbitrary field to the `subscribe` object.
            ///
            /// Reach for this only when the server documents the field: the
            /// endpoint rejects a frame it cannot parse, and an unknown field
            /// may be enough to make it unparseable.
            pub fn insert_extra(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
                self.0.extra.insert(key.into(), value.into());
                self
            }

            /// The exact JSON frame this subscription puts on the socket.
            /// Useful when debugging a rejected subscription.
            pub fn frame(&self) -> Value {
                self.0.frame()
            }
        }
    };
}

/// A subscription valid on the markets endpoint.
#[derive(Debug, Clone)]
pub struct MarketSubscription(Subscription);

impl MarketSubscription {
    /// Full order-book depth for one or more markets.
    pub fn market_data<I, S>(market_slugs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(Subscription::new(SubscriptionType::MarketData)).with_market_slugs(market_slugs)
    }

    /// Best-bid/offer for one or more markets.
    pub fn market_data_lite<I, S>(market_slugs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(Subscription::new(SubscriptionType::MarketDataLite)).with_market_slugs(market_slugs)
    }

    /// Trade executions for one or more markets.
    pub fn trades<I, S>(market_slugs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(Subscription::new(SubscriptionType::Trade)).with_market_slugs(market_slugs)
    }

    /// A markets subscription type this SDK does not model yet. The string is
    /// sent verbatim, so pass it in wire form (`SUBSCRIPTION_TYPE_...`).
    ///
    /// A string that names a type the SDK *does* know is resolved to that
    /// variant, so endpoint validation still applies and this cannot be used
    /// to smuggle a private subscription onto the markets socket.
    pub fn custom(subscription_type: impl Into<String>) -> Self {
        Self(Subscription::new(SubscriptionType::from_wire(
            &subscription_type.into(),
        )))
    }
}

/// A subscription valid on the private endpoint.
#[derive(Debug, Clone)]
pub struct PrivateSubscription(Subscription);

impl PrivateSubscription {
    /// Order lifecycle events. Restrict to specific markets with
    /// [`PrivateSubscription::with_market_slugs`].
    pub fn orders() -> Self {
        Self(Subscription::new(SubscriptionType::Order))
    }

    /// Position changes.
    pub fn positions() -> Self {
        Self(Subscription::new(SubscriptionType::Position))
    }

    /// Account balance changes.
    pub fn account_balances() -> Self {
        Self(Subscription::new(SubscriptionType::AccountBalance))
    }

    /// A private subscription type this SDK does not model yet. The string is
    /// sent verbatim, so pass it in wire form (`SUBSCRIPTION_TYPE_...`).
    ///
    /// As with [`MarketSubscription::custom`], a known type string resolves to
    /// its variant and stays subject to endpoint validation.
    pub fn custom(subscription_type: impl Into<String>) -> Self {
        Self(Subscription::new(SubscriptionType::from_wire(
            &subscription_type.into(),
        )))
    }
}

subscription_accessors!(MarketSubscription);
subscription_accessors!(PrivateSubscription);

// ---------------------------------------------------------------------------
// Clients
// ---------------------------------------------------------------------------

/// Client for the market-data socket (`/v1/ws/markets`).
#[derive(Clone)]
pub struct MarketStreamClient {
    base_url: String,
    auth: Option<UsAuth>,
}

/// Client for the private account socket (`/v1/ws/private`).
///
/// Credentials are mandatory — the endpoint rejects an unauthenticated
/// upgrade — so they are taken by value rather than as an `Option`.
#[derive(Clone)]
pub struct PrivateStreamClient {
    base_url: String,
    auth: UsAuth,
}

impl MarketStreamClient {
    /// Connect to the default markets endpoint.
    ///
    /// Auth is optional here so the same client can be used before credentials
    /// are configured, but note that the live endpoint has been observed to
    /// reject unauthenticated upgrades with 401.
    pub fn new(auth: Option<UsAuth>) -> Self {
        Self {
            base_url: StreamEndpoint::Markets.default_url(),
            auth,
        }
    }

    /// Point the client at a non-default host or path — a staging venue, or a
    /// local server in tests.
    ///
    /// A URL with no path gets the endpoint's default path appended; a URL that
    /// already has one is used as given.
    pub fn with_base_url(base_url: impl Into<String>, auth: Option<UsAuth>) -> Self {
        Self {
            base_url: normalize_stream_url(base_url.into(), StreamEndpoint::Markets),
            auth,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn connect(
        &self,
        subscriptions: Vec<MarketSubscription>,
    ) -> Result<MarketStream, PolymarketUsError> {
        self.connect_with_config(subscriptions, StreamConnectConfig::default())
            .await
    }

    pub async fn connect_with_config(
        &self,
        subscriptions: Vec<MarketSubscription>,
        config: StreamConnectConfig,
    ) -> Result<MarketStream, PolymarketUsError> {
        let inner = spawn_stream(
            self.base_url.clone(),
            self.auth.clone(),
            StreamEndpoint::Markets,
            subscriptions.into_iter().map(|sub| sub.0).collect(),
            config,
        )?;
        Ok(MarketStream { inner })
    }

    /// Connect and drive a callback until the stream closes.
    pub async fn run<F, Fut>(
        &self,
        subscriptions: Vec<MarketSubscription>,
        config: StreamConnectConfig,
        mut on_message: F,
    ) -> Result<(), PolymarketUsError>
    where
        F: FnMut(StreamMessage) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut stream = self.connect_with_config(subscriptions, config).await?;
        while let Some(message) = stream.next().await {
            on_message(message).await;
        }
        Ok(())
    }
}

impl PrivateStreamClient {
    /// Connect to the default private endpoint.
    pub fn new(auth: UsAuth) -> Self {
        Self {
            base_url: StreamEndpoint::Private.default_url(),
            auth,
        }
    }

    /// Point the client at a non-default host or path. See
    /// [`MarketStreamClient::with_base_url`].
    pub fn with_base_url(base_url: impl Into<String>, auth: UsAuth) -> Self {
        Self {
            base_url: normalize_stream_url(base_url.into(), StreamEndpoint::Private),
            auth,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn connect(
        &self,
        subscriptions: Vec<PrivateSubscription>,
    ) -> Result<PrivateStream, PolymarketUsError> {
        self.connect_with_config(subscriptions, StreamConnectConfig::default())
            .await
    }

    pub async fn connect_with_config(
        &self,
        subscriptions: Vec<PrivateSubscription>,
        config: StreamConnectConfig,
    ) -> Result<PrivateStream, PolymarketUsError> {
        let inner = spawn_stream(
            self.base_url.clone(),
            Some(self.auth.clone()),
            StreamEndpoint::Private,
            subscriptions.into_iter().map(|sub| sub.0).collect(),
            config,
        )?;
        Ok(PrivateStream { inner })
    }

    /// Connect and drive a callback until the stream closes.
    pub async fn run<F, Fut>(
        &self,
        subscriptions: Vec<PrivateSubscription>,
        config: StreamConnectConfig,
        mut on_message: F,
    ) -> Result<(), PolymarketUsError>
    where
        F: FnMut(StreamMessage) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut stream = self.connect_with_config(subscriptions, config).await?;
        while let Some(message) = stream.next().await {
            on_message(message).await;
        }
        Ok(())
    }
}

fn spawn_stream(
    base_url: String,
    auth: Option<UsAuth>,
    endpoint: StreamEndpoint,
    subscriptions: Vec<Subscription>,
    config: StreamConnectConfig,
) -> Result<StreamHandle, PolymarketUsError> {
    if subscriptions.is_empty() {
        return Err(PolymarketUsError::InvalidStreamConfig(
            "at least one subscription is required".to_string(),
        ));
    }

    for subscription in &subscriptions {
        subscription.validate(endpoint)?;
    }

    let (tx, rx) = mpsc::channel(256);
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let shutdown = Arc::new(StreamShutdown::new());
    let shutdown_task = shutdown.clone();

    tokio::spawn(async move {
        let runner = StreamRunner {
            base_url,
            auth,
            subscriptions,
            config,
            tx,
            shutdown: shutdown_task,
            cmd_rx,
        };
        runner.run().await;
    });

    Ok(StreamHandle {
        receiver: rx,
        shutdown,
        cmd_tx,
        endpoint,
    })
}

// ---------------------------------------------------------------------------
// Stream handles
// ---------------------------------------------------------------------------

struct StreamHandle {
    receiver: mpsc::Receiver<StreamMessage>,
    shutdown: Arc<StreamShutdown>,
    cmd_tx: mpsc::Sender<StreamCommand>,
    endpoint: StreamEndpoint,
}

impl StreamHandle {
    async fn subscribe(&self, subscription: Subscription) -> Result<(), PolymarketUsError> {
        subscription.validate(self.endpoint)?;
        self.cmd_tx
            .send(StreamCommand::Subscribe(subscription))
            .await
            .map_err(|_| PolymarketUsError::InvalidStreamConfig("stream is closed".to_string()))
    }
}

/// Shared surface of both handles. `subscribe` is defined per handle so the
/// two sockets cannot be crossed.
macro_rules! stream_handle {
    ($ty:ident, $sub:ident, $endpoint:expr) => {
        impl $ty {
            /// Await the next message, or `None` once the stream has closed.
            pub async fn next(&mut self) -> Option<StreamMessage> {
                self.inner.receiver.recv().await
            }

            /// Which socket this handle is attached to.
            pub fn endpoint(&self) -> StreamEndpoint {
                $endpoint
            }

            /// Close the connection and stop reconnecting.
            pub fn shutdown(&self) {
                self.inner.shutdown.shutdown();
            }

            pub fn is_shutdown(&self) -> bool {
                self.inner.shutdown.is_shutdown()
            }

            /// Add a subscription to the live connection.
            ///
            /// The frame is sent immediately and replayed after every
            /// reconnect.
            pub async fn subscribe(&self, subscription: $sub) -> Result<(), PolymarketUsError> {
                self.inner.subscribe(subscription.0).await
            }

            /// Cancel a subscription by the `requestId` it was created with.
            ///
            /// The subscription is dropped from the reconnect list immediately
            /// and an `unsubscribe` frame is sent over the live connection.
            pub async fn unsubscribe(&self, request_id: &str) -> Result<(), PolymarketUsError> {
                self.inner
                    .cmd_tx
                    .send(StreamCommand::Unsubscribe(request_id.to_string()))
                    .await
                    .map_err(|_| {
                        PolymarketUsError::InvalidStreamConfig("stream is closed".to_string())
                    })
            }
        }
    };
}

/// A live connection to the markets endpoint.
pub struct MarketStream {
    inner: StreamHandle,
}

/// A live connection to the private endpoint.
pub struct PrivateStream {
    inner: StreamHandle,
}

stream_handle!(MarketStream, MarketSubscription, StreamEndpoint::Markets);
stream_handle!(PrivateStream, PrivateSubscription, StreamEndpoint::Private);

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

enum StreamCommand {
    Subscribe(Subscription),
    Unsubscribe(String), // request_id
}

#[derive(Debug, Clone)]
pub struct StreamConnectConfig {
    /// Identifies this connection in [`StreamControlEvent`]s. Never sent to
    /// the server — each subscription carries its own `requestId`.
    pub session_id: String,
    pub reconnect: ReconnectConfig,

    /// Tear down and reconnect if no frame arrives from the server within this
    /// window. Defaults to 60 seconds; `None` disables the check.
    ///
    /// Without this, a TCP connection that dies silently (no FIN, no RST — the
    /// common case behind NAT timeouts and load-balancer drops) leaves the
    /// stream blocked forever and reconnect never fires.
    pub idle_timeout: Option<Duration>,

    /// Send a WebSocket ping this often. Defaults to 20 seconds; `None`
    /// disables it.
    ///
    /// The pong it draws counts as traffic for `idle_timeout`, so a market
    /// that simply has nothing to report is not mistaken for a dead socket.
    pub keepalive_interval: Option<Duration>,
}

impl Default for StreamConnectConfig {
    fn default() -> Self {
        Self {
            session_id: next_request_id("session"),
            reconnect: ReconnectConfig::default(),
            idle_timeout: Some(Duration::from_secs(60)),
            keepalive_interval: Some(Duration::from_secs(20)),
        }
    }
}

impl StreamConnectConfig {
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    pub fn with_reconnect(mut self, reconnect: ReconnectConfig) -> Self {
        self.reconnect = reconnect;
        self
    }

    /// Set the idle timeout. Pass `None` to disable dead-connection detection.
    pub fn with_idle_timeout(mut self, idle_timeout: Option<Duration>) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    /// Set the keepalive ping interval. Pass `None` to stop sending pings.
    pub fn with_keepalive_interval(mut self, keepalive_interval: Option<Duration>) -> Self {
        self.keepalive_interval = keepalive_interval;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    pub enabled: bool,
    pub max_attempts: Option<usize>,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: None,
            initial_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

impl ReconnectConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt == 0 {
            return self.initial_delay.min(self.max_delay);
        }

        let scaled = self
            .initial_delay
            .mul_f64(self.multiplier.powi(attempt.saturating_sub(1) as i32));
        scaled.min(self.max_delay)
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StreamMessage {
    /// The `requestId` the server echoed, when it echoed one. Matches the
    /// `request_id` of the subscription that produced this message.
    pub request_id: Option<String>,
    pub kind: StreamMessageKind,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamMessageKind {
    Data(StreamDataEvent),
    Control(StreamControlEvent),
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamDataEvent {
    /// Initial snapshot of all open orders (private endpoint).
    OrderSnapshot(Value),
    /// Order lifecycle update (private endpoint).
    OrderUpdate(Value),
    /// Full order-book depth update.
    MarketData(Value),
    /// Best-bid/offer update (lightweight).
    MarketDataLite(Value),
    /// Order-book delta / incremental update.
    OrderBookDelta(Value),
    /// Initial snapshot of all portfolio positions (private endpoint).
    PositionSnapshot(Value),
    /// Position change (private endpoint).
    PositionUpdate(Value),
    /// Initial snapshot of account balances (private endpoint).
    BalanceSnapshot(Value),
    /// Account balance change (private endpoint).
    BalanceUpdate(Value),
    /// Trade execution event.
    Trade(Value),
    /// Server heartbeat — no payload.
    Heartbeat,
    /// Any server event not yet modelled by this SDK.
    Other { event_type: String, payload: Value },
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamControlEvent {
    Connected { session_id: String },
    SubscriptionAck { event_type: String, payload: Value },
    Reconnecting { attempt: usize, delay_ms: u64 },
    Closed,
    Error(String),
}

impl StreamMessage {
    pub fn control(request_id: Option<String>, event: StreamControlEvent) -> Self {
        Self {
            request_id,
            kind: StreamMessageKind::Control(event),
        }
    }

    pub fn data(request_id: Option<String>, event: StreamDataEvent) -> Self {
        Self {
            request_id,
            kind: StreamMessageKind::Data(event),
        }
    }
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

struct StreamRunner {
    base_url: String,
    auth: Option<UsAuth>,
    subscriptions: Vec<Subscription>,
    config: StreamConnectConfig,
    tx: mpsc::Sender<StreamMessage>,
    shutdown: Arc<StreamShutdown>,
    cmd_rx: mpsc::Receiver<StreamCommand>,
}

impl StreamRunner {
    async fn run(mut self) {
        let mut attempt = 0usize;

        loop {
            if self.shutdown.is_shutdown() || self.tx.is_closed() {
                break;
            }

            match self.connect_and_consume().await {
                Ok(()) => {
                    if !self.config.reconnect.enabled {
                        break;
                    }
                }
                Err(err) => {
                    if !self
                        .emit(StreamMessage::control(
                            Some(self.config.session_id.clone()),
                            StreamControlEvent::Error(err.to_string()),
                        ))
                        .await
                    {
                        break;
                    }
                }
            }

            if !self.config.reconnect.enabled {
                break;
            }

            attempt += 1;
            if let Some(max_attempts) = self.config.reconnect.max_attempts {
                if attempt > max_attempts {
                    break;
                }
            }

            let delay = self.config.reconnect.delay_for_attempt(attempt);
            if !self
                .emit(StreamMessage::control(
                    Some(self.config.session_id.clone()),
                    StreamControlEvent::Reconnecting {
                        attempt,
                        delay_ms: delay.as_millis() as u64,
                    },
                ))
                .await
            {
                break;
            }

            let shutdown = Arc::clone(&self.shutdown);
            tokio::select! {
                _ = shutdown.notified() => break,
                _ = tokio::time::sleep(delay) => {}
            }
        }

        let _ = self
            .emit(StreamMessage::control(
                Some(self.config.session_id.clone()),
                StreamControlEvent::Closed,
            ))
            .await;
    }

    async fn connect_and_consume(&mut self) -> Result<(), PolymarketUsError> {
        let mut request = self
            .base_url
            .as_str()
            .into_client_request()
            .map_err(|err| {
                PolymarketUsError::InvalidStreamConfig(format!(
                    "invalid websocket URL {}: {err}",
                    self.base_url
                ))
            })?;

        if let Some(auth) = &self.auth {
            let path = request
                .uri()
                .path_and_query()
                .map(|path| path.as_str())
                .unwrap_or("/");
            for (name, value) in auth.signed_headers("GET", path) {
                let header_value = HeaderValue::from_str(&value).map_err(|err| {
                    PolymarketUsError::InvalidStreamConfig(format!(
                        "invalid websocket auth header value for {name}: {err}"
                    ))
                })?;
                request.headers_mut().insert(name, header_value);
            }
        }

        let (mut websocket, _) = connect_async(request).await?;
        let _ = self
            .emit(StreamMessage::control(
                Some(self.config.session_id.clone()),
                StreamControlEvent::Connected {
                    session_id: self.config.session_id.clone(),
                },
            ))
            .await;

        self.send_all_subscriptions(&mut websocket).await?;

        // Clone the Arc so the future borrows it, not &mut self, allowing
        // cmd_rx to be used in the same select! block.
        let shutdown = Arc::clone(&self.shutdown);
        let shutdown_wait = shutdown.notified();
        tokio::pin!(shutdown_wait);

        // Dead-connection detection. The sleep is armed unconditionally so the
        // select! arm has something to poll, but is only ever selected when an
        // idle timeout is configured; the fallback duration is never reached.
        let idle_timeout = self.config.idle_timeout;
        let idle_deadline =
            tokio::time::sleep(idle_timeout.unwrap_or_else(|| Duration::from_secs(3600)));
        tokio::pin!(idle_deadline);

        // Keepalive pings, armed the same way. The pong is what actually feeds
        // the idle check on a market that has nothing to report.
        let keepalive_interval = self.config.keepalive_interval;
        let keepalive =
            tokio::time::sleep(keepalive_interval.unwrap_or_else(|| Duration::from_secs(3600)));
        tokio::pin!(keepalive);

        loop {
            tokio::select! {
                _ = &mut shutdown_wait => {
                    let _ = websocket.close(None).await;
                    break;
                }
                _ = &mut idle_deadline, if idle_timeout.is_some() => {
                    // Returning Err lets the outer run loop surface the reason
                    // and then reconnect under the usual backoff policy.
                    let _ = websocket.close(None).await;
                    return Err(PolymarketUsError::StreamIdle(
                        idle_timeout.expect("guarded by idle_timeout.is_some()"),
                    ));
                }
                _ = &mut keepalive, if keepalive_interval.is_some() => {
                    let interval = keepalive_interval.expect("guarded by is_some()");
                    keepalive.as_mut().reset(tokio::time::Instant::now() + interval);
                    websocket.send(Message::Ping(Vec::new().into())).await?;
                }
                message = websocket.next() => {
                    // Any frame — including a ping or pong — proves liveness.
                    if let Some(timeout) = idle_timeout {
                        idle_deadline.as_mut().reset(tokio::time::Instant::now() + timeout);
                    }

                    let Some(message) = message else {
                        break;
                    };

                    match message {
                        Ok(Message::Text(text)) => {
                            self.handle_text(&text).await?;
                        }
                        Ok(Message::Binary(bytes)) => {
                            let text = String::from_utf8(bytes.to_vec()).map_err(|err| {
                                PolymarketUsError::InvalidStreamConfig(format!(
                                    "received non-UTF8 websocket payload: {err}"
                                ))
                            })?;
                            self.handle_text(&text).await?;
                        }
                        Ok(Message::Close(_)) => break,
                        Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                        Ok(_) => {}
                        Err(err) => return Err(err.into()),
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(StreamCommand::Subscribe(sub)) => {
                            self.send_subscription(&mut websocket, &sub).await?;
                            self.subscriptions.push(sub);
                        }
                        Some(StreamCommand::Unsubscribe(request_id)) => {
                            self.subscriptions.retain(|s| s.request_id != request_id);
                            let frame = serde_json::json!({
                                "unsubscribe": { "requestId": request_id },
                            });
                            let _ = websocket
                                .send(Message::Text(frame.to_string().into()))
                                .await;
                        }
                        None => break,
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_all_subscriptions(
        &self,
        websocket: &mut WebSocket,
    ) -> Result<(), PolymarketUsError> {
        for subscription in &self.subscriptions {
            self.send_subscription(websocket, subscription).await?;
        }
        Ok(())
    }

    async fn send_subscription(
        &self,
        websocket: &mut WebSocket,
        subscription: &Subscription,
    ) -> Result<(), PolymarketUsError> {
        let payload = serde_json::to_string(&subscription.frame())?;
        websocket.send(Message::Text(payload.into())).await?;
        Ok(())
    }

    async fn handle_text(&self, text: &str) -> Result<(), PolymarketUsError> {
        let json: Value = serde_json::from_str(text)?;
        if let Some(message) = parse_stream_message(json) {
            if !self.emit(message).await {
                return Ok(());
            }
        }
        Ok(())
    }

    async fn emit(&self, message: StreamMessage) -> bool {
        self.tx.send(message).await.is_ok()
    }
}

struct StreamShutdown {
    requested: AtomicBool,
    notify: Notify,
}

impl StreamShutdown {
    fn new() -> Self {
        Self {
            requested: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    fn shutdown(&self) {
        if !self.requested.swap(true, Ordering::SeqCst) {
            self.notify.notify_waiters();
        }
    }

    fn is_shutdown(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    fn notified(&self) -> impl Future<Output = ()> + '_ {
        self.notify.notified()
    }
}

// ---------------------------------------------------------------------------
// Inbound frame parsing
// ---------------------------------------------------------------------------

/// Keys that describe a frame rather than carry its content. Used to spot the
/// payload key in an envelope like `{"requestId": "...", "marketData": {...}}`.
const ENVELOPE_META_KEYS: &[&str] = &[
    "requestId",
    "request_id",
    "trackingId",
    "tracking_id",
    "id",
    "timestamp",
    "ts",
    "time",
    "seq",
    "sequence",
    "type",
    "event",
    "channel",
    "topic",
    "name",
    "subscriptionType",
    "subscription_type",
];

fn parse_stream_message(json: Value) -> Option<StreamMessage> {
    match json {
        Value::Object(map) => {
            let request_id = extract_request_id(&map);
            let event_type = extract_event_type(&map);
            let payload = extract_payload(&map);

            let kind = match event_type.as_str() {
                // --- Order events ---
                "order_snapshot" | "orderSnapshot" => {
                    StreamMessageKind::Data(StreamDataEvent::OrderSnapshot(payload))
                }
                "order" | "orders" | "order_update" | "order_updates" | "orderUpdate"
                | "user_order" | "fill" => {
                    StreamMessageKind::Data(StreamDataEvent::OrderUpdate(payload))
                }
                // --- Market events ---
                "market_data" | "marketData" => {
                    StreamMessageKind::Data(StreamDataEvent::MarketData(payload))
                }
                "market_data_lite" | "marketDataLite" => {
                    StreamMessageKind::Data(StreamDataEvent::MarketDataLite(payload))
                }
                "order_book_delta" | "orderbook_delta" | "book_delta" | "bookDelta" => {
                    StreamMessageKind::Data(StreamDataEvent::OrderBookDelta(payload))
                }
                "trade" | "trades" => StreamMessageKind::Data(StreamDataEvent::Trade(payload)),
                // --- Position events ---
                "position_snapshot" | "positionSnapshot" => {
                    StreamMessageKind::Data(StreamDataEvent::PositionSnapshot(payload))
                }
                "position" | "positions" | "position_update" | "positionUpdate" => {
                    StreamMessageKind::Data(StreamDataEvent::PositionUpdate(payload))
                }
                // --- Balance events ---
                "balance_snapshot" | "balanceSnapshot" | "account_balance_snapshot" => {
                    StreamMessageKind::Data(StreamDataEvent::BalanceSnapshot(payload))
                }
                "balance" | "balances" | "balance_update" | "balanceUpdate" | "account_balance"
                | "accountBalance" => {
                    StreamMessageKind::Data(StreamDataEvent::BalanceUpdate(payload))
                }
                // --- Keepalive ---
                "heartbeat" | "ping" | "pong" => {
                    StreamMessageKind::Data(StreamDataEvent::Heartbeat)
                }
                // --- Control ---
                "subscription" | "subscribe" | "subscribed" | "subscribeAck" | "ack"
                | "unsubscribe" | "unsubscribed" => {
                    StreamMessageKind::Control(StreamControlEvent::SubscriptionAck {
                        event_type: event_type.clone(),
                        payload,
                    })
                }
                "error" => {
                    StreamMessageKind::Control(StreamControlEvent::Error(payload.to_string()))
                }
                _ => StreamMessageKind::Data(StreamDataEvent::Other {
                    event_type: event_type.clone(),
                    payload,
                }),
            };

            Some(StreamMessage { request_id, kind })
        }
        other => Some(StreamMessage::data(
            None,
            StreamDataEvent::Other {
                event_type: "unknown".to_string(),
                payload: other,
            },
        )),
    }
}

fn extract_request_id(map: &Map<String, Value>) -> Option<String> {
    ["requestId", "request_id", "trackingId", "tracking_id", "id"]
        .iter()
        .find_map(|key| map.get(*key).and_then(Value::as_str).map(ToOwned::to_owned))
}

/// Keys that wrap a payload without naming it, so a frame shaped like
/// `{"subscriptionType": "...", "data": {...}}` is identified by its type
/// rather than by the word "data".
const PAYLOAD_KEYS: &[&str] = &["data", "payload", "body", "message", "result"];

/// The single key that isn't envelope metadata, if there is exactly one. This
/// is what identifies a frame shaped like `{"requestId": "x", "trade": {...}}`.
fn sole_content_key(map: &Map<String, Value>) -> Option<&String> {
    let mut content = map
        .keys()
        .filter(|key| !ENVELOPE_META_KEYS.contains(&key.as_str()));
    let first = content.next()?;
    content.next().is_none().then_some(first)
}

fn extract_event_type(map: &Map<String, Value>) -> String {
    for key in ["event", "type", "channel", "name", "topic"] {
        if let Some(value) = map.get(key).and_then(Value::as_str) {
            return normalize_event_type(value);
        }
    }

    if let Some(value) = map.get("subscriptionType").and_then(Value::as_str) {
        return normalize_event_type(value);
    }

    // Only a key that actually names the event, never a generic wrapper.
    if let Some(key) = sole_content_key(map).filter(|key| !PAYLOAD_KEYS.contains(&key.as_str())) {
        return normalize_event_type(key);
    }

    "unknown".to_string()
}

/// Fold the `SUBSCRIPTION_TYPE_MARKET_DATA` spelling onto the plain
/// `market_data` name the match arms use, so both forms land on one variant.
fn normalize_event_type(raw: &str) -> String {
    match raw.strip_prefix("SUBSCRIPTION_TYPE_") {
        Some(rest) => rest.to_ascii_lowercase(),
        None => raw.to_string(),
    }
}

fn extract_payload(map: &Map<String, Value>) -> Value {
    for key in PAYLOAD_KEYS {
        if let Some(value) = map.get(*key) {
            return value.clone();
        }
    }

    if let Some(key) = sole_content_key(map) {
        return map.get(key).cloned().unwrap_or(Value::Null);
    }

    Value::Object(map.clone())
}

// ---------------------------------------------------------------------------
// URLs and ids
// ---------------------------------------------------------------------------

fn next_request_id(prefix: &str) -> String {
    let ordinal = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{ordinal}", unix_timestamp_millis())
}

/// Turn a caller-supplied base URL into a WebSocket URL for `endpoint`.
///
/// An `http(s)` scheme is upgraded to `ws(s)`. A URL with no path of its own
/// gets the endpoint's path appended; one that already has a path is trusted as
/// given, so a caller can point at a non-standard route.
fn normalize_stream_url(url: String, endpoint: StreamEndpoint) -> String {
    let trimmed = url.trim_end_matches('/');

    let with_scheme = if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        trimmed.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("wss://{trimmed}")
    };

    let authority_start = with_scheme
        .find("://")
        .map(|index| index + 3)
        .unwrap_or_default();
    let has_path = with_scheme[authority_start..].contains('/');

    if has_path {
        with_scheme
    } else {
        format!("{with_scheme}{}", endpoint.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reconnect_delay_caps_at_max() {
        let policy = ReconnectConfig {
            enabled: true,
            max_attempts: None,
            initial_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(1),
            multiplier: 3.0,
        };

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(250));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(250));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(750));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(1));
        assert_eq!(policy.delay_for_attempt(10), Duration::from_secs(1));
    }

    // --- Outbound wire format ---

    #[test]
    fn subscribe_frame_matches_the_documented_contract() {
        let subscription =
            MarketSubscription::market_data(["btc-100k-2025"]).with_request_id("md-sub-1");

        assert_eq!(
            subscription.frame(),
            json!({
                "subscribe": {
                    "requestId": "md-sub-1",
                    "subscriptionType": "SUBSCRIPTION_TYPE_MARKET_DATA",
                    "marketSlugs": ["btc-100k-2025"],
                }
            })
        );
    }

    #[test]
    fn subscribe_frame_carries_no_undocumented_fields() {
        let frame = MarketSubscription::trades(["btc-100k-2025"]).frame();
        let body = frame["subscribe"].as_object().expect("subscribe object");

        // A protobuf-derived server rejects a frame with fields it does not
        // define, so the default frame must carry exactly these three.
        let mut keys: Vec<&str> = body.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["marketSlugs", "requestId", "subscriptionType"]);
    }

    #[test]
    fn private_subscribe_frame_omits_empty_market_slugs() {
        let frame = PrivateSubscription::orders().with_request_id("p-1").frame();
        assert_eq!(
            frame,
            json!({
                "subscribe": {
                    "requestId": "p-1",
                    "subscriptionType": "SUBSCRIPTION_TYPE_ORDER",
                }
            })
        );
    }

    #[test]
    fn multiple_market_slugs_serialize_as_an_array() {
        let frame = MarketSubscription::market_data_lite(["a-market", "b-market"])
            .add_market_slug("c-market")
            .frame();
        assert_eq!(
            frame["subscribe"]["marketSlugs"],
            json!(["a-market", "b-market", "c-market"])
        );
    }

    #[test]
    fn extras_are_only_added_when_asked_for() {
        let frame = MarketSubscription::market_data(["x"])
            .insert_extra("bookLevels", json!(2))
            .frame();
        assert_eq!(frame["subscribe"]["bookLevels"], 2);
    }

    #[test]
    fn subscription_type_round_trips_through_the_wire_form() {
        for variant in [
            SubscriptionType::MarketData,
            SubscriptionType::MarketDataLite,
            SubscriptionType::Trade,
            SubscriptionType::Order,
            SubscriptionType::Position,
            SubscriptionType::AccountBalance,
        ] {
            assert_eq!(SubscriptionType::from_wire(variant.as_wire()), variant);
            assert!(variant.as_wire().starts_with("SUBSCRIPTION_TYPE_"));
        }

        assert_eq!(
            SubscriptionType::from_wire("SUBSCRIPTION_TYPE_FUTURE"),
            SubscriptionType::Other("SUBSCRIPTION_TYPE_FUTURE".to_string())
        );
    }

    #[test]
    fn custom_subscription_type_is_sent_verbatim() {
        let frame = MarketSubscription::custom("SUBSCRIPTION_TYPE_FUTURE")
            .with_market_slugs(["x"])
            .frame();
        assert_eq!(
            frame["subscribe"]["subscriptionType"],
            "SUBSCRIPTION_TYPE_FUTURE"
        );
    }

    // --- Endpoint routing ---

    #[test]
    fn subscription_types_know_their_endpoint() {
        assert_eq!(
            SubscriptionType::Trade.endpoint(),
            Some(StreamEndpoint::Markets)
        );
        assert_eq!(
            SubscriptionType::AccountBalance.endpoint(),
            Some(StreamEndpoint::Private)
        );
        // Unmodelled types are unroutable, so they are accepted on either.
        assert_eq!(SubscriptionType::Other("X".into()).endpoint(), None);
    }

    #[test]
    fn a_private_type_is_rejected_on_the_markets_socket() {
        // Only reachable through `custom`, since the typed constructors keep
        // the two subscription families apart at compile time.
        let smuggled = MarketSubscription::custom("SUBSCRIPTION_TYPE_ORDER");
        let err = smuggled
            .0
            .validate(StreamEndpoint::Markets)
            .expect_err("should be rejected");
        assert!(
            err.to_string().contains("private"),
            "error should name the right endpoint: {err}"
        );
    }

    #[test]
    fn market_data_without_a_slug_is_rejected_before_connecting() {
        let err = MarketSubscription::market_data(Vec::<String>::new())
            .0
            .validate(StreamEndpoint::Markets)
            .expect_err("should be rejected");
        assert!(err.to_string().contains("market slug"), "got: {err}");
    }

    #[test]
    fn private_subscriptions_need_no_slug() {
        assert!(PrivateSubscription::positions()
            .0
            .validate(StreamEndpoint::Private)
            .is_ok());
    }

    // --- Inbound frame parsing ---

    #[test]
    fn parses_a_frame_keyed_by_subscription_type() {
        let message = parse_stream_message(json!({
            "requestId": "md-sub-1",
            "subscriptionType": "SUBSCRIPTION_TYPE_MARKET_DATA",
            "data": { "bids": [1, 2], "asks": [3, 4] }
        }))
        .expect("message");

        assert_eq!(message.request_id.as_deref(), Some("md-sub-1"));
        match message.kind {
            StreamMessageKind::Data(StreamDataEvent::MarketData(payload)) => {
                assert_eq!(payload["bids"][0], 1);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn parses_a_frame_wrapped_in_a_named_envelope() {
        let message = parse_stream_message(json!({
            "requestId": "md-sub-2",
            "marketDataLite": { "bid": "0.50", "ask": "0.55" }
        }))
        .expect("message");

        assert_eq!(message.request_id.as_deref(), Some("md-sub-2"));
        match message.kind {
            StreamMessageKind::Data(StreamDataEvent::MarketDataLite(payload)) => {
                assert_eq!(payload["bid"], "0.50");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn parses_order_snapshot_event() {
        let message = parse_stream_message(json!({
            "event": "order_snapshot",
            "requestId": "abc-123",
            "data": { "orders": [] }
        }))
        .expect("message");

        assert_eq!(message.request_id.as_deref(), Some("abc-123"));
        assert!(matches!(
            message.kind,
            StreamMessageKind::Data(StreamDataEvent::OrderSnapshot(_))
        ));
    }

    #[test]
    fn parses_account_balance_event() {
        let message = parse_stream_message(json!({
            "subscriptionType": "SUBSCRIPTION_TYPE_ACCOUNT_BALANCE",
            "data": { "currency": "USD", "balance": "1000.00" }
        }))
        .expect("message");
        assert!(
            matches!(
                message.kind,
                StreamMessageKind::Data(StreamDataEvent::BalanceUpdate(_))
            ),
            "expected BalanceUpdate, got {:?}",
            message.kind
        );
    }

    #[test]
    fn parses_position_event() {
        let message = parse_stream_message(json!({
            "subscriptionType": "SUBSCRIPTION_TYPE_POSITION",
            "data": { "positions": [] }
        }))
        .expect("message");
        assert!(matches!(
            message.kind,
            StreamMessageKind::Data(StreamDataEvent::PositionUpdate(_))
        ));
    }

    #[test]
    fn parses_trade_event() {
        let message = parse_stream_message(json!({
            "event": "trade",
            "data": { "price": "0.55", "size": "100" }
        }))
        .expect("message");
        assert!(matches!(
            message.kind,
            StreamMessageKind::Data(StreamDataEvent::Trade(_))
        ));
    }

    #[test]
    fn parses_heartbeat_event() {
        let message = parse_stream_message(json!({ "event": "heartbeat" })).expect("message");
        assert!(matches!(
            message.kind,
            StreamMessageKind::Data(StreamDataEvent::Heartbeat)
        ));
    }

    #[test]
    fn parses_subscription_ack() {
        let message = parse_stream_message(json!({
            "subscribed": { "requestId": "md-sub-1" }
        }))
        .expect("message");
        assert!(matches!(
            message.kind,
            StreamMessageKind::Control(StreamControlEvent::SubscriptionAck { .. })
        ));
    }

    #[test]
    fn parses_server_error() {
        let message = parse_stream_message(json!({ "error": "invalid_message" })).expect("message");
        match message.kind {
            StreamMessageKind::Control(StreamControlEvent::Error(err)) => {
                assert!(err.contains("invalid_message"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    // --- URLs ---

    #[test]
    fn endpoints_have_the_documented_paths() {
        assert_eq!(
            StreamEndpoint::Markets.default_url(),
            "wss://api.polymarket.us/v1/ws/markets"
        );
        assert_eq!(
            StreamEndpoint::Private.default_url(),
            "wss://api.polymarket.us/v1/ws/private"
        );
    }

    #[test]
    fn clients_default_to_their_own_endpoint() {
        assert_eq!(
            MarketStreamClient::new(None).base_url(),
            "wss://api.polymarket.us/v1/ws/markets"
        );
    }

    #[test]
    fn a_host_only_base_url_gets_the_endpoint_path() {
        assert_eq!(
            normalize_stream_url(
                "https://staging.example.com".to_string(),
                StreamEndpoint::Private
            ),
            "wss://staging.example.com/v1/ws/private"
        );
        assert_eq!(
            normalize_stream_url("ws://127.0.0.1:8080".to_string(), StreamEndpoint::Markets),
            "ws://127.0.0.1:8080/v1/ws/markets"
        );
    }

    #[test]
    fn an_explicit_path_is_left_alone() {
        assert_eq!(
            normalize_stream_url(
                "wss://custom.example/socket".to_string(),
                StreamEndpoint::Markets
            ),
            "wss://custom.example/socket"
        );
    }
}
