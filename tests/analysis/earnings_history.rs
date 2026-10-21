//! Synthetic tests for earnings history (REQ-YGAP-004, first bullet).
//!
//! NOTE: `tests/fixtures/analysis_api_earningsHistory_MSFT.json` is a SYNTHETIC fixture.
//! Yahoo rate-limited the recording machine, so its shape is derived from Python yfinance's
//! `earningsHistory` parser (`scrapers/analysis.py::earnings_history`). Re-record live when possible.

use chrono::NaiveDate;
use httpmock::{Method::GET, Mock, MockServer};
use paft::Decimal;
use paft::money::{Currency, IsoCurrency};
use url::Url;
use yfinance_rs::{
    ProjectionIssue, Ticker, YfClient, YfError, YfWarning, analysis::AnalysisBuilder,
};

fn preauthed_client(server: &MockServer) -> YfClient {
    YfClient::builder()
        .base_quote_api(
            Url::parse(&format!("{}/v10/finance/quoteSummary/", server.base_url())).unwrap(),
        )
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

fn mock_history<'a>(server: &'a MockServer, sym: &str, body: String) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/v10/finance/quoteSummary/{sym}"))
            .query_param("modules", "earningsHistory")
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    })
}

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

#[tokio::test]
async fn earnings_history_returns_one_row_per_quarter_with_eps_money_in_resolved_currency() {
    let server = MockServer::start();
    let mock = mock_history(
        &server,
        "MSFT",
        crate::common::fixture("analysis_api_earningsHistory", "MSFT", "json"),
    );

    let response = Ticker::new(&preauthed_client(&server), "MSFT")
        .earnings_history_with_diagnostics(None)
        .await
        .unwrap();

    mock.assert();
    assert!(
        response.diagnostics.warnings.is_empty(),
        "{:?}",
        response.diagnostics.warnings
    );
    let rows = response.data;
    assert_eq!(rows.len(), 4);
    let quarters: Vec<NaiveDate> = rows.iter().map(|r| r.quarter).collect();
    assert_eq!(
        quarters,
        [
            NaiveDate::from_ymd_opt(2024, 9, 30).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            NaiveDate::from_ymd_opt(2025, 3, 31).unwrap(),
            NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
        ]
    );

    let latest = &rows[3];
    assert_eq!(latest.period.as_deref(), Some("-1q"));
    let actual = latest.eps_actual.as_ref().unwrap();
    assert_eq!(actual.amount(), dec("3.65"));
    assert_eq!(actual.currency(), &Currency::Iso(IsoCurrency::USD));
    assert_eq!(latest.eps_estimate.as_ref().unwrap().amount(), dec("3.37"));
    assert_eq!(
        latest.eps_difference.as_ref().unwrap().amount(),
        dec("0.28")
    );
    assert_eq!(latest.surprise_percent, Some(dec("0.0831")));
    for row in &rows {
        for eps in [&row.eps_actual, &row.eps_estimate, &row.eps_difference] {
            assert_eq!(
                eps.as_ref().unwrap().currency(),
                &Currency::Iso(IsoCurrency::USD)
            );
        }
    }
}

const EUR_HISTORY: &str = r#"{
  "quoteSummary": {
    "result": [{
      "earningsHistory": {
        "history": [{
          "epsActual": { "raw": 1.5 },
          "epsEstimate": { "raw": 1.4 },
          "surprisePercent": { "raw": 0.0714 },
          "quarter": { "raw": 1751241600 },
          "currency": "EUR",
          "period": "-1q"
        }]
      }
    }],
    "error": null
  }
}"#;

#[tokio::test]
async fn earnings_history_uses_row_currency_and_honours_override() {
    let server = MockServer::start();
    let mock = mock_history(&server, "SAP.DE", EUR_HISTORY.to_string());
    let client = preauthed_client(&server);

    let rows = AnalysisBuilder::new(&client, "SAP.DE")
        .earnings_history(None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].eps_actual.as_ref().unwrap().currency(),
        &Currency::Iso(IsoCurrency::EUR)
    );
    assert!(rows[0].eps_difference.is_none());

    let rows = AnalysisBuilder::new(&client, "SAP.DE")
        .earnings_history(Some(Currency::Iso(IsoCurrency::USD)))
        .await
        .unwrap();
    assert_eq!(
        rows[0].eps_estimate.as_ref().unwrap().currency(),
        &Currency::Iso(IsoCurrency::USD)
    );
    mock.assert_calls(2);
}

#[tokio::test]
async fn missing_earnings_history_module_is_provider_unavailable_and_strict_error() {
    let server = MockServer::start();
    let mock = mock_history(
        &server,
        "NOHIST",
        r#"{"quoteSummary":{"result":[{}],"error":null}}"#.to_string(),
    );
    let client = preauthed_client(&server);

    let response = AnalysisBuilder::new(&client, "NOHIST")
        .earnings_history_with_diagnostics(None)
        .await
        .unwrap();
    assert_eq!(response.data.len(), 0);
    assert!(matches!(
        response.diagnostics.warnings.as_slice(),
        [YfWarning::ProviderFeatureUnavailable {
            feature: "earningsHistory",
            reason: ProjectionIssue::ProviderUnavailable {
                feature: "earningsHistory"
            },
            ..
        }]
    ));

    let err = Ticker::new(&client, "NOHIST")
        .strict()
        .earnings_history(None)
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)));
    mock.assert_calls(2);
}

const BAD_QUARTER_HISTORY: &str = r#"{
  "quoteSummary": {
    "result": [{
      "earningsHistory": {
        "history": [
          { "epsActual": { "raw": 1.0 }, "quarter": { "raw": "soon" }, "currency": "USD", "period": "-2q" },
          { "epsActual": { "raw": 1.1 }, "quarter": { "raw": 1751241600 }, "currency": "USD", "period": "-1q" }
        ]
      }
    }],
    "error": null
  }
}"#;

#[tokio::test]
async fn earnings_history_row_with_invalid_quarter_is_dropped_and_strict_fails() {
    let server = MockServer::start();
    let mock = mock_history(&server, "BADQ", BAD_QUARTER_HISTORY.to_string());
    let client = preauthed_client(&server);

    let response = AnalysisBuilder::new(&client, "BADQ")
        .earnings_history_with_diagnostics(None)
        .await
        .unwrap();
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].period.as_deref(), Some("-1q"));
    assert!(response.diagnostics.warnings.iter().any(|w| matches!(
        w,
        YfWarning::DroppedItem {
            item: "earnings_history_row",
            key: Some(key),
            ..
        } if key == "-2q"
    )));

    let err = AnalysisBuilder::new(&client, "BADQ")
        .strict()
        .earnings_history(None)
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)));
    mock.assert_calls(2);
}
