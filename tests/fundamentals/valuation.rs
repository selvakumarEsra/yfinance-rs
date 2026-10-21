//! REQ-YGAP-002: valuation measures from the fundamentals-timeseries endpoint.
//!
//! All responses here are SYNTHETIC (built by `timeseries_synth`) because Yahoo blocks
//! live recording from the dev machine; they mirror the recorded timeseries fixture shape
//! and use the exact Yahoo key names from Python yfinance (`quarterlyMarketCap`,
//! `quarterlyPeRatio`, `quarterlyEnterprisesValueEBITDARatio`, ...).

use crate::timeseries_synth::{
    TIMESERIES_PATH, VALUATION_KEYS, client_for, dec, empty_item, envelope, item, type_param,
};
use chrono::NaiveDate;
use httpmock::Method::GET;
use httpmock::MockServer;
use paft::money::{Currency, IsoCurrency, Money};
use serde_json::{Value, json};
use yfinance_rs::core::conversions::money_from_f64;
use yfinance_rs::{Ticker, ValuationBuilder, ValuationFrequency, YfError};

const TS_2025_06_30: i64 = 1_751_241_600;
const TS_2025_09_30: i64 = 1_759_190_400;
const TS_2025_12_31: i64 = 1_767_139_200;

fn usd(value: f64) -> Money {
    money_from_f64(value, Currency::Iso(IsoCurrency::USD)).unwrap()
}

fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
}

fn three_quarters(a: f64, b: f64, c: f64) -> [(&'static str, i64, Value); 3] {
    [
        ("2025-06-30", TS_2025_06_30, json!(a)),
        ("2025-09-30", TS_2025_09_30, json!(b)),
        ("2025-12-31", TS_2025_12_31, json!(c)),
    ]
}

/// Quarterly valuation body: all measures for three quarters, except PEG (only latest
/// quarter) and EV/EBITDA (absent: metadata-only item).
fn quarterly_body(sym: &str) -> String {
    let mut items = vec![
        item(
            sym,
            "quarterlyMarketCap",
            "3M",
            Some("USD"),
            &three_quarters(3.0e12, 3.4e12, 3.8e12),
        ),
        item(
            sym,
            "quarterlyEnterpriseValue",
            "3M",
            Some("USD"),
            &three_quarters(3.1e12, 3.5e12, 3.9e12),
        ),
        item(
            sym,
            "quarterlyPeRatio",
            "3M",
            None,
            &three_quarters(30.5, 33.25, 36.75),
        ),
        item(
            sym,
            "quarterlyForwardPeRatio",
            "3M",
            None,
            &three_quarters(28.1, 29.2, 31.3),
        ),
        item(
            sym,
            "quarterlyPegRatio",
            "3M",
            None,
            &[("2025-12-31", TS_2025_12_31, json!(2.4))],
        ),
        item(
            sym,
            "quarterlyPsRatio",
            "3M",
            None,
            &three_quarters(7.5, 8.1, 9.2),
        ),
        item(
            sym,
            "quarterlyPbRatio",
            "3M",
            None,
            &three_quarters(45.0, 50.5, 55.25),
        ),
        item(
            sym,
            "quarterlyEnterprisesValueRevenueRatio",
            "3M",
            None,
            &three_quarters(7.6, 8.2, 9.3),
        ),
        empty_item(sym, "quarterlyEnterprisesValueEBITDARatio"),
    ];
    // Yahoo pads some series with explicit `null` points; they must be skipped silently.
    items[3]["quarterlyForwardPeRatio"]
        .as_array_mut()
        .unwrap()
        .insert(0, Value::Null);
    envelope(&items)
}

/// REQ-YGAP-002.A2: one row per as-of date, newest first, with every ratio optional.
#[tokio::test]
async fn valuation_measures_returns_one_row_per_as_of_date_newest_first() {
    let server = MockServer::start();
    let sym = "AAPL";
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("symbol", sym)
            .query_param("type", type_param("quarterly", &VALUATION_KEYS))
            .query_param_exists("period1")
            .query_param_exists("period2")
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(quarterly_body(sym));
    });

    let client = client_for(&server);
    let response = Ticker::new(&client, sym)
        .valuation_measures_with_diagnostics(None)
        .await
        .unwrap();

    mock.assert();
    assert!(
        response.diagnostics.is_empty(),
        "{:?}",
        response.diagnostics
    );
    let rows = response.data;
    let dates: Vec<_> = rows.iter().map(|row| row.as_of).collect();
    assert_eq!(
        dates,
        vec![date("2025-12-31"), date("2025-09-30"), date("2025-06-30")],
        "one row per as-of date, newest first"
    );

    let newest = &rows[0];
    assert_eq!(newest.market_cap, Some(usd(3.8e12)));
    assert_eq!(newest.enterprise_value, Some(usd(3.9e12)));
    assert_eq!(newest.trailing_pe, Some(dec("36.75")));
    assert_eq!(newest.forward_pe, Some(dec("31.3")));
    assert_eq!(newest.peg_ratio, Some(dec("2.4")));
    assert_eq!(newest.price_to_sales, Some(dec("9.2")));
    assert_eq!(newest.price_to_book, Some(dec("55.25")));
    assert_eq!(newest.enterprise_value_to_revenue, Some(dec("9.3")));
    assert_eq!(newest.enterprise_value_to_ebitda, None);

    let oldest = &rows[2];
    assert_eq!(oldest.market_cap, Some(usd(3.0e12)));
    assert_eq!(oldest.trailing_pe, Some(dec("30.5")));
    assert_eq!(oldest.forward_pe, Some(dec("28.1")));
    assert_eq!(
        oldest.peg_ratio, None,
        "PEG only reported for the latest quarter"
    );
    assert_eq!(oldest.enterprise_value_to_ebitda, None);
}

