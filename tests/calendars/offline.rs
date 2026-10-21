//! Offline tests for `CalendarsBuilder` (REQ-YGAP-008). Fixtures are synthetic.

use chrono::{Days, NaiveDate, TimeZone, Utc};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use serde_json::{Value, json};
use url::Url;
use yfinance_rs::{
    CacheMode, CalendarsBuilder, Currency, EarningsCallTiming, IsoCurrency, ProjectionIssue,
    YfError, YfWarning,
};

use crate::calendars_support::{
    VIZ_PATH, amount, client, date_op, dec, empty_viz_body, viz_fixture, viz_url,
};

const fn nov3() -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 11, 3).unwrap()
}

const fn nov10() -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 11, 10).unwrap()
}

fn earnings_body(size: u32, offset: u32, extra: Vec<Value>) -> Value {
    let mut operands = vec![
        json!({"operator": "EQ", "operands": ["region", "us"]}),
        json!({"operator": "OR", "operands": [
            {"operator": "EQ", "operands": ["eventtype", "EAD"]},
            {"operator": "EQ", "operands": ["eventtype", "ERA"]}
        ]}),
        date_op("GTE", "startdatetime", "2025-11-03"),
        date_op("LTE", "startdatetime", "2025-11-10"),
    ];
    operands.extend(extra);
    json!({
        "sortType": "DESC",
        "entityIdType": "sp_earnings",
        "sortField": "intradaymarketcap",
        "includeFields": [
            "ticker", "companyshortname", "intradaymarketcap", "eventname", "startdatetime",
            "startdatetimetype", "epsestimate", "epsactual", "epssurprisepct"
        ],
        "size": size,
        "offset": offset,
        "query": {"operator": "AND", "operands": operands}
    })
}

fn window_body(entity: &str, include: &[&str], size: u32) -> Value {
    json!({
        "sortType": "DESC",
        "entityIdType": entity,
        "sortField": "startdatetime",
        "includeFields": include,
        "size": size,
        "offset": 0,
        "query": {"operator": "AND", "operands": [
            date_op("GTE", "startdatetime", "2025-11-03"),
            date_op("LTE", "startdatetime", "2025-11-10"),
        ]}
    })
}

fn dropped_keys(warnings: &[YfWarning]) -> Vec<String> {
    warnings
        .iter()
        .filter_map(|warning| match warning {
            YfWarning::DroppedItem {
                key: Some(key),
                reason: ProjectionIssue::InvalidField { field, .. },
                ..
            } if *field == "startdatetime" || *field == "filingdate" => Some(key.clone()),
            _ => None,
        })
        .collect()
}

// REQ-YGAP-008.A1 (earnings) + A3 (crumb-auth path)
#[tokio::test]
async fn earnings_calendar_posts_sp_earnings_entity_with_date_window_and_returns_typed_rows() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .query_param("lang", "en-US")
            .query_param("region", "US")
            .query_param("crumb", "crumb")
            .header("content-type", "application/json")
            .json_body(earnings_body(25, 0, vec![]));
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("sp_earnings"));
    });

    let response = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .limit(25)
        .earnings_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    let rows = response.data;
    assert_eq!(rows.len(), 2, "bad-date row dropped, siblings kept");
    let nvda = &rows[0];
    assert_eq!(nvda.symbol, "NVDA");
    assert_eq!(nvda.company_name.as_deref(), Some("NVIDIA Corporation"));
    assert_eq!(
        nvda.start,
        Utc.with_ymd_and_hms(2025, 11, 5, 21, 20, 0).unwrap()
    );
    assert_eq!(nvda.timing, Some(EarningsCallTiming::AfterMarketClose));
    assert_eq!(amount(nvda.eps_estimate.as_ref()), Some(dec("1.25")));
    assert_eq!(
        nvda.eps_estimate.as_ref().unwrap().currency(),
        &Currency::Iso(IsoCurrency::USD)
    );
    assert!(nvda.eps_actual.is_none());
    assert_eq!(amount(nvda.market_cap.as_ref()), Some(dec("4500000000000")));
    let pltr = &rows[1];
    assert_eq!(amount(pltr.eps_actual.as_ref()), Some(dec("0.21")));
    assert_eq!(pltr.surprise_percent, Some(dec("23.53")));
}

