//! Offline tests for `MarketBuilder` (REQ-YGAP-006).
//!
//! NOTE: the fixtures `market_time_US.json` and `market_summary_US.json` are
//! SYNTHETIC. Yahoo rate-limited the development machine, so their shape follows
//! Python yfinance's `domain/market.py` parser rather than a live recording.

use std::str::FromStr;

use chrono::{TimeZone, Utc};
use httpmock::Method::GET;
use httpmock::MockServer;
use paft::Decimal;
use paft::domain::AssetKind;
use url::Url;
use yfinance_rs::{MarketBuilder, MarketRegion, ProjectionIssue, YfClient, YfError, YfWarning};

const TIME_PATH: &str = "/v6/finance/markettime";
const SUMMARY_PATH: &str = "/v6/finance/quote/marketSummary";

fn fixture(endpoint: &str, key: &str) -> String {
    crate::common::fixture(endpoint, key, "json")
}

fn client() -> YfClient {
    YfClient::builder()
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

fn builder(server: &MockServer, client: &YfClient, region: MarketRegion) -> MarketBuilder {
    MarketBuilder::new(client, region)
        .market_time_base(Url::parse(&format!("{}{TIME_PATH}", server.base_url())).unwrap())
        .summary_base(Url::parse(&format!("{}{SUMMARY_PATH}", server.base_url())).unwrap())
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

// REQ-YGAP-006.A1
#[tokio::test]
async fn market_status_returns_utc_session_times_and_exchange_timezone() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(TIME_PATH)
            .query_param("market", "US")
            .query_param("formatted", "true")
            .query_param("key", "finance")
            .query_param("lang", "en-US");
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("market_time", "US"));
    });

    let client = client();
    let resp = builder(&server, &client, MarketRegion::US)
        .status_with_diagnostics()
        .await
        .unwrap();
    mock.assert();

    assert_eq!(resp.diagnostics.warnings.len(), 0, "{:?}", resp.diagnostics);
    let status = resp.data.expect("US status");
    assert_eq!(status.market_id.as_deref(), Some("us"));
    assert_eq!(status.name.as_deref(), Some("U.S. markets"));
    assert_eq!(status.status.as_deref(), Some("open"));
    assert_eq!(status.is_open(), Some(true));
    assert_eq!(
        status.open,
        Some(Utc.with_ymd_and_hms(2026, 10, 21, 13, 30, 0).unwrap())
    );
    assert_eq!(
        status.close,
        Some(Utc.with_ymd_and_hms(2026, 10, 21, 20, 0, 0).unwrap())
    );
    assert_eq!(
        status.as_of,
        Some(Utc.with_ymd_and_hms(2026, 10, 21, 17, 45, 12).unwrap())
    );
    assert_eq!(status.timezone, Some(chrono_tz::America::New_York));
    assert_eq!(status.timezone_abbreviation.as_deref(), Some("EDT"));
    assert_eq!(status.utc_offset_seconds, Some(-4 * 3600));
    assert!(status.message.unwrap().contains("close"));
}

#[tokio::test]
async fn market_status_for_non_us_region_rejects_us_market_time_with_warning_and_fails_strict() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(TIME_PATH)
            .query_param("market", "EUROPE");
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("market_time", "US"));
    });

    let client = client();
    let resp = builder(&server, &client, MarketRegion::Europe)
        .status_with_diagnostics()
        .await
        .unwrap();
    assert_eq!(resp.data, None);
    assert_eq!(resp.diagnostics.warnings.len(), 1, "{:?}", resp.diagnostics);
    assert!(matches!(
        &resp.diagnostics.warnings[0],
        YfWarning::ProviderFeatureUnavailable {
            feature: "market_status",
            reason: ProjectionIssue::ProviderError { .. },
            ..
        }
    ));

    let plain = builder(&server, &client, MarketRegion::Europe)
        .status()
        .await
        .unwrap();
    assert_eq!(plain, None);

    let err = builder(&server, &client, MarketRegion::Europe)
        .strict()
        .status()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
    mock.assert_calls(3);
}