/// REQ-YGAP-002.A2: a security that only reports some measures yields rows whose other
/// ratios are `None`, without warnings.
#[tokio::test]
async fn valuation_measures_every_ratio_is_optional() {
    let server = MockServer::start();
    let sym = "BRK-B";
    let mut items = vec![item(
        sym,
        "quarterlyMarketCap",
        "3M",
        Some("USD"),
        &[("2025-12-31", TS_2025_12_31, json!(1.0e12))],
    )];
    items.extend(
        VALUATION_KEYS[1..]
            .iter()
            .map(|key| empty_item(sym, &format!("quarterly{key}"))),
    );
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", &VALUATION_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&items));
    });

    let client = client_for(&server);
    let response = Ticker::new(&client, sym)
        .strict()
        .valuation_measures_with_diagnostics(None)
        .await
        .unwrap();

    mock.assert();
    assert!(response.diagnostics.is_empty());
    assert_eq!(response.data.len(), 1);
    let row = &response.data[0];
    assert_eq!(row.as_of, date("2025-12-31"));
    assert_eq!(row.market_cap, Some(usd(1.0e12)));
    assert_eq!(row.enterprise_value, None);
    assert_eq!(row.trailing_pe, None);
    assert_eq!(row.forward_pe, None);
    assert_eq!(row.peg_ratio, None);
    assert_eq!(row.price_to_sales, None);
    assert_eq!(row.price_to_book, None);
    assert_eq!(row.enterprise_value_to_revenue, None);
    assert_eq!(row.enterprise_value_to_ebitda, None);
}

