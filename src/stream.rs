use crate::auth::UsAuth;
use crate::error::PolymarketUsError;
use futures_util::{SinkExt, StreamExt};
use http::HeaderValue;
use serde::{Deserialize, Serialize};
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

static TRACKING_COUNTER: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------------------
// Subscription channel enum
// ---------------------------------------------------------------------------

/// All known WebSocket subscription channels.
///
/// Pass a variant to the typed constructors on [`StreamSubscription`], or use
/// [`SubscriptionChannel::as_str`] to get the wire-format channel name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SubscriptionChannel {
    /// Initial snapshot of all open orders (private).
    OrderSnapshot,
    /// Real-time order lifecycle changes (private).
    OrderUpdate,
    /// Full order-book depth (public).
    MarketData,
    /// Best-bid/offer only (public).
    MarketDataLite,
    /// Initial snapshot of portfolio positions (private).
    PositionSnapshot,
    /// Real-time position changes (private).
    PositionUpdate,
    /// Initial snapshot of account balances (private).
    BalanceSnapshot,
    /// Real-time balance changes (private).
    BalanceUpdate,
    /// Trade execution feed (public).
    Trade,
    /// Server heartbeat — useful as an aliveness check.
    Heartbeat,
}

impl SubscriptionChannel {
    /// Returns the snake_case wire-format channel name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrderSnapshot => "order_snapshot",
            Self::OrderUpdate => "order_update",
            Self::MarketData => "market_data",
            Self::MarketDataLite => "market_data_lite",
            Self::PositionSnapshot => "position_snapshot",
            Self::PositionUpdate => "position_update",
            Self::BalanceSnapshot => "balance_snapshot",
            Self::BalanceUpdate => "balance_update",
            Self::Trade => "trade",
            Self::Heartbeat => "heartbeat",
        }
    }
}

impl std::fmt::Display for SubscriptionChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Internal command sent from ManagedStream → StreamRunner
// ---------------------------------------------------------------------------

enum StreamCommand {
    Subscribe(StreamSubscription),
    Unsubscribe(String), // tracking_id
}

#[derive(Clone)]
pub struct PolymarketUsStreamClient {
    base_url: String,
    auth: Option<UsAuth>,
}

impl PolymarketUsStreamClient {
    pub fn new(base_url: impl Into<String>, auth: Option<UsAuth>) -> Self {
        Self {
            base_url: normalize_stream_url(base_url.into()),
            auth,
        }
    }

