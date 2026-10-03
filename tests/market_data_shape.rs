//! Does the gateway still send what the market-data types expect?
//!
//! Ignored by default: it needs real credentials and makes live calls. Run it
//! with `cargo test --test market_data_shape -- --ignored --nocapture` whenever
//! `BestBidOffer`, `OrderBook` or `PriceLevel` changes, and before publishing.
//!
//! This test exists because unit fixtures cannot catch the failure it guards.
//! Up to 0.9.0 these types were written to a plausible shape the gateway never
//! sends: root-level `bid`/`ask` and `bids`/`asks`, levels keyed
//! `price`/`quantity`, and no `marketData` envelope. Every field carried
//! `#[serde(default)]`, so `bbo()` and `order_book()` returned HTTP 200 and
//! deserialized to empty values on every call — a silent empty book, which a
//! caller cannot distinguish from a market with nothing resting. The unit tests
//! passed throughout, because their payloads were hand-written to match the
//! types rather than captured from the server.
//!
//! So the only check that can fail when the gateway and the types disagree is
//! one that asks the gateway.

use polymarket_us::{PolymarketUsClient, UsAuth};

/// A market with a live book, and the types that read it.
#[tokio::test]
#[ignore = "live API: needs POLYMARKET_US_KEY_ID and POLYMARKET_US_SECRET_KEY"]
async fn the_gateway_still_sends_what_the_market_data_types_expect() {
    let auth = match UsAuth::from_env() {
        Ok(a) => a,
        Err(e) => panic!("credentials required to run this test: {e}"),
    };
    let client = PolymarketUsClient::builder()
        .auth(auth)
        .build()
        .expect("client builds");

    // Open markets only, and that needs the filter: the unfiltered listing
    // returns long-settled markets whose books are empty, which parse correctly
    // and prove nothing about a populated level. The first run of this test
    // failed on exactly that, which is the refusal at the end working as
    // intended rather than a flaw in it.
    let query = [("closed", "false"), ("limit", "60")];
    let markets = client
        .markets()
        .list_with_query(Some(&query))
        .await
        .expect("markets list should succeed");

    let mut checked_a_level = false;
    let mut checked_a_quote = false;

    for m in markets.markets.iter().take(40) {
        let book = match client.markets().order_book(&m.slug).await {
            Ok(b) => b,
            // A market can vanish between the listing and the call.
            Err(_) => continue,
        };

        // The envelope and the side names. `offers`, not `asks`.
        if let Some(level) = book.bids.first().or_else(|| book.offers.first()) {
            assert!(
                !level.px.value.is_empty(),
                "a level's price must deserialize: the gateway sends `px`, an object, not a bare `price` string",
            );
            assert!(
                !level.qty.is_empty(),
                "a level's size must deserialize: the gateway sends `qty`, not `quantity`",
            );
            level
                .px
                .value
                .parse::<f64>()
                .expect("a level price should be a decimal string");
            checked_a_level = true;
        }

        let bbo = match client.markets().bbo(&m.slug).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        assert!(
            !bbo.state.is_empty(),
            "`state` must deserialize, or an empty book cannot be told from a closed market",
        );
        if let Some(px) = bbo.best_bid.as_ref().or(bbo.best_ask.as_ref()) {
            assert!(
                !px.value.is_empty(),
                "a quote must deserialize: the gateway sends `bestBid`/`bestAsk` under `marketData`, \
                 as Money objects, not root-level `bid`/`ask` price levels",
            );
            px.value
                .parse::<f64>()
                .expect("a quote should be a decimal string");
            checked_a_quote = true;
        }

        if checked_a_level && checked_a_quote {
            break;
        }
    }

    // The point of the test: if nothing populated was found, say so rather than
    // passing vacuously. Every field is `#[serde(default)]`, so a type that no
    // longer matches the gateway deserializes to empty and silently "passes".
    assert!(
        checked_a_level,
        "no populated book level was found in 40 markets — this test cannot \
         confirm the shape, which is exactly the blind spot it exists to cover",
    );
    assert!(
        checked_a_quote,
        "no populated quote was found in 40 markets — see above",
    );
}
