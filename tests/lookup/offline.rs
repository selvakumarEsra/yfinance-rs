//! Offline tests for `LookupBuilder` (REQ-YGAP-007).
//!
//! NOTE: the fixture `lookup_v1_all_apple.json` and the inline bodies below are
//! SYNTHETIC. Yahoo rate-limited the development machine, so their shape follows
//! Python yfinance's `lookup.py` parser rather than a live recording.

use std::str::FromStr;

use httpmock::Method::GET;
use httpmock::MockServer;
use paft::Decimal;
use paft::domain::{AssetKind, Exchange};
use url::Url;
use yfinance_rs::{LookupBuilder, LookupKind, ProjectionIssue, YfClient, YfError, YfWarning};

const PATH: &str = "/v1/finance/lookup";

fn fixture(endpoint: &str, key: &str) -> String {
    crate::common::fixture(endpoint, key, "json")
}

fn client() -> YfClient {
    YfClient::builder()
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

fn builder(server: &MockServer, client: &YfClient, query: &str) -> LookupBuilder {
    LookupBuilder::new(client, query)
        .lookup_base(Url::parse(&format!("{}{PATH}", server.base_url())).unwrap())
}

const EMPTY_BODY: &str = r#"{"finance":{"result":[{"documents":[]}],"error":null}}"#;

// REQ-YGAP-007.A1
#[tokio::test]
async fn lookup_sends_each_kind_as_yahoo_type_with_start_and_count() {
    let expected = [
        (LookupKind::All, "all"),
        (LookupKind::Equity, "equity"),
        (LookupKind::MutualFund, "mutualfund"),
        (LookupKind::ETF, "etf"),
        (LookupKind::Index, "index"),
        (LookupKind::Future, "future"),
        (LookupKind::Currency, "currency"),
        (LookupKind::Cryptocurrency, "cryptocurrency"),
    ];
    assert_eq!(expected.len(), LookupKind::ALL.len());

    let server = MockServer::start();
    let client = client();
    for (kind, code) in expected {
        assert_eq!(kind.yahoo_code(), code);
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path(PATH)
                .query_param("query", "gold")
                .query_param("type", code)
                .query_param("start", "40")
                .query_param("count", "20")
                .query_param("formatted", "false")
                .query_param("fetchPricingData", "true")
                .query_param("lang", "en-US")
                .query_param("region", "US");
            then.status(200)
                .header("content-type", "application/json")
                .body(EMPTY_BODY);
        });

        let results = builder(&server, &client, "gold")
            .kind(kind)
            .offset(40)
            .count(20)
            .fetch()
            .await
            .unwrap();
        assert_eq!(results.len(), 0);
        mock.assert();
    }
}

#[tokio::test]
async fn lookup_defaults_to_all_kind_first_page_of_25() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(PATH)
            .query_param("query", "apple")
            .query_param("type", "all")
            .query_param("start", "0")
            .query_param("count", "25");
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("lookup_v1", "all_apple"));
    });

    let client = client();
    let results = builder(&server, &client, "apple").fetch().await.unwrap();
    mock.assert();
    assert_eq!(results.len(), 3);
}

