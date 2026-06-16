use crate::auth::UsAuth;
use crate::error::PolymarketUsError;
use crate::types;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;

const DEFAULT_GATEWAY_BASE_URL: &str = "https://gateway.polymarket.us";
const DEFAULT_API_BASE_URL: &str = "https://api.polymarket.us";

#[derive(Clone)]
pub struct PolymarketUsClient {
    http: reqwest::Client,
    gateway_base_url: String,
    api_base_url: String,
    auth: Option<UsAuth>,
}

pub struct PolymarketUsClientBuilder {
    gateway_base_url: String,
    api_base_url: String,
    auth: Option<UsAuth>,
    http: Option<reqwest::Client>,
    timeout: std::time::Duration,
}

impl Default for PolymarketUsClientBuilder {
    fn default() -> Self {
        Self {
            gateway_base_url: DEFAULT_GATEWAY_BASE_URL.to_string(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            auth: None,
            http: None,
            timeout: std::time::Duration::from_secs(30),
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

    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
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
        }
    }

    pub fn auth(&self) -> Option<&UsAuth> {
        self.auth.as_ref()
    }

    pub fn api_base_url(&self) -> &str {
        &self.api_base_url
    }

    pub async fn health(&self) -> Result<types::HealthResponse, PolymarketUsError> {
        self.request::<(), (), types::HealthResponse>(Method::GET, "/v1/health", None, None, false)
            .await
    }

    pub async fn markets_list<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.request(Method::GET, "/v1/markets", query, None::<&()>, false)
            .await
    }

    pub async fn markets_list_authenticated<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.request(Method::GET, "/v1/markets", query, None::<&()>, true)
            .await
    }

    pub async fn account_balances(&self) -> Result<types::AccountBalancesResponse, PolymarketUsError> {
        self.request::<(), (), types::AccountBalancesResponse>(
            Method::GET,
            "/v1/account/balances",
            None,
            None,
            true,
        )
        .await
    }

    pub async fn portfolio_positions(
        &self,
    ) -> Result<types::PortfolioPositionsResponse, PolymarketUsError> {
        self.request::<(), (), types::PortfolioPositionsResponse>(
            Method::GET,
            "/v1/portfolio/positions",
            None,
            None,
            true,
        )
        .await
    }

    pub async fn portfolio_activities<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::PortfolioActivitiesResponse, PolymarketUsError> {
        self.request(
            Method::GET,
            "/v1/portfolio/activities",
            query,
            None::<&()>,
            true,
        )
        .await
    }

    pub async fn place_order(
        &self,
        body: &types::PlaceOrderRequest,
    ) -> Result<types::PlaceOrderResponse, PolymarketUsError> {
        self.request(Method::POST, "/v1/trading/orders", None::<&()>, Some(body), true)
            .await
    }

    pub async fn place_batched_orders(
        &self,
        body: &types::BatchedOrderRequest,
    ) -> Result<types::BatchedOrderResponse, PolymarketUsError> {
        self.request(Method::POST, "/v1/orders/batched", None::<&()>, Some(body), true)
            .await
    }

    pub async fn cancel_trading_order(
        &self,
        order_id: &str,
    ) -> Result<types::CancelOrderResponse, PolymarketUsError> {
        self.request::<(), (), types::CancelOrderResponse>(
            Method::DELETE,
            &format!("/v1/trading/orders/{order_id}"),
            None,
            None,
            true,
        )
        .await
    }

    pub async fn orders_create(
        &self,
        body: &types::PlaceOrderRequest,
    ) -> Result<types::PlaceOrderResponse, PolymarketUsError> {
        self.request(Method::POST, "/v1/orders", None::<&()>, Some(body), true)
            .await
    }

    pub async fn orders_open<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::GetOpenOrdersResponse, PolymarketUsError> {
        self.request(Method::GET, "/v1/orders/open", query, None::<&()>, true)
            .await
    }

    pub async fn order_retrieve(
        &self,
        order_id: &str,
    ) -> Result<types::PlaceOrderResponse, PolymarketUsError> {
        self.request::<(), (), types::PlaceOrderResponse>(
            Method::GET,
            &format!("/v1/order/{order_id}"),
            None,
            None,
            true,
        )
        .await
    }

    pub async fn order_cancel(
        &self,
        order_id: &str,
        body: &types::CancelOrderParams,
    ) -> Result<(), PolymarketUsError> {
        let _: serde_json::Value = self
            .request(
                Method::POST,
                &format!("/v1/order/{order_id}/cancel"),
                None::<&()>,
                Some(body),
                true,
            )
            .await?;
        Ok(())
    }

    pub async fn order_modify(
        &self,
        order_id: &str,
        body: &types::ModifyOrderRequest,
    ) -> Result<(), PolymarketUsError> {
        let _: serde_json::Value = self
            .request(
                Method::POST,
                &format!("/v1/order/{order_id}/modify"),
                None::<&()>,
                Some(body),
                true,
            )
            .await?;
        Ok(())
    }

    pub async fn orders_cancel_all(
        &self,
        body: &types::CancelAllOrdersParams,
    ) -> Result<types::CancelAllOrdersResponse, PolymarketUsError> {
        self.request(Method::POST, "/v1/orders/open/cancel", None::<&()>, Some(body), true)
            .await
    }

    pub async fn order_preview(
        &self,
        body: &types::PreviewOrderRequest,
    ) -> Result<types::PreviewOrderResponse, PolymarketUsError> {
        self.request(Method::POST, "/v1/order/preview", None::<&()>, Some(body), true)
            .await
    }

    pub async fn order_close_position(
        &self,
        body: &types::ClosePositionRequest,
    ) -> Result<types::ClosePositionResponse, PolymarketUsError> {
        self.request(Method::POST, "/v1/order/close-position", None::<&()>, Some(body), true)
            .await
    }

    async fn request<Q: Serialize, B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
        authenticated: bool,
    ) -> Result<T, PolymarketUsError> {
        let base = if authenticated {
            &self.api_base_url
        } else {
            &self.gateway_base_url
        };
        let url = format!("{}{}", base, path);

        let mut rb = self.http.request(method.clone(), &url).header("Content-Type", "application/json");
        if let Some(query) = query {
            rb = rb.query(query);
        }
        if let Some(body) = body {
            rb = rb.json(body);
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

        let response = rb.send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            let message = extract_error_message(&text).unwrap_or_else(|| text.clone());
            return Err(PolymarketUsError::from_status(status, message));
        }

        if text.trim().is_empty() {
            serde_json::from_str("{}")
        } else {
            serde_json::from_str(&text)
        }
        .map_err(PolymarketUsError::from)
    }
}

fn extract_error_message(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json.get("message")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| json.get("error").and_then(|v| v.as_str()).map(ToOwned::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults_match_public_endpoints() {
        let client = PolymarketUsClient::builder().build().unwrap();
        assert_eq!(client.api_base_url(), "https://api.polymarket.us");
    }
}



