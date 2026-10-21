//! REQ-YGAP-002: trailing-twelve-month (TTM) income statement and cash flow.
//!
//! All responses here are SYNTHETIC (built by `timeseries_synth`) because Yahoo blocks
//! live recording from the dev machine; they mirror the recorded timeseries fixture shape.

use crate::timeseries_synth::{
    CASHFLOW_KEYS, INCOME_KEYS, TIMESERIES_PATH, client_for, envelope, item, type_param,
};
use chrono::NaiveDate;
use httpmock::Method::GET;
use httpmock::MockServer;
use paft::domain::ReportingPeriod;
use paft::money::{Currency, IsoCurrency, Money};
use serde_json::json;
use yfinance_rs::core::conversions::money_from_f64;
use yfinance_rs::{FundamentalsBuilder, Ticker, YfError, YfWarning};

// 2026-03-31 00:00:00 UTC and 2025-12-31 00:00:00 UTC.
const TS_2026_03_31: i64 = 1_774_915_200;
const TS_2025_12_31: i64 = 1_767_139_200;

fn usd(value: f64) -> Money {
    money_from_f64(value, Currency::Iso(IsoCurrency::USD)).unwrap()
}

fn period(date: &str) -> ReportingPeriod {
    ReportingPeriod::date(NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()).unwrap()
}

fn ttm_income_body(sym: &str) -> String {
    let p = |raw: f64| [("2026-03-31", TS_2026_03_31, json!(raw))];
    envelope(&[
        item(
            sym,
            "trailingTotalRevenue",
            "TTM",
            Some("USD"),
            &p(435_617_000_000.0),
        ),
        item(
            sym,
            "trailingGrossProfit",
            "TTM",
            Some("USD"),
            &p(205_000_000_000.0),
        ),
        item(
            sym,
            "trailingOperatingIncome",
            "TTM",
            Some("USD"),
            &p(140_000_000_000.0),
        ),
        item(
            sym,
            "trailingNetIncome",
            "TTM",
            Some("USD"),
            &p(117_777_000_000.0),
        ),
        item(
            sym,
            "trailingTaxProvision",
            "TTM",
            Some("USD"),
            &p(25_095_000_000.0),
        ),
        item(
            sym,
            "trailingDepreciationAndAmortization",
            "TTM",
            Some("USD"),
            &p(11_500_000_000.0),
        ),
        crate::timeseries_synth::empty_item(sym, "trailingInterestExpense"),
    ])
}

/// REQ-YGAP-002.A1: TTM income statement requests only `trailing*` keys and returns
/// the single TTM row Yahoo provides.
#[tokio::test]
async fn ttm_income_stmt_requests_only_trailing_keys_and_returns_single_row() {
    let server = MockServer::start();
    let sym = "AAPL";
    let expected_type = type_param("trailing", &INCOME_KEYS);
    assert!(
        expected_type
            .split(',')
            .all(|key| key.starts_with("trailing"))
    );

    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("symbol", sym)
            .query_param("type", &expected_type)
            .query_param_exists("period1")
            .query_param_exists("period2")
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(ttm_income_body(sym));
    });

    let client = client_for(&server);
    let response = Ticker::new(&client, sym)
        .ttm_income_stmt_with_diagnostics(None)
        .await
        .unwrap();

    mock.assert();
    assert!(
        response.diagnostics.is_empty(),
        "{:?}",
        response.diagnostics
    );
    let rows = response.data;
    assert_eq!(rows.len(), 1, "Yahoo normally returns one TTM row");
    let row = &rows[0];
    assert_eq!(row.period, period("2026-03-31"));
    assert_eq!(row.total_revenue, Some(usd(435_617_000_000.0)));
    assert_eq!(row.gross_profit, Some(usd(205_000_000_000.0)));
    assert_eq!(row.operating_income, Some(usd(140_000_000_000.0)));
    assert_eq!(row.net_income, Some(usd(117_777_000_000.0)));
    assert_eq!(row.income_tax_expense, Some(usd(25_095_000_000.0)));
    assert_eq!(
        row.depreciation_and_amortization,
        Some(usd(11_500_000_000.0))
    );
    assert_eq!(row.interest_expense, None);
}

