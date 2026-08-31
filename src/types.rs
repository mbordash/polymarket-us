use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// String-constant modules (kept for compatibility; prefer the typed enums below)
// ---------------------------------------------------------------------------

pub mod order_action {
    pub const BUY: &str = "ORDER_ACTION_BUY";
    pub const SELL: &str = "ORDER_ACTION_SELL";
}

pub mod order_type {
    pub const LIMIT: &str = "ORDER_TYPE_LIMIT";
}

pub mod tif {
    pub const GTC: &str = "TIME_IN_FORCE_GOOD_TILL_CANCEL";
    pub const GTD: &str = "TIME_IN_FORCE_GOOD_TILL_DATE";
    pub const FAK: &str = "TIME_IN_FORCE_IMMEDIATE_OR_CANCEL";
    pub const FOK: &str = "TIME_IN_FORCE_FILL_OR_KILL";
}

pub mod outcome {
    pub const LONG: &str = "LONG";
    pub const SHORT: &str = "SHORT";
}

// ---------------------------------------------------------------------------
// Typed enums (preferred over the string-constant modules above)
// ---------------------------------------------------------------------------

/// Whether this order is a buy or a sell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderAction {
    #[serde(rename = "ORDER_ACTION_BUY")]
    Buy,
    #[serde(rename = "ORDER_ACTION_SELL")]
    Sell,
}

impl fmt::Display for OrderAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => f.write_str("ORDER_ACTION_BUY"),
            Self::Sell => f.write_str("ORDER_ACTION_SELL"),
        }
    }
}

/// Outcome side — long (yes) or short (no).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderSide {
    #[serde(rename = "LONG")]
    Long,
    #[serde(rename = "SHORT")]
    Short,
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Long => f.write_str("LONG"),
            Self::Short => f.write_str("SHORT"),
        }
    }
}

/// Order execution type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderType {
    #[serde(rename = "ORDER_TYPE_LIMIT")]
    Limit,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ORDER_TYPE_LIMIT")
    }
}

/// Time-in-force policy for an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TimeInForce {
    /// Good-till-cancel — stays open until filled or explicitly cancelled.
    #[serde(rename = "TIME_IN_FORCE_GOOD_TILL_CANCEL")]
    GoodTillCancel,
    /// Good-till-date — expires at a specified timestamp.
    #[serde(rename = "TIME_IN_FORCE_GOOD_TILL_DATE")]
    GoodTillDate,
    /// Immediate-or-cancel (fill-and-kill) — any unfilled portion is cancelled.
    #[serde(rename = "TIME_IN_FORCE_IMMEDIATE_OR_CANCEL")]
    ImmediateOrCancel,
    /// Fill-or-kill — must be filled entirely or cancelled entirely.
    #[serde(rename = "TIME_IN_FORCE_FILL_OR_KILL")]
    FillOrKill,
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::GoodTillCancel => "TIME_IN_FORCE_GOOD_TILL_CANCEL",
            Self::GoodTillDate => "TIME_IN_FORCE_GOOD_TILL_DATE",
            Self::ImmediateOrCancel => "TIME_IN_FORCE_IMMEDIATE_OR_CANCEL",
            Self::FillOrKill => "TIME_IN_FORCE_FILL_OR_KILL",
        };
        f.write_str(s)
    }
}

/// Known market status values.
///
/// [`UsMarket::status`] is kept as a raw `String` so no information is lost when
/// the API introduces a status this SDK does not model yet. Use
/// [`UsMarket::parsed_status`] to get this typed view of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum MarketStatus {
    Open,
    Closed,
    Resolved,
    /// Catch-all for any status string not yet modelled here.
    #[serde(other)]
    Unknown,
}

impl MarketStatus {
    /// Parse a raw status string, case-insensitively.
    ///
    /// Anything unrecognised maps to [`MarketStatus::Unknown`] rather than
    /// failing, so a new server-side status can never break a client.
    pub fn from_api_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "open" => Self::Open,
            "closed" => Self::Closed,
            "resolved" => Self::Resolved,
            _ => Self::Unknown,
        }
    }
}

