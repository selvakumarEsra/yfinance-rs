//! Shared helpers for synthetic fundamentals-timeseries responses.
//!
//! The JSON built here is SYNTHETIC (Yahoo blocks live recording from the dev machine),
//! but mirrors the recorded `tests/fixtures/timeseries_*` shape exactly: one result item
//! per key with `meta.symbol` / `meta.type`, a `timestamp` array, and a value array of
//! `{asOfDate, currencyCode, dataId, periodType, reportedValue: {fmt, raw}}` points.

use httpmock::MockServer;
use serde_json::{Value, json};
use url::Url;
use yfinance_rs::YfClient;

pub const TIMESERIES_PATH: &str = "/ws/fundamentals-timeseries/v1/finance/timeseries";

/// One `(asOfDate, unix timestamp, raw value)` data point.
pub type Point = (&'static str, i64, Value);

pub fn client_for(server: &MockServer) -> YfClient {
    YfClient::builder()
        .base_timeseries(Url::parse(&format!("{}{TIMESERIES_PATH}/", server.base_url())).unwrap())
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

/// Builds one timeseries result item for `key` (already prefixed, e.g. `trailingTotalRevenue`).
pub fn item(
    symbol: &str,
    key: &str,
    period_type: &str,
    currency: Option<&str>,
    points: &[Point],
) -> Value {
    let values: Vec<Value> = points
        .iter()
        .map(|(as_of, _, raw)| {
            let mut point = json!({
                "asOfDate": as_of,
                "dataId": 20100,
                "periodType": period_type,
                "reportedValue": { "fmt": raw.to_string(), "raw": raw }
            });
            if let Some(code) = currency {
                point["currencyCode"] = json!(code);
            }
            point
        })
        .collect();
    let timestamps: Vec<i64> = points.iter().map(|(_, ts, _)| *ts).collect();
    json!({
        "meta": { "symbol": [symbol], "type": [key] },
        key: values,
        "timestamp": timestamps
    })
}

/// A metadata-only item, as Yahoo returns for requested keys with no data.
pub fn empty_item(symbol: &str, key: &str) -> Value {
    json!({ "meta": { "symbol": [symbol], "type": [key] } })
}

pub fn envelope(items: &[Value]) -> String {
    json!({ "timeseries": { "error": null, "result": items } }).to_string()
}

/// Comma-joined `type=` query value for `prefix` + `keys`.
pub fn type_param(prefix: &str, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| format!("{prefix}{key}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub const INCOME_KEYS: [&str; 7] = [
    "TotalRevenue",
    "GrossProfit",
    "OperatingIncome",
    "NetIncome",
    "InterestExpense",
    "TaxProvision",
    "DepreciationAndAmortization",
];

pub const BALANCE_SHEET_KEYS: [&str; 14] = [
    "TotalAssets",
    "TotalLiabilitiesNetMinorityInterest",
    "StockholdersEquity",
    "CashAndCashEquivalents",
    "LongTermDebt",
    "OrdinarySharesNumber",
    "CurrentAssets",
    "CurrentLiabilities",
    "AccountsReceivable",
    "Inventory",
    "AccountsPayable",
    "NetPPE",
    "Goodwill",
    "OtherIntangibleAssets",
];

pub const CASHFLOW_KEYS: [&str; 5] = [
    "OperatingCashFlow",
    "CapitalExpenditure",
    "FreeCashFlow",
    "NetIncome",
    "DepreciationAndAmortization",
];

/// Yahoo valuation-measure keys, verified against Python yfinance `scrapers/quote.py`
/// `_VALUATION_MEASURE_LABELS`.
pub const VALUATION_KEYS: [&str; 9] = [
    "MarketCap",
    "EnterpriseValue",
    "PeRatio",
    "ForwardPeRatio",
    "PegRatio",
    "PsRatio",
    "PbRatio",
    "EnterprisesValueRevenueRatio",
    "EnterprisesValueEBITDARatio",
];

pub fn dec(value: &str) -> paft::Decimal {
    value.parse().unwrap()
}
