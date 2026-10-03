use crate::client::PolymarketUsClient;
use crate::error::PolymarketUsError;
use crate::types;
use reqwest::Method;
use serde::Serialize;

// ============================================================================
// Markets Resource
// ============================================================================

#[derive(Clone)]
pub struct MarketsClient<'a> {
    client: &'a PolymarketUsClient,
}

impl<'a> MarketsClient<'a> {
    pub fn new(client: &'a PolymarketUsClient) -> Self {
        Self { client }
    }

    /// List all markets
    pub async fn list(&self) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.list_with_query::<()>(None).await
    }

    /// List markets with query parameters
    pub async fn list_with_query<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/markets", query, None::<&()>, false)
            .await
    }

    /// List markets (authenticated)
    pub async fn list_authenticated(&self) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.list_authenticated_with_query::<()>(None).await
    }

    /// List markets with query parameters (authenticated)
    pub async fn list_authenticated_with_query<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/markets", query, None::<&()>, true)
            .await
    }

    /// Get order book for a market
    pub async fn order_book(&self, symbol: &str) -> Result<types::OrderBook, PolymarketUsError> {
        // The gateway wraps both market-data payloads in `marketData`; unwrap it
        // so callers keep receiving the book itself.
        self.client
            .internal_request::<(), (), types::MarketDataEnvelope<types::OrderBook>>(
                Method::GET,
                &format!("/v1/markets/{symbol}/book"),
                None,
                None,
                false,
            )
            .await
            .map(|envelope| envelope.market_data)
    }

    /// Get best bid/offer for a market
    pub async fn bbo(&self, symbol: &str) -> Result<types::BestBidOffer, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::MarketDataEnvelope<types::BestBidOffer>>(
                Method::GET,
                &format!("/v1/markets/{symbol}/bbo"),
                None,
                None,
                false,
            )
            .await
            .map(|envelope| envelope.market_data)
    }

    /// Get settlement price for a market
    pub async fn settlement_price(
        &self,
        symbol: &str,
    ) -> Result<types::SettlementPrice, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::SettlementPrice>(
                Method::GET,
                &format!("/v1/markets/{symbol}/settlement"),
                None,
                None,
                false,
            )
            .await
    }
}

// ============================================================================
// Events Resource
// ============================================================================

#[derive(Clone)]
pub struct EventsClient<'a> {
    client: &'a PolymarketUsClient,
}

impl<'a> EventsClient<'a> {
    pub fn new(client: &'a PolymarketUsClient) -> Self {
        Self { client }
    }

    /// List all events
    pub async fn list(&self) -> Result<types::EventsResponse, PolymarketUsError> {
        self.list_with_query::<()>(None).await
    }

    /// List events with query parameters
    pub async fn list_with_query<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::EventsResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/events", query, None::<&()>, false)
            .await
    }

    /// Get event by ID
    pub async fn retrieve(&self, event_id: &str) -> Result<types::UsEvent, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::UsEvent>(
                Method::GET,
                &format!("/v1/events/{event_id}"),
                None,
                None,
                false,
            )
            .await
    }

    /// Get event by slug
    pub async fn retrieve_by_slug(&self, slug: &str) -> Result<types::UsEvent, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::UsEvent>(
                Method::GET,
                &format!("/v1/events/by-slug/{slug}"),
                None,
                None,
                false,
            )
            .await
    }
}

// ============================================================================
// Orders Resource
// ============================================================================

#[derive(Clone)]
pub struct OrdersClient<'a> {
    client: &'a PolymarketUsClient,
}

impl<'a> OrdersClient<'a> {
    pub fn new(client: &'a PolymarketUsClient) -> Self {
        Self { client }
    }

