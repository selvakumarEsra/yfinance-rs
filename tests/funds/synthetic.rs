//! Synthetic tests for fund / ETF data (REQ-YGAP-003).
//!
//! NOTE: `tests/fixtures/funds_api_quoteType-summaryProfile-topHoldings-fundProfile_SPY.json`
//! is a SYNTHETIC fixture. Yahoo rate-limited the recording machine, so its shape is derived
//! from Python yfinance's fund parser (`scrapers/funds.py`) plus the recorded `fundProfile`
//! shape in the existing QQQ profile fixture. Re-record live when possible.

use httpmock::{Method::GET, Mock, MockServer};
use paft::Decimal;
use url::Url;
use yfinance_rs::{
    FundSector, FundsDataBuilder, ProjectionIssue, Ticker, YfClient, YfError, YfWarning,
};

const MODULES: &str = "quoteType,summaryProfile,topHoldings,fundProfile";
const FIXTURE_ENDPOINT: &str = "funds_api_quoteType-summaryProfile-topHoldings-fundProfile";

fn preauthed_client(server: &MockServer) -> YfClient {
    YfClient::builder()
        .base_quote_api(
            Url::parse(&format!("{}/v10/finance/quoteSummary/", server.base_url())).unwrap(),
        )
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

fn mock_funds<'a>(server: &'a MockServer, sym: &str, body: String) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/v10/finance/quoteSummary/{sym}"))
            .query_param("modules", MODULES)
            .query_param("crumb", "crumb");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    })
}

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn spy_fixture() -> String {
    crate::common::fixture(FIXTURE_ENDPOINT, "SPY", "json")
}

#[tokio::test]
async fn etf_funds_data_populates_holdings_sectors_and_asset_classes() {
    let server = MockServer::start();
    let mock = mock_funds(&server, "SPY", spy_fixture());

    let client = preauthed_client(&server);
    let response = Ticker::new(&client, "SPY")
        .funds_data_with_diagnostics()
        .await
        .unwrap();

    mock.assert();
    assert!(
        response.diagnostics.warnings.is_empty(),
        "{:?}",
        response.diagnostics.warnings
    );
    let fund = response.data;

    assert_eq!(fund.quote_type.as_deref(), Some("ETF"));
    assert!(
        fund.description
            .as_deref()
            .is_some_and(|d| d.starts_with("The Trust seeks"))
    );
    assert_eq!(fund.overview.category.as_deref(), Some("Large Blend"));
    assert_eq!(
        fund.overview.family.as_deref(),
        Some("State Street Investment Management")
    );
    assert_eq!(
        fund.overview.legal_type.as_deref(),
        Some("Exchange Traded Fund")
    );

    let symbols: Vec<&str> = fund
        .top_holdings
        .iter()
        .map(|h| h.symbol.as_str())
        .collect();
    assert_eq!(symbols, ["NVDA", "MSFT", "AAPL", "AMZN", "META"]);
    assert_eq!(fund.top_holdings[0].name.as_deref(), Some("NVIDIA Corp"));

    assert_eq!(fund.sector_weightings.len(), 11);
    assert_eq!(fund.sector_weightings[4].sector, FundSector::Technology);
    assert_eq!(fund.sector_weightings[4].weight, dec("0.3417"));

    assert_eq!(fund.asset_classes.stock, Some(dec("0.9991")));
    assert_eq!(fund.asset_classes.cash, Some(dec("0.0009")));
    assert_eq!(fund.asset_classes.bond, Some(Decimal::ZERO));

    assert_eq!(fund.bond_ratings.len(), 4);
    assert_eq!(fund.bond_ratings[3].rating, "us_government");

    assert_eq!(
        fund.equity_holdings.fund.three_year_earnings_growth,
        Some(dec("0.1283"))
    );
    assert!(fund.equity_holdings.fund.median_market_cap.is_none());
    assert_eq!(
        fund.equity_holdings.category_average.price_to_earnings,
        Some(dec("0.0415"))
    );
    assert!(fund.bond_holdings.fund.duration.is_none());

    assert_eq!(
        fund.operations.fund.total_net_assets,
        Some(dec("672559.19"))
    );
    assert!(fund.operations.category_average.total_net_assets.is_none());
}

#[tokio::test]
async fn funds_data_weights_and_percentages_are_fractions() {
    let server = MockServer::start();
    let _mock = mock_funds(&server, "SPY", spy_fixture());

    let fund = FundsDataBuilder::new(&preauthed_client(&server), "SPY")
        .fetch()
        .await
        .unwrap();

    let one = Decimal::ONE;
    let in_unit = |d: &Decimal| *d >= Decimal::ZERO && *d <= one;

    // Yahoo `fmt` says 7.81% -> fraction 0.0781, never 7.81.
    assert_eq!(fund.top_holdings[0].weight, Some(dec("0.0781")));
    assert!(
        fund.top_holdings
            .iter()
            .all(|h| h.weight.as_ref().is_some_and(in_unit))
    );
    assert!(fund.sector_weightings.iter().all(|s| in_unit(&s.weight)));
    assert!(fund.bond_ratings.iter().all(|r| in_unit(&r.weight)));
    let sector_total: Decimal = fund.sector_weightings.iter().map(|s| s.weight).sum();
    assert!(sector_total <= one, "sector total {sector_total}");

    // Expense ratio 0.09% -> 0.000945; turnover 3.00% -> 0.03.
    assert_eq!(fund.operations.fund.expense_ratio, Some(dec("0.000945")));
    assert_eq!(fund.operations.fund.turnover, Some(dec("0.03")));
    assert_eq!(
        fund.operations.category_average.turnover,
        Some(dec("0.5021"))
    );

    let classes = &fund.asset_classes;
    for value in [
        classes.cash,
        classes.stock,
        classes.bond,
        classes.preferred,
        classes.convertible,
        classes.other,
    ] {
        assert!(value.as_ref().is_some_and(in_unit));
    }
}

