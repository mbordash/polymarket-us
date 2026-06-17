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
        self.client
            .internal_request::<(), (), types::OrderBook>(
                Method::GET,
                &format!("/v1/markets/{symbol}/book"),
                None,
                None,
                false,
            )
            .await
    }

    /// Get best bid/offer for a market
    pub async fn bbo(&self, symbol: &str) -> Result<types::BestBidOffer, PolymarketUsError> {
        self.client
            .internal_request::<(), (), types::BestBidOffer>(
                Method::GET,
                &format!("/v1/markets/{symbol}/bbo"),
                None,
                None,
                false,
            )
            .await
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
        let markets = client.markets();
        // Just verify it can be created; actual API calls need mocking
        drop(markets);
    }

    #[test]
    fn markets_client_has_expected_methods() {
        let client = create_test_client();
        let markets = client.markets();

        // Verify methods exist by checking they can be called (won't execute due to no server)
        // This is primarily a compile-time check through the type system
        assert_eq!(
            std::any::type_name_of_val(&markets),
            "polymarket_us::resources::MarketsClient<'_>"
        );
    }

    // ========================================================================
    // EventsClient Tests
    // ========================================================================

    #[test]
    fn events_client_creation() {
        let client = create_test_client();
        let events = client.events();
        drop(events);
    }

    #[test]
    fn events_client_type_check() {
        let client = create_test_client();
        let events = client.events();
        assert_eq!(
            std::any::type_name_of_val(&events),
            "polymarket_us::resources::EventsClient<'_>"
        );
    }

    // ========================================================================
    // OrdersClient Tests
    // ========================================================================

    #[test]
    fn orders_client_creation() {
        let client = create_test_client();
        let orders = client.orders();
        drop(orders);
    }

    #[test]
    fn orders_client_type_check() {
        let client = create_test_client();
        let orders = client.orders();
        assert_eq!(
            std::any::type_name_of_val(&orders),
            "polymarket_us::resources::OrdersClient<'_>"
        );
    }

    #[test]
    fn place_order_request_serializes() {
        let req = types::PlaceOrderRequest {
            symbol: "BTC-USD".to_string(),
            action: types::order_action::BUY.to_string(),
            outcome_side: types::outcome::LONG.to_string(),
            order_type: types::order_type::LIMIT.to_string(),
            price: types::Money {
                value: "0.50".to_string(),
                currency: "USD".to_string(),
            },
            quantity: 100,
            tif: types::tif::GTC.to_string(),
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
                action: types::order_action::BUY.to_string(),
                outcome_side: types::outcome::LONG.to_string(),
                order_type: types::order_type::LIMIT.to_string(),
                price: types::Money {
                    value: "0.50".to_string(),
                    currency: "USD".to_string(),
                },
                quantity: 100,
                tif: types::tif::GTC.to_string(),
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
        let account = client.account();
        drop(account);
    }

    #[test]
    fn account_client_type_check() {
        let client = create_test_client();
        let account = client.account();
        assert_eq!(
            std::any::type_name_of_val(&account),
            "polymarket_us::resources::AccountClient<'_>"
        );
    }

    // ========================================================================
    // PortfolioClient Tests
    // ========================================================================

    #[test]
    fn portfolio_client_creation() {
        let client = create_test_client();
        let portfolio = client.portfolio();
        drop(portfolio);
    }

    #[test]
    fn portfolio_client_type_check() {
        let client = create_test_client();
        let portfolio = client.portfolio();
        assert_eq!(
            std::any::type_name_of_val(&portfolio),
            "polymarket_us::resources::PortfolioClient<'_>"
        );
    }

    // ========================================================================
    // SearchClient Tests
    // ========================================================================

    #[test]
    fn search_client_creation() {
        let client = create_test_client();
        let search = client.search();
        drop(search);
    }

    #[test]
    fn search_client_type_check() {
        let client = create_test_client();
        let search = client.search();
        assert_eq!(
            std::any::type_name_of_val(&search),
            "polymarket_us::resources::SearchClient<'_>"
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
            action: types::order_action::SELL.to_string(),
            outcome_side: types::outcome::SHORT.to_string(),
            order_type: types::order_type::LIMIT.to_string(),
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

    #[test]
    fn order_book_deserializes() {
        let json = r#"{"bids": [{"price": "0.50", "quantity": "100"}], "asks": [{"price": "0.55", "quantity": "150"}]}"#;
        let book: types::OrderBook = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.bids[0].price, "0.50");
    }

    #[test]
    fn best_bid_offer_deserializes() {
        let json = r#"{"bid": {"price": "0.50", "quantity": "100"}, "ask": {"price": "0.55", "quantity": "150"}}"#;
        let bbo: types::BestBidOffer = serde_json::from_str(json).expect("should deserialize");
        assert!(bbo.bid.is_some());
        assert!(bbo.ask.is_some());
        assert_eq!(bbo.bid.unwrap().price, "0.50");
    }

    #[test]
    fn price_level_serializes() {
        let level = types::PriceLevel {
            price: "0.55".to_string(),
            quantity: "200".to_string(),
        };
        let json = serde_json::to_string(&level).expect("should serialize");
        assert!(json.contains("0.55"));
        assert!(json.contains("200"));
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
        let markets2 = markets1.clone();

        // Both should reference same client
        drop(markets1);
        drop(markets2);
    }
}