    /// Create a new order
    pub async fn create(
        &self,
        body: &types::PlaceOrderRequest,
    ) -> Result<types::PlaceOrderResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::POST, "/v1/orders", None::<&()>, Some(body), true)
            .await
    }

    /// Place an order (alternative endpoint)
    pub async fn place(
        &self,
        body: &types::PlaceOrderRequest,
    ) -> Result<types::PlaceOrderResponse, PolymarketUsError> {
        self.client
            .internal_request(
                Method::POST,
                "/v1/trading/orders",
                None::<&()>,
                Some(body),
                true,
            )
            .await
    }

    /// Place multiple orders atomically
    pub async fn place_batch(
        &self,
        body: &types::BatchedOrderRequest,
    ) -> Result<types::BatchedOrderResponse, PolymarketUsError> {
        self.client
            .internal_request(
                Method::POST,
                "/v1/orders/batched",
                None::<&()>,
                Some(body),
                true,
            )
            .await
    }

    /// Get all open orders
    pub async fn open<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::GetOpenOrdersResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/orders/open", query, None::<&()>, true)
            .await
    }

    /// Get order by ID
    pub async fn retrieve(
        &self,
        order_id: &str,
    ) -> Result<types::PlaceOrderResponse, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::PlaceOrderResponse>(
                Method::GET,
                &format!("/v1/order/{order_id}"),
                None,
                None,
                true,
            )
            .await
    }

    /// Cancel an order
    pub async fn cancel(
        &self,
        order_id: &str,
        body: &types::CancelOrderParams,
    ) -> Result<(), PolymarketUsError> {
        let _: serde_json::Value = self
            .client
            .internal_request(
                Method::POST,
                &format!("/v1/order/{order_id}/cancel"),
                None::<&()>,
                Some(body),
                true,
            )
            .await?;
        Ok(())
    }

    /// Cancel order by trading endpoint
    pub async fn cancel_trading(
        &self,
        order_id: &str,
    ) -> Result<types::CancelOrderResponse, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::CancelOrderResponse>(
                Method::DELETE,
                &format!("/v1/trading/orders/{order_id}"),
                None,
                None,
                true,
            )
            .await
    }

    /// Cancel all open orders
    pub async fn cancel_all(
        &self,
        body: &types::CancelAllOrdersParams,
    ) -> Result<types::CancelAllOrdersResponse, PolymarketUsError> {
        self.client
            .internal_request(
                Method::POST,
                "/v1/orders/open/cancel",
                None::<&()>,
                Some(body),
                true,
            )
            .await
    }

    /// Modify an open order
    pub async fn modify(
        &self,
        order_id: &str,
        body: &types::ModifyOrderRequest,
    ) -> Result<(), PolymarketUsError> {
        let _: serde_json::Value = self
            .client
            .internal_request(
                Method::POST,
                &format!("/v1/order/{order_id}/modify"),
                None::<&()>,
                Some(body),
                true,
            )
            .await?;
        Ok(())
    }

    /// Preview an order
    pub async fn preview(
        &self,
        body: &types::PreviewOrderRequest,
    ) -> Result<types::PreviewOrderResponse, PolymarketUsError> {
        self.client
            .internal_request(
                Method::POST,
                "/v1/order/preview",
                None::<&()>,
                Some(body),
                true,
            )
            .await
    }

    /// Close a position
    pub async fn close_position(
        &self,
        body: &types::ClosePositionRequest,
    ) -> Result<types::ClosePositionResponse, PolymarketUsError> {
        self.client
            .internal_request(
                Method::POST,
                "/v1/order/close-position",
                None::<&()>,
                Some(body),
                true,
            )
            .await
    }
}

// ============================================================================
// Account Resource
// ============================================================================

#[derive(Clone)]
pub struct AccountClient<'a> {
    client: &'a PolymarketUsClient,
}

impl<'a> AccountClient<'a> {
    pub fn new(client: &'a PolymarketUsClient) -> Self {
        Self { client }
    }

    /// Get account balances
    pub async fn balances(&self) -> Result<types::AccountBalancesResponse, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::AccountBalancesResponse>(
                Method::GET,
                "/v1/account/balances",
                None,
                None,
                true,
            )
            .await
    }
}

// ============================================================================
// Portfolio Resource
// ============================================================================

#[derive(Clone)]
pub struct PortfolioClient<'a> {
    client: &'a PolymarketUsClient,
}