// REQ-YGAP-007.A2
#[tokio::test]
async fn lookup_results_expose_symbol_name_quote_type_exchange_and_last_price() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(PATH).query_param("query", "apple");
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("lookup_v1", "all_apple"));
    });

    let client = client();
    let resp = builder(&server, &client, "apple")
        .fetch_with_diagnostics()
        .await
        .unwrap();
    assert!(
        resp.diagnostics.warnings.is_empty(),
        "{:?}",
        resp.diagnostics
    );

    let results = resp.data;
    let aapl = &results[0];
    assert_eq!(aapl.symbol, "AAPL");
    assert_eq!(aapl.name.as_deref(), Some("Apple Inc."));
    assert_eq!(aapl.quote_type.as_deref(), Some("equity"));
    assert_eq!(aapl.kind, Some(AssetKind::Equity));
    assert_eq!(aapl.raw_exchange.as_deref(), Some("NMS"));
    assert_eq!(aapl.exchange, Some(Exchange::NASDAQ));
    assert_eq!(aapl.last_price, Some(Decimal::from_str("228.05").unwrap()));
    assert_eq!(aapl.change, Some(Decimal::from_str("1.23").unwrap()));
    assert_eq!(
        aapl.change_percent,
        Some(Decimal::from_str("0.5423").unwrap())
    );
    assert_eq!(aapl.industry.as_deref(), Some("Consumer Electronics"));
    let instrument = aapl.instrument.as_ref().unwrap();
    assert_eq!(instrument.symbol.as_str(), "AAPL");

    let leveraged_etf = &results[1];
    assert_eq!(leveraged_etf.kind, Some(AssetKind::Fund));
    assert_eq!(leveraged_etf.quote_type.as_deref(), Some("etf"));

    // Price is optional: absent when Yahoo omits pricing data.
    let aple = &results[2];
    assert_eq!(aple.symbol, "APLE");
    assert_eq!(aple.last_price, None);
    assert_eq!(aple.exchange, Some(Exchange::NYSE));
}

// REQ-YGAP-007.A3
#[tokio::test]
async fn lookup_rejects_empty_query_before_any_request() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path(PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(EMPTY_BODY);
    });

    let client = client();
    for query in ["", "  \t\n "] {
        let err = builder(&server, &client, query).fetch().await.unwrap_err();
        match err {
            YfError::InvalidParams(message) => assert!(message.contains("query")),
            other => panic!("expected InvalidParams, got {other:?}"),
        }
    }
    mock.assert_calls(0);
}

#[tokio::test]
async fn lookup_rejects_zero_count_before_any_request() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path(PATH);
        then.status(200).body(EMPTY_BODY);
    });

    let client = client();
    let err = builder(&server, &client, "apple")
        .count(0)
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::InvalidParams(_)), "{err:?}");
    mock.assert_calls(0);
}

const MALFORMED_BODY: &str = r#"{"finance":{"result":[{"documents":[
    {"symbol":"AAPL","shortName":"Apple Inc.","quoteType":"equity","exchange":"NMS","regularMarketPrice":228.05},
    {"shortName":"No symbol","quoteType":"equity"},
    {"symbol":"   ","shortName":"Blank symbol"},
    42,
    {"symbol":"MSFT","shortName":"Microsoft","quoteType":"equity","exchange":"NMS","regularMarketPrice":"n/a"}
  ]}],"error":null}}"#;

#[tokio::test]
async fn lookup_drops_malformed_documents_with_warnings() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(MALFORMED_BODY);
    });

    let client = client();
    let resp = builder(&server, &client, "x")
        .fetch_with_diagnostics()
        .await
        .unwrap();

    let symbols: Vec<_> = resp.data.iter().map(|r| r.symbol.as_str()).collect();
    assert_eq!(symbols, ["AAPL", "MSFT"]);
    assert_eq!(resp.data[1].last_price, None);

    let warns = &resp.diagnostics.warnings;
    let dropped = warns
        .iter()
        .filter(|w| {
            matches!(
                w,
                YfWarning::DroppedItem {
                    item: "lookup_result",
                    ..
                }
            )
        })
        .count();
    assert_eq!(dropped, 3, "{warns:?}");
    assert!(warns.iter().any(|w| matches!(
        w,
        YfWarning::OmittedPresentField {
            path: "regularMarketPrice",
            key: Some(k),
            reason: ProjectionIssue::InvalidField { .. },
            ..
        } if k == "MSFT"
    )));
}

#[tokio::test]
async fn lookup_strict_mode_fails_on_malformed_document() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(MALFORMED_BODY);
    });

    let client = client();
    let err = builder(&server, &client, "x")
        .strict()
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
}

#[tokio::test]
async fn lookup_yahoo_error_is_api_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"finance":{"result":null,"error":{"code":"Bad Request","description":"Invalid type"}}}"#);
    });

    let client = client();
    let err = builder(&server, &client, "x").fetch().await.unwrap_err();
    assert!(matches!(err, YfError::Api(_)), "{err:?}");
}
