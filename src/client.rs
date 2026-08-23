use crate::auth::UsAuth;
use crate::error::PolymarketUsError;
use crate::resources::{
    AccountClient, EventsClient, MarketsClient, OrdersClient, PortfolioClient, SearchClient,
};
use crate::retry::{is_retryable_status, RetryConfig};
use crate::stream::{MarketStreamClient, PrivateStreamClient};
use crate::types;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

const DEFAULT_GATEWAY_BASE_URL: &str = "https://gateway.polymarket.us";
const DEFAULT_API_BASE_URL: &str = "https://api.polymarket.us";
const DEFAULT_CORRELATION_ID_PREFIX: &str = "pmrs";

#[derive(Clone)]
pub struct PolymarketUsClient {
    http: reqwest::Client,
    gateway_base_url: String,
    api_base_url: String,
    auth: Option<UsAuth>,
    retry_config: RetryConfig,
    correlation_id_prefix: String,
}

pub struct PolymarketUsClientBuilder {
    gateway_base_url: String,
    api_base_url: String,
    auth: Option<UsAuth>,
    http: Option<reqwest::Client>,
    timeout: Duration,
    retry_config: RetryConfig,
    correlation_id_prefix: String,
}

impl Default for PolymarketUsClientBuilder {
    fn default() -> Self {
        Self {
            gateway_base_url: DEFAULT_GATEWAY_BASE_URL.to_string(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            auth: None,
            http: None,
            timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
            correlation_id_prefix: DEFAULT_CORRELATION_ID_PREFIX.to_string(),
        }
    }
}

impl PolymarketUsClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn gateway_base_url(mut self, url: impl Into<String>) -> Self {
        self.gateway_base_url = url.into();
        self
    }

    pub fn api_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base_url = url.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn auth(mut self, auth: UsAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Set the retry policy. Applies only to idempotent methods (GET, DELETE).
    ///
    /// Use [`RetryConfig::none()`] to disable retries entirely.
    pub fn retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set a prefix for the `X-Correlation-ID` header sent with every request.
    ///
    /// The full header value is `{prefix}-{uuid_v4}`. Defaults to `"pmrs"`.
    /// Useful for filtering SDK requests in Polymarket support logs.
    pub fn correlation_id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.correlation_id_prefix = prefix.into();
        self
    }

    pub fn build(self) -> Result<PolymarketUsClient, PolymarketUsError> {
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder().timeout(self.timeout).build()?,
        };
        Ok(PolymarketUsClient {
            http,
            gateway_base_url: self.gateway_base_url,
            api_base_url: self.api_base_url,
            auth: self.auth,
            retry_config: self.retry_config,
            correlation_id_prefix: self.correlation_id_prefix,
        })
    }
}

impl PolymarketUsClient {
    pub fn builder() -> PolymarketUsClientBuilder {
        PolymarketUsClientBuilder::new()
    }

    pub fn with_reqwest(http: reqwest::Client, auth: Option<UsAuth>) -> Self {
        Self {
            http,
            gateway_base_url: DEFAULT_GATEWAY_BASE_URL.to_string(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            auth,
            retry_config: RetryConfig::default(),
            correlation_id_prefix: DEFAULT_CORRELATION_ID_PREFIX.to_string(),
        }
    }

    pub fn auth(&self) -> Option<&UsAuth> {
        self.auth.as_ref()
    }

    pub fn api_base_url(&self) -> &str {
        &self.api_base_url
    }

    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }

    /// The prefix prepended to the `X-Correlation-ID` header of every request.
    pub fn correlation_id_prefix(&self) -> &str {
        &self.correlation_id_prefix
    }

    pub fn gateway_base_url(&self) -> &str {
        &self.gateway_base_url
    }

    // ========================================================================
    // Resource Access
    // ========================================================================

