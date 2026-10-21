//! REQ-YGAP-002: chunking of long `type=` key lists, and no regression in the
//! annual/quarterly statement requests.
//!
//! Chunked responses are SYNTHETIC (built by `timeseries_synth`) in the recorded
//! timeseries shape. The regression tests replay the existing recorded fixtures.

use crate::timeseries_synth::{
    BALANCE_SHEET_KEYS, CASHFLOW_KEYS, INCOME_KEYS, TIMESERIES_PATH, VALUATION_KEYS, client_for,
    envelope, item, type_param,
};
use httpmock::Method::GET;
use httpmock::MockServer;
use serde_json::{Value, json};
use yfinance_rs::{FundamentalsBuilder, Ticker, ValuationBuilder, YfError};

const TS_2025_09_30: i64 = 1_759_190_400;
const TS_2025_12_31: i64 = 1_767_139_200;

fn two_quarters(a: f64, b: f64) -> [(&'static str, i64, Value); 2] {
    [
        ("2025-09-30", TS_2025_09_30, json!(a)),
        ("2025-12-31", TS_2025_12_31, json!(b)),
    ]
}

/// 1-based position of `key` in `all`, so each key gets a distinct value regardless of chunk.
fn key_weight(all: &[&str], key: &str) -> f64 {
    let pos = all.iter().position(|k| *k == key).unwrap() + 1;
    f64::from(u8::try_from(pos).unwrap())
}

/// One synthetic quarterly income item per key, with distinct values per key.
fn income_items(sym: &str, keys: &[&str]) -> Vec<Value> {
    keys.iter()
        .map(|key| {
            let base = 1_000.0 * key_weight(&INCOME_KEYS, key);
            item(
                sym,
                &format!("quarterly{key}"),
                "3M",
                Some("USD"),
                &two_quarters(base, base + 1.0),
            )
        })
        .collect()
}

fn valuation_items(sym: &str, keys: &[&str]) -> Vec<Value> {
    keys.iter()
        .map(|key| {
            let base = 10.0 * key_weight(&VALUATION_KEYS, key);
            let currency = matches!(*key, "MarketCap" | "EnterpriseValue").then_some("USD");
            item(
                sym,
                &format!("quarterly{key}"),
                "3M",
                currency,
                &two_quarters(base, base + 0.5),
            )
        })
        .collect()
}

/// REQ-YGAP-002.A3: a key list longer than the chunk size is sent as several requests
/// and the merged result equals the single-request result.
#[tokio::test]
async fn long_key_list_is_split_into_multiple_requests_and_merged() {
    let sym = "AAPL";
    let (first_keys, second_keys) = INCOME_KEYS.split_at(4);

    // Chunked: two requests, each answered with only its own keys.
    let chunked_server = MockServer::start();
    let first = chunked_server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", first_keys))
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&income_items(sym, first_keys)));
    });
    let second = chunked_server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", second_keys))
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&income_items(sym, second_keys)));
    });
    let chunked = FundamentalsBuilder::new(&client_for(&chunked_server), sym)
        ._timeseries_chunk_size(4)
        .income_statement_with_diagnostics(true, None)
        .await
        .unwrap();
    first.assert_calls(1);
    second.assert_calls(1);

    // Single request: the same data in one response (default chunk size).
    let single_server = MockServer::start();
    let single = single_server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", &INCOME_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&income_items(sym, &INCOME_KEYS)));
    });
    let unchunked = FundamentalsBuilder::new(&client_for(&single_server), sym)
        .income_statement_with_diagnostics(true, None)
        .await
        .unwrap();
    single.assert_calls(1);

    assert_eq!(chunked.data.len(), 2);
    assert!(chunked.data[0].total_revenue.is_some(), "from chunk 1");
    assert!(
        chunked.data[0].depreciation_and_amortization.is_some(),
        "from chunk 2"
    );
    assert_eq!(chunked.data, unchunked.data);
    assert_eq!(chunked.diagnostics, unchunked.diagnostics);
}

