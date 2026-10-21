//! Synthetic tests for SEC filings (REQ-YGAP-001).
//!
//! NOTE: `tests/fixtures/filings_api_secFilings_AAPL.json` is a SYNTHETIC fixture. Yahoo
//! rate-limited the recording machine, so its shape is derived from Python yfinance's
//! `secFilings` parser (`scrapers/quote.py::_fetch_sec_filings`). Re-record live when possible.

use chrono::NaiveDate;
use httpmock::{Method::GET, Mock, MockServer};
use url::Url;
use yfinance_rs::{ProjectionIssue, SecFilingsBuilder, Ticker, YfClient, YfError, YfWarning};

fn preauthed_client(server: &MockServer) -> YfClient {
    YfClient::builder()
        .base_quote_api(
            Url::parse(&format!("{}/v10/finance/quoteSummary/", server.base_url())).unwrap(),
        )
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

fn mock_sec_filings<'a>(server: &'a MockServer, sym: &str, body: String) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/v10/finance/quoteSummary/{sym}"))
            .query_param("modules", "secFilings")
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    })
}

#[tokio::test]
async fn sec_filings_requests_only_the_sec_filings_module() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v10/finance/quoteSummary/AAPL")
            .is_true(|req| {
                let modules: Vec<String> = req
                    .query_params()
                    .into_iter()
                    .filter(|(k, _)| k == "modules")
                    .map(|(_, v)| v)
                    .collect();
                modules == ["secFilings"]
            });
        then.status(200)
            .header("content-type", "application/json")
            .body(crate::common::fixture(
                "filings_api_secFilings",
                "AAPL",
                "json",
            ));
    });

    let ticker = Ticker::new(&preauthed_client(&server), "AAPL");
    let filings = ticker.sec_filings().await.unwrap();

    mock.assert();
    assert_eq!(filings.len(), 3);
}

#[tokio::test]
async fn sec_filings_expose_date_type_title_edgar_url_and_all_exhibits() {
    let server = MockServer::start();
    let mock = mock_sec_filings(
        &server,
        "AAPL",
        crate::common::fixture("filings_api_secFilings", "AAPL", "json"),
    );

    let client = preauthed_client(&server);
    let response = Ticker::new(&client, "AAPL")
        .sec_filings_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    assert_eq!(response.diagnostics.warnings.len(), 0);
    let filings = response.data;
    assert_eq!(filings.len(), 3);

    let annual = &filings[0];
    assert_eq!(annual.date, NaiveDate::from_ymd_opt(2025, 10, 31).unwrap());
    assert_eq!(annual.filing_type, "10-K");
    assert_eq!(
        annual.title.as_deref(),
        Some("Annual report pursuant to Section 13 and 15(d)")
    );
    assert_eq!(
        annual.edgar_url.as_deref(),
        Some("https://finance.yahoo.com/sec-filing/AAPL/0000320193-25-000079_320193")
    );
    let types: Vec<&str> = annual
        .exhibits
        .iter()
        .map(|e| e.exhibit_type.as_str())
        .collect();
    assert_eq!(types, ["10-K", "EX-21.1", "EX-31.1"]);
    assert!(annual.exhibits[0].url.ends_with("aapl-20250927.htm"));
    assert!(annual.exhibits[0].download_url.is_some());
    assert!(annual.exhibits[2].download_url.is_none());

    assert_eq!(filings[1].filing_type, "10-Q");
    assert_eq!(filings[1].exhibits.len(), 2);
    assert_eq!(filings[2].filing_type, "8-K");
    assert_eq!(filings[2].exhibits[0].exhibit_type, "EX-99.1");
}

#[tokio::test]
async fn missing_sec_filings_module_returns_empty_with_warning_and_strict_error() {
    let server = MockServer::start();
    let mock = mock_sec_filings(
        &server,
        "NOFILE",
        r#"{"quoteSummary":{"result":[{}],"error":null}}"#.to_string(),
    );

    let client = preauthed_client(&server);
    let response = SecFilingsBuilder::new(&client, "NOFILE")
        .fetch_with_diagnostics()
        .await
        .unwrap();

    assert_eq!(response.data.len(), 0);
    assert!(matches!(
        response.diagnostics.warnings.as_slice(),
        [YfWarning::ProviderFeatureUnavailable {
            feature: "secFilings",
            reason: ProjectionIssue::ProviderUnavailable {
                feature: "secFilings"
            },
            ..
        }]
    ));

    let err = SecFilingsBuilder::new(&client, "NOFILE")
        .strict()
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)));

    let err = Ticker::new(&client, "NOFILE")
        .strict()
        .sec_filings()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)));
    mock.assert_calls(3);
}

const BAD_DATE_BODY: &str = r#"{
  "quoteSummary": {
    "result": [{
      "secFilings": {
        "filings": [
          { "date": "2025-10-31", "type": "10-K", "title": "Annual", "edgarUrl": "https://example.test/a", "exhibits": [] },
          { "date": "31/10/2025", "type": "8-K", "title": "Bad date", "edgarUrl": "https://example.test/bad", "exhibits": [] },
          { "date": "2025-08-01", "type": "10-Q", "title": "Quarterly", "edgarUrl": "https://example.test/q" }
        ]
      }
    }],
    "error": null
  }
}"#;

#[tokio::test]
async fn sec_filing_with_unparseable_date_is_dropped_and_siblings_kept() {
    let server = MockServer::start();
    let mock = mock_sec_filings(&server, "BADDATE", BAD_DATE_BODY.to_string());

    let client = preauthed_client(&server);
    let response = SecFilingsBuilder::new(&client, "BADDATE")
        .fetch_with_diagnostics()
        .await
        .unwrap();

    let types: Vec<&str> = response
        .data
        .iter()
        .map(|f| f.filing_type.as_str())
        .collect();
    assert_eq!(types, ["10-K", "10-Q"]);
    assert_eq!(response.data[1].exhibits.len(), 0);
    assert!(matches!(
        response.diagnostics.warnings.as_slice(),
        [YfWarning::DroppedItem {
            item: "sec_filing",
            key: Some(key),
            reason: ProjectionIssue::InvalidField { field: "date", .. },
            ..
        }] if key == "https://example.test/bad"
    ));

    let err = SecFilingsBuilder::new(&client, "BADDATE")
        .strict()
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)));
    mock.assert_calls(2);
}
