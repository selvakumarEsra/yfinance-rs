//! Shared helpers for the calendars tests (fixtures are synthetic; see `tests/calendars.rs`).

use httpmock::MockServer;
use serde_json::{Value, json};
use url::Url;
use yfinance_rs::{Decimal, Money, YfClient};

pub const VIZ_PATH: &str = "/v1/finance/visualization";

pub fn viz_fixture(entity: &str) -> String {
    crate::common::fixture("calendars_api_visualization", entity, "json")
}

pub fn earnings_dates_fixture(symbol: &str) -> String {
    crate::common::fixture("earnings_dates_api_visualization", symbol, "json")
}

pub fn viz_url(server: &MockServer) -> Url {
    Url::parse(&format!("{}{VIZ_PATH}", server.base_url())).unwrap()
}

/// A preauthed client whose visualization base points at the mock server.
pub fn client(server: &MockServer) -> YfClient {
    YfClient::builder()
        .base_visualization(viz_url(server))
        .cookie_url(Url::parse(&format!("{}/consent", server.base_url())).unwrap())
        .crumb_url(Url::parse(&format!("{}/v1/test/getcrumb", server.base_url())).unwrap())
        ._preauth("cookie", "crumb")
        .build()
        .unwrap()
}

pub fn dec(value: &str) -> Decimal {
    value.parse().unwrap()
}

pub fn amount(money: Option<&Money>) -> Option<Decimal> {
    money.map(Money::amount)
}

pub fn date_op(op: &str, field: &str, date: &str) -> Value {
    json!({"operator": op, "operands": [field, date]})
}

pub const fn empty_viz_body() -> &'static str {
    r#"{"finance":{"result":[{"documents":[{"columns":[],"rows":[]}]}],"error":null}}"#
}