// REQ-YGAP-008.A1 (earnings filters)
#[tokio::test]
async fn earnings_calendar_min_market_cap_and_most_active_only_add_query_filters() {
    let server = MockServer::start();
    let screener = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/finance/screener/predefined/saved")
            .query_param("scrIds", "most_actives")
            .query_param("count", "200");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"finance":{"error":null,"result":[{"count":2,"quotes":[
                  {"symbol":"NVDA","quoteType":"EQUITY","marketCap":4500000000000,"currency":"USD","regularMarketPrice":180.0},
                  {"symbol":"TINY","quoteType":"EQUITY","marketCap":1000000000,"currency":"USD","regularMarketPrice":2.0}
                ]}]}}"#,
            );
    });
    let expected = earnings_body(
        12,
        0,
        vec![
            json!({"operator": "GTE", "operands": ["intradaymarketcap", 10_000_000_000_u64]}),
            json!({"operator": "OR", "operands": [
                {"operator": "EQ", "operands": ["ticker", "NVDA"]}
            ]}),
        ],
    );
    let viz = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH).json_body(expected);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("sp_earnings"));
    });

    let rows = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .min_market_cap(10_000_000_000)
        .most_active_only(true)
        .most_active_screener_base(
            Url::parse(&format!(
                "{}/v1/finance/screener/predefined/saved",
                server.base_url()
            ))
            .unwrap(),
        )
        .cache_mode(CacheMode::Bypass)
        .earnings()
        .await
        .unwrap();

    screener.assert();
    viz.assert();
    assert_eq!(rows.len(), 2);
}

// REQ-YGAP-008.A1 (IPOs)
#[tokio::test]
async fn ipo_calendar_posts_ipo_info_entity_with_gtelt_date_filters() {
    let server = MockServer::start();
    let range =
        |field: &str| json!({"operator": "GTELT", "operands": [field, "2025-11-03", "2025-11-10"]});
    let expected = json!({
        "sortType": "DESC",
        "entityIdType": "ipo_info",
        "sortField": "startdatetime",
        "includeFields": [
            "ticker", "companyshortname", "exchange_short_name", "filingdate", "startdatetime",
            "amendeddate", "pricefrom", "priceto", "offerprice", "currencyname", "shares", "dealtype"
        ],
        "size": 12,
        "offset": 0,
        "query": {"operator": "OR", "operands": [
            range("startdatetime"), range("filingdate"), range("amendeddate")
        ]}
    });
    let mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH).json_body(expected);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("ipo_info"));
    });

    // DateTime bounds are reduced to their calendar dates.
    let response = CalendarsBuilder::new(&client(&server))
        .between(
            Utc.with_ymd_and_hms(2025, 11, 3, 15, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2025, 11, 10, 1, 0, 0).unwrap(),
        )
        .ipos_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    let rows = response.data;
    assert_eq!(rows.len(), 2);
    let newco = &rows[0];
    assert_eq!(newco.symbol, "NEWC");
    assert_eq!(newco.exchange.as_deref(), Some("NASDAQ"));
    assert_eq!(newco.filing_date, NaiveDate::from_ymd_opt(2025, 10, 1));
    assert_eq!(newco.amended_date, NaiveDate::from_ymd_opt(2025, 10, 20));
    assert_eq!(
        newco.date,
        Some(Utc.with_ymd_and_hms(2025, 11, 6, 5, 0, 0).unwrap())
    );
    assert_eq!(amount(newco.price_from.as_ref()), Some(dec("15")));
    assert_eq!(amount(newco.price_to.as_ref()), Some(dec("17")));
    assert!(newco.offer_price.is_none());
    assert_eq!(newco.shares, Some(10_000_000));
    assert_eq!(newco.deal_type.as_deref(), Some("EXPECTED"));
    assert_eq!(amount(rows[1].offer_price.as_ref()), Some(dec("21.5")));
    assert_eq!(
        dropped_keys(&response.diagnostics.warnings),
        vec!["BADF".to_string()]
    );
}