/// REQ-YGAP-002.A2: the builder's frequency selects the Yahoo key prefix
/// (`trailing*` gives the current values).
#[tokio::test]
async fn valuation_builder_trailing_frequency_requests_trailing_keys() {
    let server = MockServer::start();
    let sym = "MSFT";
    let body = envelope(&[
        item(
            sym,
            "trailingMarketCap",
            "TTM",
            Some("USD"),
            &[("2026-03-31", 1_774_915_200, json!(2.9e12))],
        ),
        item(
            sym,
            "trailingPeRatio",
            "TTM",
            None,
            &[("2026-03-31", 1_774_915_200, json!(34.5))],
        ),
    ]);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("trailing", &VALUATION_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });

    let client = client_for(&server);
    let rows = ValuationBuilder::new(&client, sym)
        .frequency(ValuationFrequency::Trailing)
        .fetch()
        .await
        .unwrap();

    mock.assert();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].as_of, date("2026-03-31"));
    assert_eq!(rows[0].market_cap, Some(usd(2.9e12)));
    assert_eq!(rows[0].trailing_pe, Some(dec("34.5")));
}

/// REQ-YGAP-002.A2 (strict path): an unparseable ratio is dropped with a warning in
/// best-effort mode and fails the call in strict mode.
#[tokio::test]
async fn valuation_measures_strict_mode_rejects_invalid_ratio() {
    let server = MockServer::start();
    let sym = "AAPL";
    let body = envelope(&[
        item(
            sym,
            "quarterlyPeRatio",
            "3M",
            None,
            &[("2025-12-31", TS_2025_12_31, json!(36.75))],
        ),
        item(
            sym,
            "quarterlyPsRatio",
            "3M",
            None,
            &[("2025-12-31", TS_2025_12_31, json!("bogus"))],
        ),
    ]);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", &VALUATION_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    let client = client_for(&server);

    let lenient = Ticker::new(&client, sym)
        .valuation_measures_with_diagnostics(None)
        .await
        .unwrap();
    assert_eq!(lenient.data.len(), 1);
    assert_eq!(lenient.data[0].trailing_pe, Some(dec("36.75")));
    assert_eq!(lenient.data[0].price_to_sales, None);
    assert!(!lenient.diagnostics.is_empty());

    let err = ValuationBuilder::new(&client, sym)
        .strict()
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
    mock.assert_calls(2);
}

/// REQ-YGAP-002.A2 (strict path): a point with an unparseable `asOfDate` is dropped with
/// a warning in best-effort mode and fails the call in strict mode.
#[tokio::test]
async fn valuation_measures_strict_mode_rejects_invalid_as_of_date() {
    let server = MockServer::start();
    let sym = "AAPL";
    let mut pe = item(
        sym,
        "quarterlyPeRatio",
        "3M",
        None,
        &three_quarters(30.5, 33.25, 36.75),
    );
    pe["quarterlyPeRatio"][0]["asOfDate"] = json!("June 30th");
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", &VALUATION_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&[pe]));
    });
    let client = client_for(&server);

    let lenient = Ticker::new(&client, sym)
        .valuation_measures_with_diagnostics(None)
        .await
        .unwrap();
    let dates: Vec<_> = lenient.data.iter().map(|row| row.as_of).collect();
    assert_eq!(dates, vec![date("2025-12-31"), date("2025-09-30")]);
    assert!(!lenient.diagnostics.is_empty());

    let err = Ticker::new(&client, sym)
        .strict()
        .valuation_measures(None)
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
    mock.assert_calls(2);
}

/// REQ-YGAP-002.A2: an explicit currency override applies to market cap and EV.
#[tokio::test]
async fn valuation_measures_override_currency_applies_to_money_values() {
    let server = MockServer::start();
    let sym = "SAP.DE";
    let body = envelope(&[item(
        sym,
        "quarterlyMarketCap",
        "3M",
        None,
        &[("2025-12-31", TS_2025_12_31, json!(2.5e11))],
    )]);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", &VALUATION_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    let client = client_for(&server);

    let rows = Ticker::new(&client, sym)
        .valuation_measures(Some(Currency::Iso(IsoCurrency::EUR)))
        .await
        .unwrap();

    mock.assert();
    assert_eq!(
        rows[0].market_cap,
        Some(money_from_f64(2.5e11, Currency::Iso(IsoCurrency::EUR)).unwrap())
    );
}