    /// Access markets resource (discovery, order book, pricing)
    pub fn markets(&self) -> MarketsClient<'_> {
        MarketsClient::new(self)
    }

    /// Access events resource
    pub fn events(&self) -> EventsClient<'_> {
        EventsClient::new(self)
    }

    /// Access orders resource (lifecycle management)
    pub fn orders(&self) -> OrdersClient<'_> {
        OrdersClient::new(self)
    }

    /// Access account resource (balances, buying power)
    pub fn account(&self) -> AccountClient<'_> {
        AccountClient::new(self)
    }

    /// Access portfolio resource (positions, activity)
    pub fn portfolio(&self) -> PortfolioClient<'_> {
        PortfolioClient::new(self)
    }

    /// Access search resource (full-text search)
    pub fn search(&self) -> SearchClient<'_> {
        SearchClient::new(self)
    }

    /// Build a client for the market-data socket, inheriting this client's
    /// credentials.
    ///
    /// The WebSocket endpoints live on the API host, not the gateway host used
    /// for public REST traffic, so this does not derive its URL from
    /// `gateway_base_url`. The returned client is independent of `self` and can
    /// outlive it.
    ///
    /// ```no_run
    /// # use polymarket_us::{PolymarketUsClient, MarketSubscription};
    /// # async fn run() -> Result<(), polymarket_us::PolymarketUsError> {
    /// let client = PolymarketUsClient::builder().build()?;
    /// let mut stream = client
    ///     .market_stream()
    ///     .connect(vec![MarketSubscription::market_data_lite(["btc-100k-2025"])])
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn market_stream(&self) -> MarketStreamClient {
        MarketStreamClient::new(self.auth.clone())
    }

    /// Build a client for the private account socket (orders, positions,
    /// balances).
    ///
    /// Fails with [`PolymarketUsError::MissingAuth`] if this client has no
    /// credentials — the private endpoint rejects an unauthenticated upgrade,
    /// so there is nothing useful to hand back.
    ///
    /// ```no_run
    /// # use polymarket_us::{PolymarketUsClient, PrivateSubscription, UsAuth};
    /// # async fn run() -> Result<(), polymarket_us::PolymarketUsError> {
    /// let client = PolymarketUsClient::builder().auth(UsAuth::from_env()?).build()?;
    /// let mut stream = client
    ///     .private_stream()?
    ///     .connect(vec![PrivateSubscription::orders()])
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn private_stream(&self) -> Result<PrivateStreamClient, PolymarketUsError> {
        let auth = self
            .auth
            .clone()
            .ok_or(PolymarketUsError::MissingAuth("/v1/ws/private"))?;
        Ok(PrivateStreamClient::new(auth))
    }

    pub async fn health(&self) -> Result<types::HealthResponse, PolymarketUsError> {
        self.internal_request::<(), (), types::HealthResponse>(
            Method::GET,
            "/v1/health",
            None,
            None,
            false,
        )
        .await
    }

    // ========================================================================
    // Internal Request Method
    // ========================================================================

    /// Execute an HTTP request with correlation ID injection, automatic retry
    /// (GET/DELETE only), and `Retry-After`-aware rate-limit handling.
    pub(crate) async fn internal_request<Q: Serialize, B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
        authenticated: bool,
    ) -> Result<T, PolymarketUsError> {
        let is_idempotent = matches!(method, Method::GET | Method::DELETE);
        let max_attempts = if is_idempotent {
            self.retry_config.max_retries + 1
        } else {
            1
        };

        let base = if authenticated {
            &self.api_base_url
        } else {
            &self.gateway_base_url
        };
        let url = format!("{}{}", base, path);

        let mut attempt = 0u32;
        loop {
            attempt += 1;

            // Fresh correlation ID per attempt so each retry is independently traceable.
            let correlation_id = format!("{}-{}", self.correlation_id_prefix, uuid::Uuid::new_v4());

            let mut rb = self
                .http
                .request(method.clone(), &url)
                .header("Content-Type", "application/json")
                .header("X-Correlation-ID", &correlation_id);

            if let Some(q) = query {
                rb = rb.query(q);
            }
            if let Some(b) = body {
                rb = rb.json(b);
            }
            if authenticated {
                let auth = self
                    .auth
                    .as_ref()
                    .ok_or(PolymarketUsError::MissingAuth("authenticated endpoint"))?;
                for (name, value) in auth.signed_headers(method.as_str(), path) {
                    rb = rb.header(name, value);
                }
            }

            // --- Send request, retry on transport errors for idempotent calls ---
            let response = match rb.send().await {
                Ok(r) => r,
                Err(e) if is_idempotent && attempt < max_attempts && is_transport_retryable(&e) => {
                    tokio::time::sleep(self.retry_config.backoff_for(attempt)).await;
                    continue;
                }
                Err(e) => return Err(PolymarketUsError::Transport(e)),
            };

            let status = response.status();

            // Parse Retry-After before consuming the response body.
            let retry_after = parse_retry_after(&response);

            let text = response.text().await?;

            if !status.is_success() {
                let message = extract_error_message(&text).unwrap_or_else(|| text.clone());

                // Surface rate-limit errors with the server's retry_after hint.
                let err = if status.as_u16() == 429 {
                    PolymarketUsError::RateLimited {
                        message,
                        retry_after,
                    }
                } else {
                    PolymarketUsError::from_status(status, message)
                };

                // Retry on retryable status codes (idempotent calls only).
                if is_idempotent && attempt < max_attempts && is_retryable_status(status.as_u16()) {
                    let delay =
                        retry_after.unwrap_or_else(|| self.retry_config.backoff_for(attempt));
                    tokio::time::sleep(delay).await;
                    continue;
                }

                return Err(err);
            }

            // An empty 2xx body (e.g. 204 No Content) is deserialized as JSON
            // `null`, which satisfies both `()` and `Option<T>`. Using `{}` here
            // would fail for any `T` that is not a struct or map.
            return if text.trim().is_empty() {
                serde_json::from_str("null").map_err(PolymarketUsError::from)
            } else {
                serde_json::from_str(&text).map_err(PolymarketUsError::from)
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a `Retry-After` header in either form permitted by RFC 9110:
/// a delay in seconds, or an absolute HTTP-date.
fn parse_retry_after(response: &reqwest::Response) -> Option<Duration> {
    let raw = response.headers().get("retry-after")?.to_str().ok()?;
    parse_retry_after_value(raw)
}

fn parse_retry_after_value(raw: &str) -> Option<Duration> {
    let raw = raw.trim();

    // Form 1: delay-seconds.
    if let Ok(secs) = raw.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }

    // Form 2: HTTP-date. Convert to a delay relative to now, clamping the past
    // to zero so a skewed clock cannot produce a negative or huge wait.
    let target = httpdate_to_unix_secs(raw)?;
    let now = crate::auth::unix_timestamp_millis() / 1000;
    Some(Duration::from_secs(target.saturating_sub(now).max(0) as u64))
}

/// Parse the IMF-fixdate form of HTTP-date, e.g.
/// `Wed, 21 Oct 2015 07:28:00 GMT`, into seconds since the Unix epoch.
///
/// This is the only form servers are required to emit by RFC 9110, and
/// implementing it here avoids taking on a date-parsing dependency.
fn httpdate_to_unix_secs(raw: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    // "Wed, 21 Oct 2015 07:28:00 GMT" -> ["Wed,", "21", "Oct", "2015", "07:28:00", "GMT"]
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() != 6 || parts[5] != "GMT" {
        return None;
    }

    let day: i64 = parts[1].parse().ok()?;
    let month = MONTHS.iter().position(|m| *m == parts[2])? as i64 + 1;
    let year: i64 = parts[3].parse().ok()?;

    let hms: Vec<&str> = parts[4].split(':').collect();
    if hms.len() != 3 {
        return None;
    }
    let (hour, minute, second): (i64, i64, i64) = (
        hms[0].parse().ok()?,
        hms[1].parse().ok()?,
        hms[2].parse().ok()?,
    );

    // Days from civil epoch (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Returns `true` for transport errors worth retrying (connect/timeout).
fn is_transport_retryable(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout()
}

fn extract_error_message(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json.get("message")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            json.get("error")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults_match_public_endpoints() {
        let client = PolymarketUsClient::builder().build().unwrap();
        assert_eq!(client.api_base_url(), "https://api.polymarket.us");
    }

    #[test]
    fn builder_retry_config_applied() {
        let client = PolymarketUsClient::builder()
            .retry(RetryConfig::none())
            .build()
            .unwrap();
        assert_eq!(client.retry_config().max_retries, 0);
    }

    #[test]
    fn builder_default_retry_is_three() {
        let client = PolymarketUsClient::builder().build().unwrap();
        assert_eq!(client.retry_config().max_retries, 3);
    }

    #[test]
    fn builder_correlation_id_prefix_applied() {
        let client = PolymarketUsClient::builder()
            .correlation_id_prefix("myapp")
            .build()
            .unwrap();
        assert_eq!(client.correlation_id_prefix(), "myapp");
    }

    #[test]
    fn market_stream_targets_the_api_host_not_the_gateway() {
        // The gateway URL is deliberately ignored: the sockets are served from
        // the API host, and deriving from the gateway pointed at a dead path.
        let client = PolymarketUsClient::builder()
            .gateway_base_url("https://gateway.example.com")
            .build()
            .unwrap();
        assert_eq!(
            client.market_stream().base_url(),
            "wss://api.polymarket.us/v1/ws/markets"
        );
    }

    #[test]
    fn private_stream_requires_credentials() {
        let client = PolymarketUsClient::builder().build().unwrap();
        assert!(matches!(
            client.private_stream(),
            Err(PolymarketUsError::MissingAuth(_))
        ));
    }

    #[test]
    fn default_correlation_id_prefix() {
        let client = PolymarketUsClient::builder().build().unwrap();
        assert_eq!(client.correlation_id_prefix(), "pmrs");
    }

    #[test]
    fn retry_after_parses_delay_seconds() {
        assert_eq!(
            parse_retry_after_value("120"),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            parse_retry_after_value("  30 "),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn retry_after_parses_http_date() {
        // A date far in the past clamps to zero rather than going negative.
        assert_eq!(
            parse_retry_after_value("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(Duration::from_secs(0))
        );
        // A date far in the future yields a positive delay.
        let future = parse_retry_after_value("Fri, 01 Jan 2100 00:00:00 GMT").unwrap();
        assert!(future > Duration::from_secs(0));
    }

    #[test]
    fn retry_after_rejects_garbage() {
        assert_eq!(parse_retry_after_value("not-a-date"), None);
        assert_eq!(parse_retry_after_value(""), None);
    }

    #[test]
    fn http_date_epoch_is_zero() {
        assert_eq!(
            httpdate_to_unix_secs("Thu, 01 Jan 1970 00:00:00 GMT"),
            Some(0)
        );
        // Known reference value.
        assert_eq!(
            httpdate_to_unix_secs("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1_445_412_480)
        );
    }

    #[test]
    fn empty_body_deserializes_to_unit() {
        // Guards the 204 No Content path in internal_request.
        serde_json::from_str::<()>("null").expect("unit from null");
        serde_json::from_str::<Option<String>>("null").expect("option from null");
    }

    #[test]
    fn with_reqwest_uses_default_retry() {
        let http = reqwest::Client::new();
        let client = PolymarketUsClient::with_reqwest(http, None);
        assert_eq!(client.retry_config().max_retries, 3);
    }
}