#[tokio::test]
async fn market_status_for_non_us_region_keeps_regional_market_time() {
    let server = MockServer::start();
    let body = r#"{"finance":{"marketTimes":[{"marketTime":[{"id":"gb","name":"U.K. markets",
        "status":"closed","open":"2026-10-21T08:00:00+01:00","close":"2026-10-21T16:30:00+01:00",
        "timezone":[{"$text":"Europe/London","short":"BST","gmtoffset":"3600000"}]}]}],"error":null}}"#;
    server.mock(|when, then| {
        when.method(GET).path(TIME_PATH).query_param("market", "GB");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });

    let client = client();
    let status = builder(&server, &client, MarketRegion::GB)
        .strict()
        .status()
        .await
        .unwrap()
        .expect("regional status");
    assert_eq!(status.market_id.as_deref(), Some("gb"));
    assert_eq!(status.timezone, Some(chrono_tz::Europe::London));
    assert_eq!(
        status.open,
        Some(Utc.with_ymd_and_hms(2026, 10, 21, 7, 0, 0).unwrap())
    );
}

#[tokio::test]
async fn market_status_unparseable_timezone_is_omitted_with_warning_and_fails_strict() {
    let server = MockServer::start();
    let body = r#"{"finance":{"marketTimes":[{"marketTime":[{"id":"us","status":"closed",
        "open":"2026-10-21T09:30:00-04:00","close":"not-a-date",
        "timezone":[{"$text":"Mars/Olympus","short":"MST","gmtoffset":"0"}]}]}],"error":null}}"#;
    let mock = server.mock(|when, then| {
        when.method(GET).path(TIME_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });

    let client = client();
    let resp = builder(&server, &client, MarketRegion::US)
        .status_with_diagnostics()
        .await
        .unwrap();
    let status = resp.data.expect("status");
    assert!(status.open.is_some());
    assert_eq!(status.close, None);
    assert_eq!(status.timezone, None);
    assert_eq!(status.timezone_abbreviation.as_deref(), Some("MST"));
    assert_eq!(status.is_open(), Some(false));
    let warns = &resp.diagnostics.warnings;
    assert!(warns.iter().any(|w| matches!(
        w,
        YfWarning::OmittedPresentField {
            path: "marketTime.timezone",
            ..
        }
    )));
    assert!(warns.iter().any(|w| matches!(
        w,
        YfWarning::OmittedPresentField {
            path: "marketTime.close",
            ..
        }
    )));

    let err = builder(&server, &client, MarketRegion::US)
        .strict()
        .status()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
    mock.assert_calls(2);
}

#[tokio::test]
async fn market_status_without_market_time_entry_is_missing_data() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(TIME_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"finance":{"marketTimes":[],"error":null}}"#);
    });

    let client = client();
    let err = builder(&server, &client, MarketRegion::US)
        .status()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::MissingData(_)), "{err:?}");
}

// REQ-YGAP-006.A2
#[tokio::test]
async fn market_summary_returns_one_entry_per_index_with_price_and_change() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path(SUMMARY_PATH)
            .query_param("market", "US")
            .query_param("formatted", "false")
            .query_param("lang", "en-US")
            .query_param_exists("fields");
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("market_summary", "US"));
    });

    let client = client();
    let resp = builder(&server, &client, MarketRegion::US)
        .summary_with_diagnostics()
        .await
        .unwrap();
    mock.assert();
    assert_eq!(resp.diagnostics.warnings.len(), 0, "{:?}", resp.diagnostics);

    let entries = resp.data;
    let symbols: Vec<_> = entries.iter().map(|e| e.symbol.as_str()).collect();
    assert_eq!(symbols, ["^GSPC", "^DJI", "^IXIC", "CL=F"]);

    let spx = &entries[0];
    assert_eq!(spx.name.as_deref(), Some("S&P 500"));
    assert_eq!(spx.exchange.as_deref(), Some("SNP"));
    assert_eq!(spx.kind, Some(AssetKind::Index));
    assert_eq!(spx.price, dec("5945.67"));
    assert_eq!(spx.change, Some(dec("-12.34")));
    assert_eq!(spx.change_percent, Some(dec("-0.2071")));
    assert_eq!(spx.market_state.as_deref(), Some("REGULAR"));
    assert_eq!(
        spx.market_time,
        Some(Utc.timestamp_opt(1_792_604_712, 0).unwrap())
    );
    assert_eq!(spx.currency, None);

    let crude = &entries[3];
    assert_eq!(crude.kind, Some(AssetKind::Future));
    assert_eq!(crude.currency.as_deref(), Some("USD"));
}

