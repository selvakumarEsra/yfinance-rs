//! Offline tests for `EarningsDatesBuilder` / `Ticker::earnings_dates` (REQ-YGAP-004).
//! Fixtures are synthetic; future rows are dated 2030 so they stay in the future.

use chrono::{TimeZone, Utc};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use serde_json::json;
use url::Url;
use yfinance_rs::{
    Currency, EarningsDatesBuilder, EarningsEventType, IsoCurrency, ProjectionIssue, Ticker,
    YfError, YfWarning,
};

use crate::calendars_support::{
    VIZ_PATH, amount, client, dec, earnings_dates_fixture, empty_viz_body,
};

const fn usd() -> Currency {
    Currency::Iso(IsoCurrency::USD)
}

fn expected_body(symbol: &str, size: u32) -> serde_json::Value {
    json!({
        "sortType": "DESC",
        "entityIdType": "earnings",
        "sortField": "startdatetime",
        "includeFields": [
            "ticker", "companyshortname", "eventname", "startdatetime", "startdatetimetype",
            "timeZoneShortName", "gmtOffsetMilliSeconds", "epsestimate", "epsactual",
            "epssurprisepct", "eventtype"
        ],
        "size": size,
        "query": {"operator": "EQ", "operands": ["ticker", symbol]}
    })
}

// REQ-YGAP-004.A2 (entityIdType earnings, default limit 12, newest first)
#[tokio::test]
async fn earnings_dates_posts_earnings_entity_for_symbol_and_returns_newest_first() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .query_param("crumb", "crumb")
            .json_body(expected_body("AAPL", 12));
        then.status(200)
            .header("content-type", "application/json")
            .body(earnings_dates_fixture("AAPL"));
    });

    let response = EarningsDatesBuilder::new(&client(&server), "AAPL")
        .currency(usd())
        .fetch_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    assert!(
        response.diagnostics.is_empty(),
        "{:?}",
        response.diagnostics
    );
    let dates: Vec<_> = response.data.iter().map(|row| row.date).collect();
    let mut sorted = dates.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(dates, sorted, "rows must be newest first");
    assert_eq!(dates.len(), 6);
    assert_eq!(
        dates[0],
        Utc.with_ymd_and_hms(2030, 4, 30, 20, 30, 0).unwrap()
    );

    let q4 = response
        .data
        .iter()
        .find(|row| row.date == Utc.with_ymd_and_hms(2025, 10, 30, 20, 30, 0).unwrap())
        .unwrap();
    assert_eq!(amount(q4.eps_estimate.as_ref()), Some(dec("1.77")));
    assert_eq!(amount(q4.eps_actual.as_ref()), Some(dec("1.85")));
    assert_eq!(q4.eps_actual.as_ref().unwrap().currency(), &usd());
    assert_eq!(q4.surprise_percent, Some(dec("4.52")));
    assert_eq!(q4.timezone_short_name.as_deref(), Some("EDT"));
    assert_eq!(q4.event_type, Some(EarningsEventType::Report));
    assert_eq!(q4.event_name.as_deref(), Some("Q4 2025 Earnings Call"));
}

// REQ-YGAP-004.A2 (at most `limit` rows)
#[tokio::test]
async fn earnings_dates_returns_at_most_limit_rows() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .json_body(expected_body("AAPL", 3));
        then.status(200)
            .header("content-type", "application/json")
            // Yahoo may return more rows than asked; the client still caps them.
            .body(earnings_dates_fixture("AAPL"));
    });

    let rows = EarningsDatesBuilder::new(&client(&server), "AAPL")
        .limit(3)
        .currency(usd())
        .fetch()
        .await
        .unwrap();

    mock.assert();
    assert_eq!(rows.len(), 3);
    assert!(rows.windows(2).all(|pair| pair[0].date >= pair[1].date));
    assert_eq!(
        rows[2].date,
        Utc.with_ymd_and_hms(2025, 10, 30, 20, 30, 0).unwrap()
    );
}

// REQ-YGAP-004.A2 (maximum 100, rejected before any request)
#[tokio::test]
async fn earnings_dates_limit_above_100_is_rejected_before_any_request() {
    let server = MockServer::start();
    let any = server.mock(|when, then| {
        when.any_request();
        then.status(500);
    });

    let builder_err = EarningsDatesBuilder::new(&client(&server), "AAPL")
        .limit(101)
        .fetch()
        .await
        .unwrap_err();
    let ticker_err = Ticker::new(&client(&server), "AAPL")
        .earnings_dates(Some(0))
        .await
        .unwrap_err();

    assert!(matches!(builder_err, YfError::InvalidParams(_)));
    assert!(matches!(ticker_err, YfError::InvalidParams(_)));
    assert_eq!(any.calls(), 0);
}