impl std::str::FromStr for MarketStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_api_str(s))
    }
}

// ---------------------------------------------------------------------------
// REST response/request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketsResponse {
    #[serde(default)]
    pub markets: Vec<UsMarket>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsMarket {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub category: String,
    #[serde(default, rename = "startDate")]
    pub start_date: String,
    #[serde(default, rename = "endDate")]
    pub end_date: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub closed: bool,
    #[serde(default, rename = "marketType")]
    pub market_type: String,
    #[serde(default, rename = "marketSides")]
    pub market_sides: Vec<MarketSide>,
    #[serde(default)]
    pub instruments: Vec<serde_json::Value>,
    /// Outcome names, e.g. `["Titans", "Chargers"]`.
    ///
    /// The gateway sends this as a JSON-encoded *string* rather than an array,
    /// so it is decoded on the way in; a plain array is accepted too.
    #[serde(default, deserialize_with = "json_encoded_array")]
    pub outcomes: Vec<String>,
    /// Prices matching [`Self::outcomes`] positionally, as decimal strings.
    ///
    /// Encoded the same way as `outcomes`.
    #[serde(
        default,
        rename = "outcomePrices",
        deserialize_with = "json_encoded_array"
    )]
    pub outcome_prices: Vec<String>,
}

impl UsMarket {
    /// [`Self::status`] as a typed value. Unrecognised statuses become
    /// [`MarketStatus::Unknown`]; the raw string remains available on the field.
    pub fn parsed_status(&self) -> MarketStatus {
        MarketStatus::from_api_str(&self.status)
    }
}

/// Deserialize a field the gateway double-encodes: an array delivered as a
/// string whose *contents* are JSON.
///
/// `outcomes` arrives as `"[\"Titans\",\"Chargers\"]"` — note the outer
/// quotes — not as `["Titans", "Chargers"]`. Declaring the field as a sequence
/// therefore fails the whole response with `invalid type: string ..., expected
/// a sequence`, which is what broke `markets().list()` against live data.
///
/// A plain array is accepted too, so this keeps working if the gateway stops
/// double-encoding. Non-string elements are rendered rather than rejected,
/// since a price is equally plausible as `"0.55"` or `0.55` and neither is
/// worth failing an entire market listing over.
fn json_encoded_array<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Encoding {
        Array(Vec<serde_json::Value>),
        Encoded(String),
    }

    let items = match Option::<Encoding>::deserialize(deserializer)? {
        None => return Ok(Vec::new()),
        Some(Encoding::Array(items)) => items,
        Some(Encoding::Encoded(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(Vec::new());
            }
            serde_json::from_str(trimmed).map_err(serde::de::Error::custom)?
        }
    };

    Ok(items.into_iter().map(render_scalar).collect())
}