const UNKNOWN_SECTOR_BODY: &str = r#"{
  "quoteSummary": {
    "result": [{
      "quoteType": { "quoteType": "MUTUALFUND" },
      "summaryProfile": { "longBusinessSummary": "A fund." },
      "fundProfile": { "categoryName": "Allocation", "family": "Example", "legalType": "Open Ended Investment Company" },
      "topHoldings": {
        "stockPosition": { "raw": 0.6 },
        "holdings": [],
        "sectorWeightings": [
          { "technology": { "raw": 0.4 } },
          { "realestate": { "raw": 0.1 } },
          { "space_exploration": { "raw": 0.05 } }
        ]
      }
    }],
    "error": null
  }
}"#;

#[tokio::test]
async fn unknown_sector_keys_are_kept_and_known_keys_map_to_stable_names() {
    let server = MockServer::start();
    let _mock = mock_funds(&server, "MIXFX", UNKNOWN_SECTOR_BODY.to_string());

    let response = FundsDataBuilder::new(&preauthed_client(&server), "MIXFX")
        .fetch_with_diagnostics()
        .await
        .unwrap();

    let sectors = &response.data.sector_weightings;
    assert_eq!(sectors.len(), 3);
    assert_eq!(sectors[0].sector, FundSector::Technology);
    assert_eq!(sectors[0].sector.as_str(), "technology");
    assert_eq!(sectors[1].sector, FundSector::RealEstate);
    assert_eq!(sectors[1].sector.as_str(), "real_estate");
    assert_eq!(
        sectors[2].sector,
        FundSector::Other("space_exploration".to_string())
    );
    assert_eq!(sectors[2].sector.to_string(), "space_exploration");
    assert_eq!(sectors[2].weight, dec("0.05"));
    assert_eq!(response.data.top_holdings.len(), 0);

    assert_eq!(
        FundSector::from_yahoo_key("consumer_cyclical"),
        FundSector::ConsumerCyclical
    );
    assert_eq!(
        FundSector::from_yahoo_key("financial_services"),
        FundSector::FinancialServices
    );
}

const EQUITY_BODY: &str = r#"{
  "quoteSummary": {
    "result": [{
      "quoteType": { "quoteType": "EQUITY", "symbol": "AAPL" },
      "summaryProfile": { "longBusinessSummary": "Apple designs phones." }
    }],
    "error": null
  }
}"#;

#[tokio::test]
async fn non_fund_symbol_returns_error_naming_symbol_and_quote_type() {
    let server = MockServer::start();
    let mock = mock_funds(&server, "AAPL", EQUITY_BODY.to_string());
    let client = preauthed_client(&server);

    let err = Ticker::new(&client, "AAPL").funds_data().await.unwrap_err();
    let message = err.to_string();
    assert!(matches!(err, YfError::MissingData(_)), "{err:?}");
    assert!(message.contains("AAPL"), "{message}");
    assert!(message.contains("EQUITY"), "{message}");

    let err = FundsDataBuilder::new(&client, "AAPL")
        .strict()
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::MissingData(_)), "{err:?}");
    mock.assert_calls(2);
}

const PARTIAL_FUND_BODY: &str = r#"{
  "quoteSummary": {
    "result": [{
      "quoteType": { "quoteType": "ETF" },
      "topHoldings": {
        "holdings": [
          { "symbol": "AAA", "holdingName": "Alpha", "holdingPercent": { "raw": 0.2 } },
          { "holdingName": "No symbol", "holdingPercent": { "raw": 0.1 } }
        ],
        "sectorWeightings": [ { "energy": { "raw": "lots" } } ]
      }
    }],
    "error": null
  }
}"#;

#[tokio::test]
async fn partial_fund_response_warns_in_best_effort_and_fails_in_strict() {
    let server = MockServer::start();
    let mock = mock_funds(&server, "PART", PARTIAL_FUND_BODY.to_string());
    let client = preauthed_client(&server);

    let response = FundsDataBuilder::new(&client, "PART")
        .fetch_with_diagnostics()
        .await
        .unwrap();
    assert_eq!(response.data.top_holdings.len(), 1);
    assert_eq!(response.data.sector_weightings.len(), 0);
    let warnings = &response.diagnostics.warnings;
    assert!(warnings.iter().any(|w| matches!(
        w,
        YfWarning::ProviderFeatureUnavailable {
            feature: "fundProfile",
            ..
        }
    )));
    assert!(warnings.iter().any(|w| matches!(
        w,
        YfWarning::DroppedItem {
            item: "fund_holding",
            reason: ProjectionIssue::MissingRequiredField { field: "symbol" },
            ..
        }
    )));
    assert!(warnings.iter().any(|w| matches!(
        w,
        YfWarning::DroppedItem {
            item: "fund_sector_weighting",
            key: Some(key),
            ..
        } if key == "energy"
    )));

    let err = FundsDataBuilder::new(&client, "PART")
        .strict()
        .fetch()
        .await
        .unwrap_err();
    assert!(matches!(err, YfError::DataQuality(_)));
    mock.assert_calls(2);
}