/// REQ-YGAP-002.A1: when Yahoo returns more than one trailing period, every row is kept,
/// newest first.
#[tokio::test]
async fn ttm_income_stmt_returns_every_trailing_row_newest_first() {
    let server = MockServer::start();
    let sym = "AAPL";
    let points = [
        ("2025-12-31", TS_2025_12_31, json!(400.0)),
        ("2026-03-31", TS_2026_03_31, json!(435.0)),
    ];
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("trailing", &INCOME_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(envelope(&[item(
                sym,
                "trailingTotalRevenue",
                "TTM",
                Some("USD"),
                &points,
            )]));
    });

    let client = client_for(&server);
    let rows = FundamentalsBuilder::new(&client, sym)
        .ttm_income_statement(None)
        .await
        .unwrap();

    mock.assert();
    let periods: Vec<_> = rows.iter().map(|row| row.period.clone()).collect();
    assert_eq!(periods, vec![period("2026-03-31"), period("2025-12-31")]);
    assert_eq!(rows[0].total_revenue, Some(usd(435.0)));
    assert_eq!(rows[1].total_revenue, Some(usd(400.0)));
}

/// REQ-YGAP-002.A1: TTM cash flow requests only `trailing*` keys and returns the TTM row,
/// with the same free-cash-flow inference as annual/quarterly cash flow.
#[tokio::test]
async fn ttm_cashflow_requests_only_trailing_keys_and_returns_single_row() {
    let server = MockServer::start();
    let sym = "MSFT";
    let p = |raw: f64| [("2026-03-31", TS_2026_03_31, json!(raw))];
    let body = envelope(&[
        item(
            sym,
            "trailingOperatingCashFlow",
            "TTM",
            Some("USD"),
            &p(150.0),
        ),
        item(
            sym,
            "trailingCapitalExpenditure",
            "TTM",
            Some("USD"),
            &p(-60.0),
        ),
        item(sym, "trailingNetIncome", "TTM", Some("USD"), &p(100.0)),
        item(
            sym,
            "trailingDepreciationAndAmortization",
            "TTM",
            Some("USD"),
            &p(30.0),
        ),
        crate::timeseries_synth::empty_item(sym, "trailingFreeCashFlow"),
    ]);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("trailing", &CASHFLOW_KEYS))
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });

    let client = client_for(&server);
    let response = Ticker::new(&client, sym)
        .ttm_cashflow_with_diagnostics(None)
        .await
        .unwrap();

    mock.assert();
    let rows = response.data;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].period, period("2026-03-31"));
    assert_eq!(rows[0].operating_cashflow, Some(usd(150.0)));
    assert_eq!(rows[0].capital_expenditures, Some(usd(-60.0)));
    assert_eq!(rows[0].free_cash_flow, Some(usd(90.0)));
    assert_eq!(rows[0].net_income, Some(usd(100.0)));
    assert!(response.diagnostics.warnings.iter().any(|warning| matches!(
        warning,
        YfWarning::RepairedData {
            item: "cash_flow",
            ..
        }
    )));
}

/// REQ-YGAP-002.A1 (strict path): an unparseable TTM value is a warning in best-effort
/// mode and an error in strict mode.
#[tokio::test]
async fn ttm_income_stmt_strict_mode_rejects_invalid_reported_value() {
    let server = MockServer::start();
    let sym = "AAPL";
    let body = envelope(&[
        item(
            sym,
            "trailingTotalRevenue",
            "TTM",
            Some("USD"),
            &[("2026-03-31", TS_2026_03_31, json!(435.0))],
        ),
        item(
            sym,
            "trailingNetIncome",
            "TTM",
            Some("USD"),
            &[("2026-03-31", TS_2026_03_31, json!("not-a-number"))],
        ),
    ]);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("trailing", &INCOME_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    let client = client_for(&server);

    let lenient = Ticker::new(&client, sym)
        .ttm_income_stmt_with_diagnostics(None)
        .await
        .unwrap();
    assert_eq!(lenient.data.len(), 1);
    assert_eq!(lenient.data[0].total_revenue, Some(usd(435.0)));
    assert_eq!(lenient.data[0].net_income, None);
    assert!(!lenient.diagnostics.is_empty());

    let err = FundamentalsBuilder::new(&client, sym)
        .strict()
        .ttm_income_statement(None)
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
    mock.assert_calls(2);
}

/// REQ-YGAP-002.A1: a TTM response with no data (metadata-only items) yields no rows.
#[tokio::test]
async fn ttm_cashflow_with_no_trailing_data_returns_empty() {
    let server = MockServer::start();
    let sym = "EMPTY";
    let body = envelope(
        &CASHFLOW_KEYS
            .iter()
            .map(|key| crate::timeseries_synth::empty_item(sym, &format!("trailing{key}")))
            .collect::<Vec<_>>(),
    );
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(format!("{TIMESERIES_PATH}/{sym}"))
            .query_param("type", type_param("trailing", &CASHFLOW_KEYS));
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    let client = client_for(&server);
    let response = Ticker::new(&client, sym)
        .ttm_cashflow_with_diagnostics(None)
        .await
        .unwrap();
    mock.assert();
    assert_eq!(response.data.len(), 0);
    assert!(response.diagnostics.is_empty());
}
