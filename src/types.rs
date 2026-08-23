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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Money {
    pub value: String,
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

#[derive(Debug, Clone, Deserialize)]
pub struct GetOpenOrdersResponse {
    #[serde(default)]
    pub orders: Vec<PlaceOrderResponse>,
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