    pub fn from_gateway_base_url(
        gateway_base_url: impl Into<String>,
        auth: Option<UsAuth>,
    ) -> Self {
        let gateway_base_url = gateway_base_url.into();
        Self::new(derive_stream_url(&gateway_base_url), auth)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn connect(
        &self,
        subscriptions: Vec<StreamSubscription>,
    ) -> Result<ManagedStream, PolymarketUsError> {
        self.connect_with_config(subscriptions, StreamConnectConfig::default())
            .await
    }

    pub async fn connect_with_config(
        &self,
        subscriptions: Vec<StreamSubscription>,
        config: StreamConnectConfig,
    ) -> Result<ManagedStream, PolymarketUsError> {
        if subscriptions.is_empty() {
            return Err(PolymarketUsError::InvalidStreamConfig(
                "at least one subscription is required".to_string(),
            ));
        }

        let (tx, rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let shutdown = Arc::new(StreamShutdown::new());
        let base_url = self.base_url.clone();
        let auth = self.auth.clone();
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

        Ok(ManagedStream {
            receiver: rx,
            shutdown,
            cmd_tx,
        })
    }

    pub async fn run<F, Fut>(
        &self,
        subscriptions: Vec<StreamSubscription>,
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

pub struct ManagedStream {
    receiver: mpsc::Receiver<StreamMessage>,
    shutdown: Arc<StreamShutdown>,
    cmd_tx: mpsc::Sender<StreamCommand>,
}

impl ManagedStream {
    pub async fn next(&mut self) -> Option<StreamMessage> {
        self.receiver.recv().await
    }

    pub fn shutdown(&self) {
        self.shutdown.shutdown();
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.is_shutdown()
    }

    /// Dynamically add a subscription to the live connection.
    ///
    /// The subscription frame is sent immediately over the existing WebSocket
    /// and re-sent automatically after every reconnect.
    pub async fn subscribe(&self, sub: StreamSubscription) -> Result<(), PolymarketUsError> {
        self.cmd_tx
            .send(StreamCommand::Subscribe(sub))
            .await
            .map_err(|_| PolymarketUsError::InvalidStreamConfig("stream is closed".to_string()))
    }

    /// Remove a subscription by its `tracking_id`.
    ///
    /// The subscription is removed from the reconnect list immediately.
    /// An unsubscribe frame is also sent to the server over the live connection.
    pub async fn unsubscribe(&self, tracking_id: &str) -> Result<(), PolymarketUsError> {
        self.cmd_tx
            .send(StreamCommand::Unsubscribe(tracking_id.to_string()))
            .await
            .map_err(|_| PolymarketUsError::InvalidStreamConfig("stream is closed".to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct StreamConnectConfig {
    pub tracking_id: String,
    pub responses_debounced: bool,
    pub reconnect: ReconnectConfig,
}

impl Default for StreamConnectConfig {
    fn default() -> Self {
        Self {
            tracking_id: next_tracking_id("session"),
            responses_debounced: false,
            reconnect: ReconnectConfig::default(),
        }
    }
}

impl StreamConnectConfig {
    pub fn with_tracking_id(mut self, tracking_id: impl Into<String>) -> Self {
        self.tracking_id = tracking_id.into();
        self
    }

    pub fn with_responses_debounced(mut self, responses_debounced: bool) -> Self {
        self.responses_debounced = responses_debounced;
        self
    }

    pub fn with_reconnect(mut self, reconnect: ReconnectConfig) -> Self {
        self.reconnect = reconnect;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscription {
    pub channel: String,
    pub tracking_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responses_debounced: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

impl StreamSubscription {
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            tracking_id: next_tracking_id("sub"),
            responses_debounced: None,
            symbol: None,
            market_id: None,
            outcome: None,
            extra: Map::new(),
        }
    }

    /// Create a subscription for the given typed channel.
    pub fn for_channel(channel: SubscriptionChannel) -> Self {
        Self::new(channel.as_str())
    }

    // --- Market (public) ---

    /// Full order-book depth updates for a market symbol.
    pub fn market_data(symbol: impl Into<String>) -> Self {
        let mut s = Self::new(SubscriptionChannel::MarketData.as_str());
        s.symbol = Some(symbol.into());
        s
    }

    /// Best-bid/offer updates for a market symbol (lightweight).
    pub fn market_data_lite(symbol: impl Into<String>) -> Self {
        let mut s = Self::new(SubscriptionChannel::MarketDataLite.as_str());
        s.symbol = Some(symbol.into());
        s
    }

    /// Trade executions for a market symbol.
    pub fn trades(symbol: impl Into<String>) -> Self {
        let mut s = Self::new(SubscriptionChannel::Trade.as_str());
        s.symbol = Some(symbol.into());
        s
    }

    /// Server heartbeat channel — useful for keepalive monitoring.
    pub fn heartbeat() -> Self {
        Self::new(SubscriptionChannel::Heartbeat.as_str())
    }

    // --- Private (authenticated) ---

    /// Initial snapshot of all open orders for a symbol.
    pub fn order_snapshot(symbol: impl Into<String>) -> Self {
        let mut s = Self::new(SubscriptionChannel::OrderSnapshot.as_str());
        s.symbol = Some(symbol.into());
        s
    }

    /// Real-time order lifecycle events.
    pub fn order_update() -> Self {
        Self::new(SubscriptionChannel::OrderUpdate.as_str())
    }

    /// Initial snapshot of all portfolio positions.
    pub fn position_snapshot() -> Self {
        Self::new(SubscriptionChannel::PositionSnapshot.as_str())
    }

    /// Real-time position changes.
    pub fn position_update() -> Self {
        Self::new(SubscriptionChannel::PositionUpdate.as_str())
    }

    /// Initial snapshot of account balances.
    pub fn balance_snapshot() -> Self {
        Self::new(SubscriptionChannel::BalanceSnapshot.as_str())
    }

    /// Real-time balance changes.
    pub fn balance_update() -> Self {
        Self::new(SubscriptionChannel::BalanceUpdate.as_str())
    }

    // --- Builder methods ---

    pub fn with_tracking_id(mut self, tracking_id: impl Into<String>) -> Self {
        self.tracking_id = tracking_id.into();
        self
    }

    pub fn with_responses_debounced(mut self, responses_debounced: bool) -> Self {
        self.responses_debounced = Some(responses_debounced);
        self
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    pub fn with_market_id(mut self, market_id: impl Into<String>) -> Self {
        self.market_id = Some(market_id.into());
        self
    }

    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = Some(outcome.into());
        self
    }

    pub fn insert_extra(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct StreamMessage {
    pub tracking_id: Option<String>,
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
    /// Initial snapshot of all open orders (private channel).
    OrderSnapshot(Value),
    /// Real-time order lifecycle update (private channel).
    OrderUpdate(Value),
    /// Full order-book depth update.
    MarketData(Value),
    /// Best-bid/offer update (lightweight).
    MarketDataLite(Value),
    /// Order-book delta / incremental update.
    OrderBookDelta(Value),
    /// Initial snapshot of all portfolio positions (private channel).
    PositionSnapshot(Value),
    /// Real-time position change (private channel).
    PositionUpdate(Value),
    /// Initial snapshot of account balances (private channel).
    BalanceSnapshot(Value),
    /// Real-time balance change (private channel).
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
    Connected { session_tracking_id: String },
    SubscriptionAck { event_type: String, payload: Value },
    Reconnecting { attempt: usize, delay_ms: u64 },
    Closed,
    Error(String),
}

impl StreamMessage {
    pub fn control(tracking_id: Option<String>, event: StreamControlEvent) -> Self {
        Self {
            tracking_id,
            kind: StreamMessageKind::Control(event),
        }
    }

    pub fn data(tracking_id: Option<String>, event: StreamDataEvent) -> Self {
        Self {
            tracking_id,
            kind: StreamMessageKind::Data(event),
        }
    }
}

struct StreamRunner {
    base_url: String,
    auth: Option<UsAuth>,
    subscriptions: Vec<StreamSubscription>,
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
                            Some(self.config.tracking_id.clone()),
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
                    Some(self.config.tracking_id.clone()),
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
                Some(self.config.tracking_id.clone()),
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
                Some(self.config.tracking_id.clone()),
                StreamControlEvent::Connected {
                    session_tracking_id: self.config.tracking_id.clone(),
                },
            ))
            .await;

        self.send_all_subscriptions(&mut websocket).await?;

        // Clone the Arc so the future borrows it, not &mut self, allowing
        // cmd_rx to be used in the same select! block.
        let shutdown = Arc::clone(&self.shutdown);
        let shutdown_wait = shutdown.notified();
        tokio::pin!(shutdown_wait);

        loop {
            tokio::select! {
                _ = &mut shutdown_wait => {
                    let _ = websocket.close(None).await;
                    break;
                }
                message = websocket.next() => {
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
                        Some(StreamCommand::Unsubscribe(tracking_id)) => {
                            self.subscriptions.retain(|s| s.tracking_id != tracking_id);
                            // Best-effort unsubscribe frame; server may not support it.
                            let frame = serde_json::json!({
                                "type": "unsubscribe",
                                "trackingId": tracking_id,
                            });
                            let _ = websocket
                                .send(Message::Text(frame.to_string()))
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
        websocket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Result<(), PolymarketUsError> {
        for subscription in &self.subscriptions {
            self.send_subscription(websocket, subscription).await?;
        }
        Ok(())
    }

    async fn send_subscription(
        &self,
        websocket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        subscription: &StreamSubscription,
    ) -> Result<(), PolymarketUsError> {
        let mut prepared = subscription.clone();
        if prepared.responses_debounced.is_none() {
            prepared.responses_debounced = Some(self.config.responses_debounced);
        }
        let payload = serde_json::to_string(&prepared)?;
        websocket.send(Message::Text(payload)).await?;
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

fn parse_stream_message(json: Value) -> Option<StreamMessage> {
    match json {
        Value::Object(map) => {
            let tracking_id = extract_tracking_id(&map);
            let event_type = extract_event_type(&map);
            let payload = extract_payload(&map);

            let kind = match event_type.as_str() {
                // --- Order channels ---
                "order_snapshot" | "orderSnapshot" => {
                    StreamMessageKind::Data(StreamDataEvent::OrderSnapshot(payload))
                }
                "order_update" | "order_updates" | "orderUpdate" | "user_order" | "fill" => {
                    StreamMessageKind::Data(StreamDataEvent::OrderUpdate(payload))
                }
                // --- Market channels ---
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
                // --- Position channels ---
                "position_snapshot" | "positionSnapshot" => {
                    StreamMessageKind::Data(StreamDataEvent::PositionSnapshot(payload))
                }
                "position_update" | "positionUpdate" => {
                    StreamMessageKind::Data(StreamDataEvent::PositionUpdate(payload))
                }
                // --- Balance channels ---
                "balance_snapshot" | "balanceSnapshot" => {
                    StreamMessageKind::Data(StreamDataEvent::BalanceSnapshot(payload))
                }
                "balance_update" | "balanceUpdate" => {
                    StreamMessageKind::Data(StreamDataEvent::BalanceUpdate(payload))
                }
                // --- Keepalive ---
                "heartbeat" | "ping" | "pong" => {
                    StreamMessageKind::Data(StreamDataEvent::Heartbeat)
                }
                // --- Control ---
                "subscription" | "subscribe" | "subscribed" | "ack" => {
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

            Some(StreamMessage { tracking_id, kind })
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

fn extract_tracking_id(map: &Map<String, Value>) -> Option<String> {
    ["trackingId", "tracking_id", "trackingID", "id"]
        .iter()
        .find_map(|key| map.get(*key).and_then(Value::as_str).map(ToOwned::to_owned))
}

fn extract_event_type(map: &Map<String, Value>) -> String {
    for key in ["event", "type", "channel", "name", "topic"] {
        if let Some(value) = map.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }

    if map.len() == 1 {
        return map
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
    }

    "unknown".to_string()
}

fn extract_payload(map: &Map<String, Value>) -> Value {
    for key in ["data", "payload", "body", "message", "result"] {
        if let Some(value) = map.get(key) {
            return value.clone();
        }
    }

    if map.len() == 1 {
        return map.values().next().cloned().unwrap_or(Value::Null);
    }

    Value::Object(map.clone())
}

fn next_tracking_id(prefix: &str) -> String {
    let ordinal = TRACKING_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{prefix}-{}-{ordinal}",
        chrono::Utc::now().timestamp_millis()
    )
}

fn normalize_stream_url(url: String) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        trimmed.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}/ws")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}/ws")
    } else {
        format!("wss://{trimmed}/ws")
    }
}

fn derive_stream_url(gateway_base_url: &str) -> String {
    let trimmed = gateway_base_url.trim_end_matches('/');
    if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        trimmed.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}/ws")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}/ws")
    } else {
        format!("wss://{trimmed}/ws")
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

    #[test]
    fn subscription_serializes_debounced_flag_and_tracking_id() {
        let subscription = StreamSubscription::order_snapshot("ABC")
            .with_tracking_id("tracking-1")
            .with_responses_debounced(true)
            .insert_extra("bookLevel", json!(2));

        let json = serde_json::to_value(subscription).unwrap();
        assert_eq!(json["channel"], "order_snapshot");
        assert_eq!(json["trackingId"], "tracking-1");
        assert_eq!(json["responsesDebounced"], true);
        assert_eq!(json["symbol"], "ABC");
        assert_eq!(json["bookLevel"], 2);
    }

    #[test]
    fn parses_order_snapshot_event() {
        let message = parse_stream_message(json!({
            "event": "order_snapshot",
            "trackingId": "abc-123",
            "data": { "bids": [1, 2], "asks": [3, 4] }
        }))
        .expect("message");

        assert_eq!(message.tracking_id.as_deref(), Some("abc-123"));
        match message.kind {
            StreamMessageKind::Data(StreamDataEvent::OrderSnapshot(payload)) => {
                assert_eq!(payload["bids"][0], 1);
                assert_eq!(payload["asks"][1], 4);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn parses_position_snapshot_event() {
        let message = parse_stream_message(json!({
            "event": "position_snapshot",
            "data": { "positions": [] }
        }))
        .expect("message");
        assert!(
            matches!(
                message.kind,
                StreamMessageKind::Data(StreamDataEvent::PositionSnapshot(_))
            ),
            "expected PositionSnapshot"
        );
    }

    #[test]
    fn parses_balance_update_event() {
        let message = parse_stream_message(json!({
            "event": "balance_update",
            "data": { "currency": "USD", "balance": "1000.00" }
        }))
        .expect("message");
        assert!(
            matches!(
                message.kind,
                StreamMessageKind::Data(StreamDataEvent::BalanceUpdate(_))
            ),
            "expected BalanceUpdate"
        );
    }

    #[test]
    fn parses_trade_event() {
        let message = parse_stream_message(json!({
            "event": "trade",
            "data": { "price": "0.55", "size": "100" }
        }))
        .expect("message");
        assert!(
            matches!(
                message.kind,
                StreamMessageKind::Data(StreamDataEvent::Trade(_))
            ),
            "expected Trade"
        );
    }

    #[test]
    fn parses_heartbeat_event() {
        let message = parse_stream_message(json!({ "event": "heartbeat" })).expect("message");
        assert!(
            matches!(
                message.kind,
                StreamMessageKind::Data(StreamDataEvent::Heartbeat)
            ),
            "expected Heartbeat"
        );
    }

    #[test]
    fn parses_market_data_lite_event() {
        let message = parse_stream_message(json!({
            "event": "market_data_lite",
            "data": { "bid": "0.50", "ask": "0.55" }
        }))
        .expect("message");
        assert!(
            matches!(
                message.kind,
                StreamMessageKind::Data(StreamDataEvent::MarketDataLite(_))
            ),
            "expected MarketDataLite"
        );
    }

    #[test]
    fn subscription_channel_as_str() {
        assert_eq!(
            SubscriptionChannel::OrderSnapshot.as_str(),
            "order_snapshot"
        );
        assert_eq!(
            SubscriptionChannel::MarketDataLite.as_str(),
            "market_data_lite"
        );
        assert_eq!(
            SubscriptionChannel::PositionUpdate.as_str(),
            "position_update"
        );
        assert_eq!(
            SubscriptionChannel::BalanceSnapshot.as_str(),
            "balance_snapshot"
        );
        assert_eq!(SubscriptionChannel::Trade.as_str(), "trade");
        assert_eq!(SubscriptionChannel::Heartbeat.as_str(), "heartbeat");
    }

    #[test]
    fn subscription_constructors_set_channel() {
        assert_eq!(StreamSubscription::market_data("X").channel, "market_data");
        assert_eq!(
            StreamSubscription::market_data_lite("X").channel,
            "market_data_lite"
        );
        assert_eq!(StreamSubscription::trades("X").channel, "trade");
        assert_eq!(StreamSubscription::heartbeat().channel, "heartbeat");
        assert_eq!(StreamSubscription::order_update().channel, "order_update");
        assert_eq!(
            StreamSubscription::position_snapshot().channel,
            "position_snapshot"
        );
        assert_eq!(
            StreamSubscription::position_update().channel,
            "position_update"
        );
        assert_eq!(
            StreamSubscription::balance_snapshot().channel,
            "balance_snapshot"
        );
        assert_eq!(
            StreamSubscription::balance_update().channel,
            "balance_update"
        );
    }

    #[test]
    fn for_channel_constructor() {
        let sub = StreamSubscription::for_channel(SubscriptionChannel::BalanceUpdate);
        assert_eq!(sub.channel, "balance_update");
    }

    #[test]
    fn derives_stream_url_from_gateway_base_url() {
        assert_eq!(
            derive_stream_url("https://gateway.polymarket.us"),
            "wss://gateway.polymarket.us/ws"
        );
        assert_eq!(
            normalize_stream_url("wss://custom.example/ws".to_string()),
            "wss://custom.example/ws"
        );
    }
}
