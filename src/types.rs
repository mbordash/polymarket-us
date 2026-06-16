use serde::{Deserialize, Serialize};

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
    pub market_sides: Vec<serde_json::Value>,
    #[serde(default)]
    pub instruments: Vec<serde_json::Value>,
    #[serde(default)]
    pub outcomes: Vec<serde_json::Value>,
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
    pub action: String,
    #[serde(rename = "outcomeSide")]
    pub outcome_side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub price: Money,
    pub quantity: u64,
    pub tif: String,
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
    pub action: String,
    #[serde(rename = "outcomeSide")]
    pub outcome_side: String,
    #[serde(rename = "type")]
    pub order_type: String,
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