// REQ-YGAP-008.A1 (splits)
#[tokio::test]
async fn splits_calendar_posts_splits_entity_with_startdatetime_window() {
    let server = MockServer::start();
    let expected = window_body(
        "splits",
        &[
            "ticker",
            "companyshortname",
            "startdatetime",
            "optionable",
            "old_share_worth",
            "share_worth",
        ],
        12,
    );
    let mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH).json_body(expected);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("splits"));
    });

    let response = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .splits_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    let rows = response.data;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].symbol, "SPLT");
    assert_eq!(rows[0].optionable, Some(true));
    assert_eq!(rows[0].old_shares, Some(dec("1")));
    assert_eq!(rows[0].new_shares, Some(dec("10")));
    assert_eq!(
        rows[0].payable_on,
        Utc.with_ymd_and_hms(2025, 11, 7, 5, 0, 0).unwrap()
    );
    assert_eq!(rows[1].optionable, Some(false));
    assert_eq!(
        dropped_keys(&response.diagnostics.warnings),
        vec!["BADS".to_string()]
    );
}

// REQ-YGAP-008.A1 (economic events)
#[tokio::test]
async fn economic_events_calendar_posts_economic_event_entity_with_startdatetime_window() {
    let server = MockServer::start();
    let expected = window_body(
        "economic_event",
        &[
            "econ_release",
            "country_code",
            "startdatetime",
            "period",
            "after_release_actual",
            "consensus_estimate",
            "prior_release_actual",
            "originally_reported_actual",
        ],
        12,
    );
    let mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH).json_body(expected);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("economic_event"));
    });

    let response = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .economic_events_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    let rows = response.data;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].event, "Nonfarm Payrolls");
    assert_eq!(rows[0].country_code.as_deref(), Some("US"));
    assert_eq!(rows[0].period.as_deref(), Some("Oct"));
    assert!(rows[0].actual.is_none());
    assert_eq!(rows[0].expected, Some(dec("60000")));
    assert_eq!(rows[1].actual, Some(dec("3.1")));
    assert_eq!(rows[1].revised_from, Some(dec("3.0")));
    assert_eq!(
        rows[1].time,
        Utc.with_ymd_and_hms(2025, 11, 4, 13, 30, 0).unwrap()
    );
    assert_eq!(response.diagnostics.len(), 1);
}

// REQ-YGAP-008 (default window = today .. today + 7 days)
#[tokio::test]
async fn calendars_default_window_is_today_plus_seven_days() {
    let server = MockServer::start();
    let today = Utc::now().date_naive();
    let gte = today.format("%Y-%m-%d").to_string();
    let lte = today
        .checked_add_days(Days::new(7))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    let mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH).json_body_includes(
            json!({"query": {"operator": "AND", "operands": [
                date_op("GTE", "startdatetime", &gte),
                date_op("LTE", "startdatetime", &lte),
            ]}})
            .to_string(),
        );
        then.status(200)
            .header("content-type", "application/json")
            .body(empty_viz_body());
    });

    let rows = CalendarsBuilder::new(&client(&server))
        .splits()
        .await
        .unwrap();

    mock.assert();
    assert_eq!(rows, [] as [yfinance_rs::StockSplitEvent; 0]);
}

// REQ-YGAP-008.A2 (limit > 100 rejected before any request)
#[tokio::test]
async fn calendars_limit_above_100_is_rejected_before_any_request() {
    let server = MockServer::start();
    let mut any = server.mock(|when, then| {
        when.any_request();
        then.status(500);
    });
    let builder = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .limit(101);

    for result in [
        builder.earnings().await.map(|_| ()),
        builder.ipos().await.map(|_| ()),
        builder.splits().await.map(|_| ()),
        builder.economic_events().await.map(|_| ()),
    ] {
        assert!(matches!(result, Err(YfError::InvalidParams(_))));
    }
    assert_eq!(any.calls(), 0);
    any.delete();

    // 100 is the documented maximum and is accepted.
    let max = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .json_body_includes(r#"{"size":100}"#);
        then.status(200)
            .header("content-type", "application/json")
            .body(empty_viz_body());
    });
    CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .limit(100)
        .splits()
        .await
        .unwrap();
    max.assert();
}

// REQ-YGAP-008.A2 (offset passed through for paging)
#[tokio::test]
async fn calendars_offset_is_passed_through_for_paging() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .json_body(earnings_body(50, 100, vec![]));
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("sp_earnings"));
    });

    let rows = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .limit(50)
        .offset(100)
        .earnings()
        .await
        .unwrap();

    mock.assert();
    assert_eq!(rows.len(), 2);

    let err = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .offset(12)
        .most_active_only(true)
        .earnings()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::InvalidParams(_)));
}