/// A string element as itself, anything else as its JSON rendering — so `1`
/// becomes `"1"` rather than `"\"1\""`.
fn render_scalar(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text,
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketSide {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub identifier: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub long: bool,
    #[serde(default, rename = "marketSideType")]
    pub market_side_type: String,
    #[serde(default)]
    pub team: Option<serde_json::Value>,
    #[serde(default)]
    pub player: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceOrderRequest {
    pub symbol: String,
    pub action: OrderAction,
    #[serde(rename = "outcomeSide")]
    pub outcome_side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub price: Money,
    pub quantity: u64,
    pub tif: TimeInForce,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub post_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

/// A monetary amount: a decimal string plus its currency.
///
/// Kept as a string rather than a float because the API sends it that way and a
/// caller pricing an order wants an exact decimal.
///
/// Both fields default when absent. That matters on the read side: `OpenOrder`
/// carries a `Money` price, and its consumers cancel leftover orders after a
/// restart — a response that fails to parse there leaves real orders working on
/// the venue. Serialization is unaffected, so request bodies still send both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Money {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaceOrderResponse {
    pub order_id: String,
    #[serde(default)]
    pub client_order_id: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub filled_quantity: u64,
    #[serde(default)]
    pub remaining_quantity: u64,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchedOrderRequest {
    pub orders: Vec<PlaceOrderRequest>,
    pub atomic: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchedOrderResponse {
    #[serde(default)]
    pub orders: Vec<PlaceOrderResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CancelOrderResponse {
    pub order_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub cancelled_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortfolioPositionsResponse {
    #[serde(default)]
    pub positions: std::collections::HashMap<String, UsPosition>,
    #[serde(default)]
    pub next_cursor: String,
    #[serde(default)]
    pub eof: bool,
    #[serde(default, rename = "availablePositions")]
    pub available_positions: Vec<UsPosition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsPosition {
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub quantity: i64,
    #[serde(default, rename = "avgEntryPrice")]
    pub avg_entry_price: String,
    #[serde(default, rename = "unrealizedPnl")]
    pub unrealized_pnl: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortfolioActivitiesResponse {
    #[serde(default)]
    pub activities: Vec<serde_json::Value>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountBalancesResponse {
    #[serde(default)]
    pub balances: Vec<UserBalance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserBalance {
    #[serde(default, rename = "currentBalance")]
    pub current_balance: f64,
    #[serde(default)]
    pub currency: String,
    #[serde(default, rename = "lastUpdated")]
    pub last_updated: Option<String>,
    #[serde(default, rename = "buyingPower")]
    pub buying_power: f64,
    #[serde(default, rename = "assetNotional")]
    pub asset_notional: Option<f64>,
    #[serde(default, rename = "assetAvailable")]
    pub asset_available: Option<f64>,
    #[serde(default, rename = "pendingCredit")]
    pub pending_credit: Option<f64>,
    #[serde(default, rename = "openOrders")]
    pub open_orders: Option<f64>,
    #[serde(default, rename = "unsettledFunds")]
    pub unsettled_funds: Option<f64>,
    #[serde(default, rename = "marginRequirement")]
    pub margin_requirement: Option<f64>,
    #[serde(default, rename = "balanceReservation")]
    pub balance_reservation: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CancelOrderParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CancelAllOrdersParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CancelAllOrdersResponse {
    #[serde(default)]
    pub cancelled: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifyOrderRequest {
    pub price: Money,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewOrderRequest {
    pub symbol: String,
    pub action: OrderAction,
    #[serde(rename = "outcomeSide")]
    pub outcome_side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub price: Money,
    pub quantity: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewOrderResponse {
    #[serde(default)]
    pub estimate: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClosePositionRequest {
    pub symbol: String,
    pub quantity: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClosePositionResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub order_id: Option<String>,
}

/// Direction of an open order, as reported on its `side` field.
///
/// Distinct from [`OrderAction`] despite meaning the same thing: the open-orders
/// response spells these `ORDER_SIDE_*` while `action` spells them
/// `ORDER_ACTION_*`, so they cannot share a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderSideDirection {
    #[serde(rename = "ORDER_SIDE_BUY")]
    Buy,
    #[serde(rename = "ORDER_SIDE_SELL")]
    Sell,
    /// Any value this crate does not know. Present so a single unrecognized enum
    /// cannot fail the whole open-orders response — callers use that response to
    /// cancel stale orders, and failing closed there leaves real orders working.
    #[serde(other)]
    Unknown,
}

/// Which outcome of a market an order is against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OutcomeSide {
    #[serde(rename = "OUTCOME_SIDE_YES")]
    Yes,
    #[serde(rename = "OUTCOME_SIDE_NO")]
    No,
    #[serde(other)]
    Unknown,
}

/// What an order does to a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderIntent {
    #[serde(rename = "ORDER_INTENT_BUY_LONG")]
    BuyLong,
    #[serde(rename = "ORDER_INTENT_SELL_LONG")]
    SellLong,
    #[serde(rename = "ORDER_INTENT_BUY_SHORT")]
    BuyShort,
    #[serde(rename = "ORDER_INTENT_SELL_SHORT")]
    SellShort,
    #[serde(other)]
    Unknown,
}

/// One resting order, as returned by `GET /v1/orders/open`.
///
/// Modeled from the documented response schema. Every field is `#[serde(default)]`
/// and the unknown-tolerant enums above are used throughout, because the primary
/// consumer of this type cancels leftover orders at startup: a response that fails
/// to parse leaves real orders working on the venue, which is strictly worse than
/// one field arriving as `Unknown`.
///
/// Previously this endpoint was typed as [`PlaceOrderResponse`], which carries only
/// an id, a status and quantities — so the market, side and price were discarded
/// and a caller could not tell which order it was looking at.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct OpenOrder {
    /// Exchange-assigned order id, and what `cancel` takes.
    #[serde(default)]
    pub id: String,
    /// Market identifier. Combine with [`Self::outcome_side`] to name a leg.
    #[serde(default, rename = "marketSlug")]
    pub market_slug: String,
    #[serde(default)]
    pub side: Option<OrderSideDirection>,
    #[serde(default, rename = "outcomeSide")]
    pub outcome_side: Option<OutcomeSide>,
    #[serde(default)]
    pub price: Option<Money>,
    /// Original order quantity, in contracts.
    #[serde(default)]
    pub quantity: f64,
    /// Cumulative filled quantity.
    #[serde(default, rename = "cumQuantity")]
    pub cum_quantity: f64,
    /// Remaining unfilled quantity.
    #[serde(default, rename = "leavesQuantity")]
    pub leaves_quantity: f64,
    #[serde(default)]
    pub tif: Option<TimeInForce>,
    #[serde(default)]
    pub intent: Option<OrderIntent>,
    /// Lifecycle state. Left as a string because the documented schema names the
    /// enum without enumerating its values.
    #[serde(default)]
    pub state: String,
    #[serde(default, rename = "createTime")]
    pub create_time: Option<String>,
    #[serde(default, rename = "goodTillTime")]
    pub good_till_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetOpenOrdersResponse {
    #[serde(default)]
    pub orders: Vec<OpenOrder>,
}

// Events
#[derive(Debug, Clone, Deserialize)]
pub struct EventsResponse {
    #[serde(default)]
    pub events: Vec<UsEvent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// Market data helpers
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBook {
    #[serde(default)]
    pub bids: Vec<PriceLevel>,
    #[serde(default)]
    pub asks: Vec<PriceLevel>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PriceLevel {
    pub price: String,
    pub quantity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BestBidOffer {
    #[serde(default)]
    pub bid: Option<PriceLevel>,
    #[serde(default)]
    pub ask: Option<PriceLevel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettlementPrice {
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub timestamp: String,
}

// Search
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResults {
    #[serde(default)]
    pub markets: Vec<UsMarket>,
    #[serde(default)]
    pub events: Vec<UsEvent>,
}

// `League` and `Team` placeholders were removed in 0.4.0. They were never
// referenced by any request or response type, and publishing unreachable types
// commits the SDK to a shape the API has not been checked against. They will
// return alongside the endpoints that populate them.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_parse_from_the_gateway_double_encoded_form() {
        // The exact shape that broke `markets().list()` in 0.5.1.
        let market: UsMarket = serde_json::from_str(
            r#"{"slug":"x","outcomes":"[\"Titans\",\"Chargers\"]","outcomePrices":"[\"1\",\"0\"]"}"#,
        )
        .expect("should deserialize");
        assert_eq!(market.outcomes, ["Titans", "Chargers"]);
        assert_eq!(market.outcome_prices, ["1", "0"]);
    }

    #[test]
    fn outcomes_parse_from_a_plain_array() {
        // Accepted so the SDK survives the gateway dropping the encoding.
        let market: UsMarket =
            serde_json::from_str(r#"{"outcomes":["Yes","No"],"outcomePrices":["0.6","0.4"]}"#)
                .expect("should deserialize");
        assert_eq!(market.outcomes, ["Yes", "No"]);
        assert_eq!(market.outcome_prices, ["0.6", "0.4"]);
    }

    #[test]
    fn numeric_elements_are_rendered_rather_than_rejected() {
        let market: UsMarket =
            serde_json::from_str(r#"{"outcomePrices":"[1,0.55]"}"#).expect("should deserialize");
        assert_eq!(market.outcome_prices, ["1", "0.55"]);
    }

    #[test]
    fn absent_null_and_empty_outcomes_all_become_an_empty_vec() {
        for body in [r#"{}"#, r#"{"outcomes":null}"#, r#"{"outcomes":""}"#] {
            let market: UsMarket =
                serde_json::from_str(body).unwrap_or_else(|err| panic!("{body}: {err}"));
            assert!(market.outcomes.is_empty(), "{body}");
        }
    }

    #[test]
    fn a_malformed_encoding_is_an_error_not_a_silent_empty() {
        let err = serde_json::from_str::<UsMarket>(r#"{"outcomes":"[\"unterminated"}"#)
            .expect_err("should fail");
        assert!(
            err.to_string().contains("EOF") || err.to_string().contains("control"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn market_status_parses_case_insensitively() {
        assert_eq!(MarketStatus::from_api_str("open"), MarketStatus::Open);
        assert_eq!(MarketStatus::from_api_str("OPEN"), MarketStatus::Open);
        assert_eq!(
            MarketStatus::from_api_str("  Closed "),
            MarketStatus::Closed
        );
        assert_eq!(
            MarketStatus::from_api_str("RESOLVED"),
            MarketStatus::Resolved
        );
    }

    #[test]
    fn market_status_falls_back_to_unknown() {
        // A status the SDK does not model must not be an error.
        assert_eq!(MarketStatus::from_api_str("halted"), MarketStatus::Unknown);
        assert_eq!(MarketStatus::from_api_str(""), MarketStatus::Unknown);
    }

    #[test]
    fn parsed_status_reads_the_raw_field() {
        let json = r#"{"id": "m1", "status": "OPEN"}"#;
        let market: UsMarket = serde_json::from_str(json).expect("deserialize");
        // Raw string is preserved, typed view is derived from it.
        assert_eq!(market.status, "OPEN");
        assert_eq!(market.parsed_status(), MarketStatus::Open);
    }

    #[test]
    fn market_sides_deserialize_into_typed_values() {
        let json = r#"{
            "id": "m1",
            "marketSides": [
                {"id": "s1", "identifier": "YES", "price": "0.62", "long": true,
                 "marketSideType": "BINARY", "unmodelledField": 7}
            ]
        }"#;
        let market: UsMarket = serde_json::from_str(json).expect("deserialize");
        assert_eq!(market.market_sides.len(), 1);

        let side = &market.market_sides[0];
        assert_eq!(side.identifier, "YES");
        assert_eq!(side.price, "0.62");
        assert!(side.long);
        // Unmodelled keys survive in `extra` rather than being dropped.
        assert_eq!(
            side.extra.get("unmodelledField"),
            Some(&serde_json::json!(7))
        );
    }

    #[test]
    fn market_sides_tolerate_missing_fields() {
        // Every MarketSide field defaults, so a sparse object must still parse.
        let json = r#"{"id": "m1", "marketSides": [{}]}"#;
        let market: UsMarket = serde_json::from_str(json).expect("deserialize");
        assert_eq!(market.market_sides.len(), 1);
        assert_eq!(market.market_sides[0].identifier, "");
    }
}

#[cfg(test)]
mod open_order_tests {
    use super::*;

    /// The documented shape, deserialized field for field.
    #[test]
    fn a_documented_open_order_parses() {
        let json = r#"{"orders":[{
            "id":"ord_123",
            "marketSlug":"cpc-btc-above-yr-12-31-2026-200k",
            "side":"ORDER_SIDE_BUY",
            "type":"ORDER_TYPE_LIMIT",
            "price":{"value":"0.93","currency":"USD"},
            "quantity":8.6,
            "cumQuantity":1.6,
            "leavesQuantity":7.0,
            "tif":"TIME_IN_FORCE_GOOD_TILL_CANCEL",
            "intent":"ORDER_INTENT_BUY_SHORT",
            "outcomeSide":"OUTCOME_SIDE_NO",
            "state":"ORDER_STATE_OPEN",
            "createTime":"2026-08-31T01:00:00Z"
        }]}"#;
        let parsed: GetOpenOrdersResponse = serde_json::from_str(json).expect("documented shape parses");
        let o = &parsed.orders[0];
        assert_eq!(o.id, "ord_123");
        assert_eq!(o.market_slug, "cpc-btc-above-yr-12-31-2026-200k");
        assert_eq!(o.side, Some(OrderSideDirection::Buy));
        assert_eq!(o.outcome_side, Some(OutcomeSide::No));
        assert_eq!(o.price.as_ref().unwrap().value, "0.93");
        assert_eq!(o.quantity, 8.6);
        assert_eq!(o.cum_quantity, 1.6);
        assert_eq!(o.tif, Some(TimeInForce::GoodTillCancel));
        assert_eq!(o.intent, Some(OrderIntent::BuyShort));
    }

    /// An unknown enum value must NOT fail the response.
    ///
    /// The caller cancels leftover orders at startup. A parse failure there leaves
    /// real orders working on the venue with nothing managing them, so one
    /// unrecognized value degrading to `Unknown` is much the lesser harm.
    #[test]
    fn an_unknown_enum_value_does_not_fail_the_response() {
        let json = r#"{"orders":[{
            "id":"ord_future",
            "marketSlug":"some-market",
            "side":"ORDER_SIDE_SOMETHING_NEW",
            "outcomeSide":"OUTCOME_SIDE_MAYBE",
            "intent":"ORDER_INTENT_WHATEVER",
            "quantity":1.0
        }]}"#;
        let parsed: GetOpenOrdersResponse = serde_json::from_str(json).expect("unknown values tolerated");
        let o = &parsed.orders[0];
        assert_eq!(o.id, "ord_future", "the id still arrives, which is what cancel needs");
        assert_eq!(o.side, Some(OrderSideDirection::Unknown));
        assert_eq!(o.outcome_side, Some(OutcomeSide::Unknown));
        assert_eq!(o.intent, Some(OrderIntent::Unknown));
    }

    /// A price missing its currency must not fail the response.
    #[test]
    fn a_price_without_a_currency_parses() {
        let parsed: GetOpenOrdersResponse = serde_json::from_str(
            r#"{"orders":[{"id":"ord_px","price":{"value":"0.42"}}]}"#,
        )
        .expect("a partial price must not fail the whole response");
        let price = parsed.orders[0].price.as_ref().unwrap();
        assert_eq!(price.value, "0.42");
        assert_eq!(price.currency, "");
    }

    /// Absent optional fields are absent, not errors.
    #[test]
    fn a_sparse_order_parses() {
        let parsed: GetOpenOrdersResponse =
            serde_json::from_str(r#"{"orders":[{"id":"ord_min"}]}"#).expect("sparse order parses");
        let o = &parsed.orders[0];
        assert_eq!(o.id, "ord_min");
        assert!(o.price.is_none());
        assert!(o.tif.is_none());
        assert_eq!(o.quantity, 0.0);
    }

    /// An empty list is an empty list, and must not be confused with a failure.
    #[test]
    fn an_empty_response_parses() {
        let parsed: GetOpenOrdersResponse =
            serde_json::from_str(r#"{"orders":[]}"#).expect("empty parses");
        assert!(parsed.orders.is_empty());
    }
}