// REQ-YGAP-004.A3 (empty result => empty list + provider-feature-unavailable warning)
#[tokio::test]
async fn earnings_dates_empty_result_warns_provider_feature_unavailable() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(empty_viz_body());
    });

    let response = Ticker::new(&client(&server), "DELISTED")
        .earnings_dates_with_diagnostics(None)
        .await
        .unwrap();

    assert_eq!(response.data, [] as [yfinance_rs::EarningsDate; 0]);
    assert!(
        matches!(
            response.diagnostics.warnings.as_slice(),
            [YfWarning::ProviderFeatureUnavailable {
                feature: "earnings_dates",
                reason: ProjectionIssue::ProviderUnavailable { .. },
                ..
            }]
        ),
        "{:?}",
        response.diagnostics
    );
}

// REQ-YGAP-004.A3 (strict mode => error)
#[tokio::test]
async fn earnings_dates_empty_result_is_error_in_strict_mode() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(empty_viz_body());
    });

    let err = Ticker::new(&client(&server), "DELISTED")
        .strict()
        .earnings_dates(None)
        .await
        .unwrap_err();

    assert!(
        matches!(&err, YfError::DataQuality(warning) if matches!(
            warning.as_ref(),
            YfWarning::ProviderFeatureUnavailable { feature: "earnings_dates", .. }
        )),
        "{err:?}"
    );
}

// REQ-YGAP-004.A4 (future rows have no reported EPS/surprise; past rows carry both)
#[tokio::test]
async fn earnings_dates_future_rows_have_no_reported_eps_or_surprise() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(earnings_dates_fixture("AAPL"));
    });

    let rows = EarningsDatesBuilder::new(&client(&server), "AAPL")
        .currency(usd())
        .fetch()
        .await
        .unwrap();

    let now = Utc::now();
    let (future, past): (Vec<_>, Vec<_>) = rows.iter().partition(|row| row.date > now);
    assert_eq!(future.len(), 2);
    assert_eq!(past.len(), 4);
    for row in &future {
        assert!(row.eps_estimate.is_some(), "future rows keep the estimate");
        assert!(row.eps_actual.is_none(), "{row:?}");
        assert!(row.surprise_percent.is_none(), "{row:?}");
    }
    for row in &past {
        assert!(row.eps_actual.is_some(), "{row:?}");
        assert!(row.surprise_percent.is_some(), "{row:?}");
    }
}

// REQ-YGAP-004 (Ticker convenience; EPS currency resolved from Yahoo quote metadata)
#[tokio::test]
async fn ticker_earnings_dates_resolves_eps_currency_from_quote_metadata() {
    let server = MockServer::start();
    let viz = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .json_body(expected_body("AAPL", 12));
        then.status(200)
            .header("content-type", "application/json")
            .body(earnings_dates_fixture("AAPL"));
    });
    let quote = server.mock(|when, then| {
        when.method(GET)
            .path("/v7/finance/quote")
            .query_param("symbols", "AAPL");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"quoteResponse":{"result":[{"symbol":"AAPL","quoteType":"EQUITY","currency":"USD","financialCurrency":"USD"}],"error":null}}"#);
    });

    let client = yfinance_rs::YfClient::builder()
        .base_visualization(crate::calendars_support::viz_url(&server))
        .base_quote_v7(Url::parse(&format!("{}/v7/finance/quote", server.base_url())).unwrap())
        ._preauth("cookie", "crumb")
        .build()
        .unwrap();
    let response = Ticker::new(&client, "AAPL")
        .earnings_dates_with_diagnostics(None)
        .await
        .unwrap();

    viz.assert();
    quote.assert();
    assert_eq!(response.data.len(), 6);
    assert!(
        response.diagnostics.is_empty(),
        "{:?}",
        response.diagnostics
    );
    assert_eq!(
        response.data[0].eps_estimate.as_ref().unwrap().currency(),
        &usd()
    );
}

#[tokio::test]
async fn earnings_dates_row_with_unparseable_date_is_dropped_with_warning() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"finance":{"error":null,"result":[{"documents":[{
                "columns":[{"id":"ticker"},{"id":"startdatetime"},{"id":"epsestimate"}],
                "rows":[["AAPL","2025-10-30T20:30:00Z",1.77],["AAPL","garbage",1.0]]
            }]}]}}"#,
            );
    });

    let response = EarningsDatesBuilder::new(&client(&server), "AAPL")
        .currency(usd())
        .fetch_with_diagnostics()
        .await
        .unwrap();

    assert_eq!(response.data.len(), 1);
    assert!(matches!(
        response.diagnostics.warnings.as_slice(),
        [YfWarning::DroppedItem {
            item: "earnings_date",
            reason: ProjectionIssue::InvalidField {
                field: "startdatetime",
                ..
            },
            ..
        }]
    ));
}