impl<'a> PortfolioClient<'a> {
    pub fn new(client: &'a PolymarketUsClient) -> Self {
        Self { client }
    }

    /// Get portfolio positions
    pub async fn positions(&self) -> Result<types::PortfolioPositionsResponse, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::PortfolioPositionsResponse>(
                Method::GET,
                "/v1/portfolio/positions",
                None,
                None,
                true,
            )
            .await
    }

    /// Get portfolio activities with optional query parameters
    pub async fn activities<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::PortfolioActivitiesResponse, PolymarketUsError> {
        self.client
            .internal_request(
                Method::GET,
                "/v1/portfolio/activities",
                query,
                None::<&()>,
                true,
            )
            .await
    }
}

// ============================================================================
// Search Resource
// ============================================================================

#[derive(Clone)]
pub struct SearchClient<'a> {
    client: &'a PolymarketUsClient,
}

impl<'a> SearchClient<'a> {
    pub fn new(client: &'a PolymarketUsClient) -> Self {
        Self { client }
    }

    /// Full-text search across markets and events
    pub async fn search<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::SearchResults, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/search", query, None::<&()>, false)
            .await
    }

    /// Search markets
    pub async fn markets<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::MarketsResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/search/markets", query, None::<&()>, false)
            .await
    }

    /// Search events
    pub async fn events<Q: Serialize>(
        &self,
        query: Option<&Q>,
    ) -> Result<types::EventsResponse, PolymarketUsError> {
        self.client
            .internal_request(Method::GET, "/v1/search/events", query, None::<&()>, false)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> PolymarketUsClient {
        PolymarketUsClient::builder().build().unwrap()
    }

    // ========================================================================
    // MarketsClient Tests
    // ========================================================================

    #[test]
    fn markets_client_creation() {
        let client = create_test_client();
        // Just verify it can be created; actual API calls need mocking
        let _markets = client.markets();
    }

    #[test]
    fn markets_client_has_expected_methods() {
        let client = create_test_client();
        let markets = client.markets();

        // Binding the value is the real check: it only compiles if the
        // accessor returns this type. The assertion below guards the name, and
        // deliberately ignores how the compiler renders the lifetime — that
        // rendering changed between 1.86 and current stable, and pinning the
        // exact string made these tests fail on the crate's own MSRV.
        let rendered = std::any::type_name_of_val(&markets);
        assert!(
            rendered.starts_with("polymarket_us::resources::MarketsClient"),
            "unexpected type: {rendered}"
        );
    }

    // ========================================================================
    // EventsClient Tests
    // ========================================================================

    #[test]
    fn events_client_creation() {
        let client = create_test_client();
        let _events = client.events();
    }

    #[test]
    fn events_client_type_check() {
        let client = create_test_client();
        let events = client.events();
        let rendered = std::any::type_name_of_val(&events);
        assert!(
            rendered.starts_with("polymarket_us::resources::EventsClient"),
            "unexpected type: {rendered}"
        );
    }

    // ========================================================================
    // OrdersClient Tests
    // ========================================================================

    #[test]
    fn orders_client_creation() {
        let client = create_test_client();
        let _orders = client.orders();
    }

    #[test]
    fn orders_client_type_check() {
        let client = create_test_client();
        let orders = client.orders();
        let rendered = std::any::type_name_of_val(&orders);
        assert!(
            rendered.starts_with("polymarket_us::resources::OrdersClient"),
            "unexpected type: {rendered}"
        );
    }

    #[test]
    fn place_order_request_serializes() {
        let req = types::PlaceOrderRequest {
            symbol: "BTC-USD".to_string(),
            action: types::OrderAction::Buy,
            outcome_side: types::OrderSide::Long,
            order_type: types::OrderType::Limit,
            price: types::Money {
                value: "0.50".to_string(),
                currency: "USD".to_string(),
            },
            quantity: 100,
            tif: types::TimeInForce::GoodTillCancel,
            client_order_id: Some("test-123".to_string()),
            post_only: false,
            expires_at: None,
        };

        let json = serde_json::to_string(&req).expect("should serialize");
        assert!(json.contains("BTC-USD"));
        assert!(json.contains("ORDER_ACTION_BUY"));
        assert!(json.contains("0.50"));
    }

    #[test]
    fn batched_order_request_serializes() {
        let req = types::BatchedOrderRequest {
            orders: vec![types::PlaceOrderRequest {
                symbol: "BTC-USD".to_string(),
                action: types::OrderAction::Buy,
                outcome_side: types::OrderSide::Long,
                order_type: types::OrderType::Limit,
                price: types::Money {
                    value: "0.50".to_string(),
                    currency: "USD".to_string(),
                },
                quantity: 100,
                tif: types::TimeInForce::GoodTillCancel,
                client_order_id: None,
                post_only: false,
                expires_at: None,
            }],
            atomic: true,
        };

        let json = serde_json::to_string(&req).expect("should serialize");
        assert!(json.contains("atomic"));
        assert!(json.contains("BTC-USD"));
    }

    #[test]
    fn cancel_order_params_serializes() {
        let params = types::CancelOrderParams { quantity: Some(50) };
        let json = serde_json::to_string(&params).expect("should serialize");
        assert!(json.contains("50"));
    }

    // ========================================================================
    // AccountClient Tests
    // ========================================================================

    #[test]
    fn account_client_creation() {
        let client = create_test_client();
        let _account = client.account();
    }

    #[test]
    fn account_client_type_check() {
        let client = create_test_client();
        let account = client.account();
        let rendered = std::any::type_name_of_val(&account);
        assert!(
            rendered.starts_with("polymarket_us::resources::AccountClient"),
            "unexpected type: {rendered}"
        );
    }

    // ========================================================================
    // PortfolioClient Tests
    // ========================================================================

    #[test]
    fn portfolio_client_creation() {
        let client = create_test_client();
        let _portfolio = client.portfolio();
    }

    #[test]
    fn portfolio_client_type_check() {
        let client = create_test_client();
        let portfolio = client.portfolio();
        let rendered = std::any::type_name_of_val(&portfolio);
        assert!(
            rendered.starts_with("polymarket_us::resources::PortfolioClient"),
            "unexpected type: {rendered}"
        );
    }

    // ========================================================================
    // SearchClient Tests
    // ========================================================================

    #[test]
    fn search_client_creation() {
        let client = create_test_client();
        let _search = client.search();
    }

    #[test]
    fn search_client_type_check() {
        let client = create_test_client();
        let search = client.search();
        let rendered = std::any::type_name_of_val(&search);
        assert!(
            rendered.starts_with("polymarket_us::resources::SearchClient"),
            "unexpected type: {rendered}"
        );
    }

    // ========================================================================
    // Type Serialization Tests
    // ========================================================================

    #[test]
    fn money_serializes() {
        let money = types::Money {
            value: "100.00".to_string(),
            currency: "USD".to_string(),
        };
        let json = serde_json::to_string(&money).expect("should serialize");
        assert!(json.contains("100.00"));
        assert!(json.contains("USD"));
    }

    #[test]
    fn preview_order_request_serializes() {
        let req = types::PreviewOrderRequest {
            symbol: "BTC-USD".to_string(),
            action: types::OrderAction::Sell,
            outcome_side: types::OrderSide::Short,
            order_type: types::OrderType::Limit,
            price: types::Money {
                value: "0.75".to_string(),
                currency: "USD".to_string(),
            },
            quantity: 50,
        };

        let json = serde_json::to_string(&req).expect("should serialize");
        assert!(json.contains("ORDER_ACTION_SELL"));
        assert!(json.contains("0.75"));
    }

    #[test]
    fn close_position_request_serializes() {
        let req = types::ClosePositionRequest {
            symbol: "BTC-USD".to_string(),
            quantity: 100,
        };
        let json = serde_json::to_string(&req).expect("should serialize");
        assert!(json.contains("BTC-USD"));
        assert!(json.contains("100"));
    }

    #[test]
    fn modify_order_request_serializes() {
        let req = types::ModifyOrderRequest {
            price: types::Money {
                value: "0.60".to_string(),
                currency: "USD".to_string(),
            },
            quantity: 200,
        };
        let json = serde_json::to_string(&req).expect("should serialize");
        assert!(json.contains("0.60"));
        assert!(json.contains("200"));
    }

    /// Captured from the gateway on 2026-10-02, not hand-written.
    ///
    /// The previous fixtures were invented to match the types rather than the
    /// server: `{"bids": [{"price": ..., "quantity": ...}]}` with no
    /// `marketData` envelope. They passed against the wrong types, which is
    /// precisely why `bbo()` and `order_book()` shipped returning empty books on
    /// every call. Keep these payloads verbatim; shorten them only by dropping
    /// whole fields, never by renaming one.
    const BOOK_JSON: &str = r#"{"marketData":{"marketSlug":"tsc-setkamecz-krajan-veiluk-2026-10-03-st-4pt5","bids":[{"px":{"value":"0.2300","currency":"USD"},"qty":"8.7600"}],"offers":[{"px":{"value":"0.7700","currency":"USD"},"qty":"12.0000"}],"state":"MARKET_STATE_OPEN","transactTime":"2026-10-02T20:10:00Z"}}"#;

    const BBO_JSON: &str = r#"{"marketData":{"marketSlug":"tsc-setkamecz-krajan-veiluk-2026-10-03-st-4pt5","currentPx":null,"lastTradePx":null,"bestAsk":{"value":"0.7700","currency":"USD"},"bestBid":{"value":"0.2300","currency":"USD"},"askDepth":1,"bidDepth":1,"state":"MARKET_STATE_OPEN","bidShares":"8.7600","askShares":"12.0000"}}"#;

    #[test]
    fn order_book_deserializes() {
        let env: types::MarketDataEnvelope<types::OrderBook> =
            serde_json::from_str(BOOK_JSON).expect("should deserialize");
        let book = env.market_data;
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.offers.len(), 1);
        assert_eq!(book.bids[0].px.value, "0.2300");
        assert_eq!(book.bids[0].px.currency, "USD");
        assert_eq!(book.bids[0].qty, "8.7600");
        assert_eq!(book.offers[0].px.value, "0.7700");
        assert_eq!(book.state, "MARKET_STATE_OPEN");
    }

    #[test]
    fn best_bid_offer_deserializes() {
        let env: types::MarketDataEnvelope<types::BestBidOffer> =
            serde_json::from_str(BBO_JSON).expect("should deserialize");
        let bbo = env.market_data;
        assert_eq!(
            bbo.best_bid.as_ref().map(|m| m.value.as_str()),
            Some("0.2300")
        );
        assert_eq!(
            bbo.best_ask.as_ref().map(|m| m.value.as_str()),
            Some("0.7700")
        );
        assert_eq!(bbo.bid_shares, "8.7600");
        assert_eq!(bbo.state, "MARKET_STATE_OPEN");
    }

    /// An expired market sends `null` for both sides, and that must stay `None`
    /// rather than becoming a zero price: a zero bid reads as a real price to
    /// anything pricing an exit.
    #[test]
    fn an_absent_side_stays_none() {
        let json = r#"{"marketData":{"marketSlug":"aec-nfl-lac-ten-2025-11-02","bestBid":null,"bestAsk":null,"bidShares":"0","askShares":"0","state":"MARKET_STATE_EXPIRED"}}"#;
        let env: types::MarketDataEnvelope<types::BestBidOffer> =
            serde_json::from_str(json).expect("should deserialize");
        let bbo = env.market_data;
        assert!(
            bbo.best_bid.is_none(),
            "a null side must not become a price"
        );
        assert!(bbo.best_ask.is_none());
        assert_eq!(bbo.state, "MARKET_STATE_EXPIRED");
    }

    /// The failure that shipped: the old shape must no longer silently succeed.
    ///
    /// Every field was `#[serde(default)]`, so a payload the gateway never sends
    /// deserialized into an empty book and the caller could not tell. A missing
    /// envelope is now an error instead of an empty answer.
    #[test]
    fn the_invented_shape_is_rejected_rather_than_emptied() {
        let old = r#"{"bid": {"price": "0.50", "quantity": "100"}, "ask": {"price": "0.55", "quantity": "150"}}"#;
        assert!(
            serde_json::from_str::<types::MarketDataEnvelope<types::BestBidOffer>>(old).is_err(),
            "a payload without marketData must fail, not parse as an empty book",
        );
    }

    #[test]
    fn price_level_serializes() {
        let level = types::PriceLevel {
            px: types::Money {
                value: "0.55".to_string(),
                currency: "USD".to_string(),
            },
            qty: "200".to_string(),
        };
        let json = serde_json::to_string(&level).expect("should serialize");
        assert!(json.contains("0.55"));
        assert!(json.contains("200"));
        assert!(
            json.contains("px"),
            "the gateway's field name is px, not price"
        );
    }

    #[test]
    fn settlement_price_deserializes() {
        let json = r#"{"symbol": "BTC-USD", "price": "0.75", "timestamp": "2024-01-01T00:00:00Z"}"#;
        let settlement: types::SettlementPrice =
            serde_json::from_str(json).expect("should deserialize");
        assert_eq!(settlement.symbol, "BTC-USD");
        assert_eq!(settlement.price, "0.75");
    }

    #[test]
    fn user_balance_deserializes() {
        let json = r#"{
            "currentBalance": 1000.00,
            "currency": "USD",
            "buyingPower": 950.00,
            "assetNotional": 500.00,
            "assetAvailable": 450.00
        }"#;
        let balance: types::UserBalance = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(balance.current_balance, 1000.00);
        assert_eq!(balance.buying_power, 950.00);
        assert_eq!(balance.currency, "USD");
    }

    #[test]
    fn cancel_all_orders_params_serializes() {
        let params = types::CancelAllOrdersParams {
            symbol: Some("BTC-USD".to_string()),
        };
        let json = serde_json::to_string(&params).expect("should serialize");
        assert!(json.contains("BTC-USD"));
    }

    #[test]
    fn us_position_deserializes() {
        let json = r#"{
            "symbol": "BTC-USD",
            "quantity": 100,
            "avgEntryPrice": "0.50",
            "unrealizedPnl": "25.00"
        }"#;
        let position: types::UsPosition = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(position.symbol, "BTC-USD");
        assert_eq!(position.quantity, 100);
        assert_eq!(position.avg_entry_price, "0.50");
    }

    #[test]
    fn us_market_deserializes() {
        let json = r#"{
            "id": "market-123",
            "slug": "btc-usd",
            "question": "Will BTC be above $50k?",
            "status": "open",
            "category": "crypto",
            "startDate": "2024-01-01",
            "endDate": "2024-12-31",
            "description": "Test market",
            "active": true,
            "closed": false,
            "marketType": "binary",
            "marketSides": [],
            "instruments": [],
            "outcomes": []
        }"#;
        let market: types::UsMarket = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(market.id, "market-123");
        assert_eq!(market.slug, "btc-usd");
        assert!(market.active);
    }

    #[test]
    fn us_event_deserializes() {
        let json = r#"{
            "id": "event-123",
            "slug": "2024-election",
            "title": "2024 US Election",
            "category": "politics",
            "startDate": "2024-01-01",
            "endDate": "2024-11-05"
        }"#;
        let event: types::UsEvent = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(event.id, "event-123");
        assert_eq!(event.slug, "2024-election");
        assert_eq!(event.title, "2024 US Election");
    }

    #[test]
    fn client_has_all_resource_accessors() {
        let client = create_test_client();

        // Just verify all resources can be accessed
        let _ = client.markets();
        let _ = client.events();
        let _ = client.orders();
        let _ = client.account();
        let _ = client.portfolio();
        let _ = client.search();

        // If this compiles, all accessors are available
    }

    #[test]
    fn resources_are_cheap_to_clone() {
        let client = create_test_client();
        let markets1 = client.markets();
        let _markets2 = markets1.clone();
    }
}