// REQ-YGAP-008.A3 (crumb-auth path, retried once on invalid crumb)
#[tokio::test]
async fn calendars_request_retries_once_with_fresh_crumb_when_crumb_is_invalid() {
    let server = MockServer::start();
    let stale = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .query_param("crumb", "stale-crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"finance":{"result":null,"error":{"code":"Unauthorized","description":"Invalid Crumb"}}}"#);
    });
    let (cookie, crumb) = crate::common::mock_cookie_crumb(&server);
    let fresh = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .query_param("crumb", "crumb-value")
            .json_body_includes(r#"{"entityIdType":"economic_event"}"#);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("economic_event"));
    });

    let client = yfinance_rs::YfClient::builder()
        .base_visualization(viz_url(&server))
        .cookie_url(Url::parse(&format!("{}/consent", server.base_url())).unwrap())
        .crumb_url(Url::parse(&format!("{}/v1/test/getcrumb", server.base_url())).unwrap())
        ._preauth("cookie", "stale-crumb")
        .build()
        .unwrap();
    let rows = CalendarsBuilder::new(&client)
        .between(nov3(), nov10())
        .economic_events()
        .await
        .unwrap();

    stale.assert_calls(1);
    cookie.assert();
    crumb.assert();
    fresh.assert_calls(1);
    assert_eq!(rows.len(), 2);
}

// REQ-YGAP-008.A3 (only one retry)
#[tokio::test]
async fn calendars_invalid_crumb_after_refresh_fails_without_second_retry() {
    let server = MockServer::start();
    let invalid = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(401);
    });
    let (_cookie, crumb) = crate::common::mock_cookie_crumb(&server);

    let err = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .splits()
        .await
        .unwrap_err();

    assert!(
        matches!(err, YfError::Status { status: 401, .. }),
        "{err:?}"
    );
    crumb.assert_calls(1);
    invalid.assert_calls(2);
}

// REQ-YGAP-008.A4 (unparseable dates dropped with warnings, siblings kept)
#[tokio::test]
async fn calendar_rows_with_unparseable_dates_are_dropped_with_warnings() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(VIZ_PATH)
            .json_body_includes(r#"{"entityIdType":"sp_earnings"}"#);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("sp_earnings"));
    });

    let response = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .earnings_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    let symbols: Vec<_> = response.data.iter().map(|r| r.symbol.as_str()).collect();
    assert_eq!(symbols, ["NVDA", "PLTR"]);
    assert_eq!(
        dropped_keys(&response.diagnostics.warnings),
        vec!["BADD".to_string()]
    );
    assert!(matches!(
        &response.diagnostics.warnings[0],
        YfWarning::DroppedItem {
            endpoint: "calendars",
            item: "earnings_event",
            ..
        }
    ));
}

// REQ-YGAP-008.A4 (strict mode turns the dropped row into an error)
#[tokio::test]
async fn calendar_unparseable_date_is_error_in_strict_mode() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(viz_fixture("splits"));
    });

    let err = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .strict()
        .splits()
        .await
        .unwrap_err();

    assert!(
        matches!(&err, YfError::DataQuality(warning) if matches!(
            warning.as_ref(),
            YfWarning::DroppedItem { key: Some(key), .. } if key == "BADS"
        )),
        "{err:?}"
    );
}

#[tokio::test]
async fn calendars_api_error_body_is_returned_as_api_error() {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(POST).path(VIZ_PATH);
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"finance":{"result":null,"error":{"code":"Bad Request","description":"entityIdType not supported, please check the request and try again later"}}}"#);
    });

    let err = CalendarsBuilder::new(&client(&server))
        .between(nov3(), nov10())
        .ipos()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::Api(message) if message.contains("Bad Request")));
}

#[tokio::test]
async fn calendars_inverted_window_is_rejected_before_any_request() {
    let server = MockServer::start();
    let any = server.mock(|when, then| {
        when.any_request();
        then.status(500);
    });
    let err = CalendarsBuilder::new(&client(&server))
        .between(nov10(), nov3())
        .splits()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::InvalidParams(_)));
    assert_eq!(any.calls(), 0);
}