/// REQ-YGAP-002.A3: valuation measures chunk and merge the same way (three requests for
/// nine keys at chunk size four).
#[tokio::test]
async fn valuation_chunked_requests_merge_to_single_request_result() {
    let sym = "MSFT";
    let chunked_server = MockServer::start();
    let mocks: Vec<_> = VALUATION_KEYS
        .chunks(4)
        .map(|chunk| {
            let body = envelope(&valuation_items(sym, chunk));
            chunked_server.mock(|when, then| {
                when.method(GET)
                    .path(format!("{TIMESERIES_PATH}/{sym}"))
                    .query_param("type", type_param("quarterly", chunk));
                then.status(200)
                    .header("content-type", "application/json")
                    .body(body);
            })
        })
        .collect();
    assert_eq!(mocks.len(), 3);
    let chunked = ValuationBuilder::new(&client_for(&chunked_server), sym)
        ._timeseries_chunk_size(4)
        .fetch()
        .await
        .unwrap();
    for mock in &mocks {
        mock.assert_calls(1);
    }

    let single_server = MockServer::start();
    let single = single_server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("quarterly", &VALUATION_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&valuation_items(sym, &VALUATION_KEYS)));
    });
    let unchunked = Ticker::new(&client_for(&single_server), sym)
        .valuation_measures(None)
        .await
        .unwrap();
    single.assert_calls(1);

    assert_eq!(chunked.len(), 2);
    assert!(chunked[0].market_cap.is_some(), "from chunk 1");
    assert!(
        chunked[0].enterprise_value_to_ebitda.is_some(),
        "from chunk 3"
    );
    assert_eq!(chunked, unchunked);
}

/// REQ-YGAP-002.A3: if any chunk fails, the whole call fails (no silently partial rows).
#[tokio::test]
async fn chunked_request_fails_when_any_chunk_fails() {
    let sym = "AAPL";
    let (first_keys, second_keys) = INCOME_KEYS.split_at(4);
    let server = MockServer::start();
    let first = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("annual", first_keys));
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&[]));
    });
    let second = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("annual", second_keys));
        then.status(200)
            .header("content-type", "application/json")
            .body(
                json!({
                    "timeseries": {
                        "result": null,
                        "error": { "code": "Bad Request", "description": "too many types" }
                    }
                })
                .to_string(),
            );
    });

    let err = FundamentalsBuilder::new(&client_for(&server), sym)
        ._timeseries_chunk_size(4)
        .income_statement(false, None)
        .await
        .unwrap_err();
    first.assert_calls(1);
    assert!(second.calls() >= 1);
    assert!(matches!(err, YfError::Api(_)), "{err:?}");
}

/// Mocks a single timeseries request matching `expected_type` exactly, replaying `fixture`.
fn exact_type_mock<'a>(
    server: &'a MockServer,
    sym: &str,
    expected_type: &str,
    fixture: &str,
) -> httpmock::Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("symbol", sym)
            .query_param("type", expected_type)
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(crate::common::fixture(fixture, sym, "json"));
    })
}

/// REQ-YGAP-002.A4: annual and quarterly statement calls still send exactly one request
/// with the unchanged `annual*` / `quarterly*` key lists (recorded fixtures still match).
#[tokio::test]
async fn annual_and_quarterly_statement_requests_are_unchanged() {
    let server = MockServer::start();
    let client = client_for(&server);

    let income_q = exact_type_mock(
        &server,
        "AAPL",
        &type_param("quarterly", &INCOME_KEYS),
        "timeseries_income_statement_quarterly",
    );
    let income_a = exact_type_mock(
        &server,
        "SAP",
        &type_param("annual", &INCOME_KEYS),
        "timeseries_income_statement_annual",
    );
    let balance_a = exact_type_mock(
        &server,
        "MSFT",
        &type_param("annual", &BALANCE_SHEET_KEYS),
        "timeseries_balance_sheet_annual",
    );
    let cashflow_a = exact_type_mock(
        &server,
        "GOOGL",
        &type_param("annual", &CASHFLOW_KEYS),
        "timeseries_cash_flow_annual",
    );

    let quarterly_income = Ticker::new(&client, "AAPL")
        .quarterly_income_stmt(None)
        .await
        .unwrap();
    let annual_income = Ticker::new(&client, "SAP").income_stmt(None).await.unwrap();
    let balance_sheet = Ticker::new(&client, "MSFT")
        .balance_sheet(None)
        .await
        .unwrap();
    let cashflow = Ticker::new(&client, "GOOGL").cashflow(None).await.unwrap();
    for rows in [
        quarterly_income.len(),
        annual_income.len(),
        balance_sheet.len(),
        cashflow.len(),
    ] {
        assert_ne!(rows, 0, "recorded fixture should project rows");
    }

    income_q.assert_calls(1);
    income_a.assert_calls(1);
    balance_a.assert_calls(1);
    cashflow_a.assert_calls(1);
}