const MALFORMED_SUMMARY: &str = r#"{"marketSummaryResponse":{"result":[
    {"symbol":"^GSPC","shortName":"S&P 500","quoteType":"INDEX","exchange":"SNP",
     "regularMarketPrice":5945.67,"regularMarketChange":-12.34,"regularMarketChangePercent":-0.2071},
    {"shortName":"No symbol","regularMarketPrice":1.0},
    {"symbol":"^BAD","shortName":"Bad price","regularMarketPrice":"n/a"},
    {"symbol":"^NOPRICE","shortName":"No price"},
    "not-an-object"
  ],"error":null}}"#;

// REQ-YGAP-006.A2
#[tokio::test]
async fn market_summary_drops_malformed_entries_with_warnings() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(SUMMARY_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(MALFORMED_SUMMARY);
    });

    let client = client();
    let resp = builder(&server, &client, MarketRegion::US)
        .summary_with_diagnostics()
        .await
        .unwrap();

    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].symbol, "^GSPC");

    let dropped: Vec<_> = resp
        .diagnostics
        .warnings
        .iter()
        .filter_map(|w| match w {
            YfWarning::DroppedItem {
                item: "market_summary_entry",
                key,
                reason,
                ..
            } => Some((key.clone(), reason.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(dropped.len(), 4, "{dropped:?}");
    assert!(
        dropped
            .iter()
            .any(|(_, r)| matches!(r, ProjectionIssue::MissingRequiredField { field: "symbol" }))
    );
    assert!(dropped.iter().any(|(k, r)| k.as_deref() == Some("^BAD")
        && matches!(
            r,
            ProjectionIssue::InvalidField {
                field: "regularMarketPrice",
                ..
            }
        )));
    assert!(dropped.iter().any(|(k, r)| k.as_deref() == Some("^NOPRICE")
        && matches!(
            r,
            ProjectionIssue::MissingRequiredField {
                field: "regularMarketPrice"
            }
        )));
}

// REQ-YGAP-006.A2
#[tokio::test]
async fn market_summary_strict_mode_fails_on_malformed_entry() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(SUMMARY_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(MALFORMED_SUMMARY);
    });

    let client = client();
    let err = builder(&server, &client, MarketRegion::US)
        .strict()
        .summary()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)), "{err:?}");
}

#[tokio::test]
async fn market_summary_yahoo_error_is_api_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(SUMMARY_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"marketSummaryResponse":{"result":null,"error":{"code":"Bad Request","description":"Invalid market"}}}"#);
    });

    let client = client();
    let err = builder(&server, &client, MarketRegion::US)
        .summary()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::Api(_)), "{err:?}");
}

// REQ-YGAP-006.A3
#[tokio::test]
async fn market_requests_send_region_as_yahoo_market_code_for_all_regions() {
    let expected = [
        (MarketRegion::US, "US"),
        (MarketRegion::GB, "GB"),
        (MarketRegion::Asia, "ASIA"),
        (MarketRegion::Europe, "EUROPE"),
        (MarketRegion::Rates, "RATES"),
        (MarketRegion::Commodities, "COMMODITIES"),
        (MarketRegion::Currencies, "CURRENCIES"),
        (MarketRegion::Cryptocurrencies, "CRYPTOCURRENCIES"),
    ];
    assert_eq!(expected.len(), MarketRegion::ALL.len());

    let server = MockServer::start();
    let client = client();
    for (region, code) in expected {
        assert_eq!(region.yahoo_code(), code);
        let time = server.mock(|when, then| {
            when.method(GET).path(TIME_PATH).query_param("market", code);
            then.status(200)
                .header("content-type", "application/json")
                .body(fixture("market_time", "US"));
        });
        let summary = server.mock(|when, then| {
            when.method(GET)
                .path(SUMMARY_PATH)
                .query_param("market", code);
            then.status(200)
                .header("content-type", "application/json")
                .body(fixture("market_summary", "US"));
        });

        let b = builder(&server, &client, region);
        assert_eq!(b.region(), region);
        // The synthetic markettime body is U.S. data, so only US yields a status.
        let status = b.status().await.unwrap();
        assert_eq!(status.is_some(), region == MarketRegion::US);
        b.summary().await.unwrap();
        time.assert();
        summary.assert();
    }
}
