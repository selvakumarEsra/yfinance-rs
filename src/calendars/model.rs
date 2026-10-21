use chrono::{DateTime, NaiveDate, Utc};
use paft::Decimal;
use paft::money::Money;
use serde::{Deserialize, Serialize};

/// When during the trading day an earnings event is scheduled
/// (Yahoo's `startdatetimetype` column).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EarningsCallTiming {
    /// Before market open (`BMO`).
    BeforeMarketOpen,
    /// After market close (`AMC`).
    AfterMarketClose,
    /// Time not supplied by the company (`TNS`).
    TimeNotSupplied,
    /// Any other Yahoo timing code, preserved verbatim (for example `TAS`).
    Other(String),
}

impl EarningsCallTiming {
    pub(crate) fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "BMO" => Self::BeforeMarketOpen,
            "AMC" => Self::AfterMarketClose,
            "TNS" => Self::TimeNotSupplied,
            _ => Self::Other(raw.trim().to_string()),
        }
    }
}

/// Kind of per-symbol earnings event (Yahoo's `eventtype` column).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EarningsEventType {
    /// Earnings call (Yahoo code `1`).
    Call,
    /// Earnings report (Yahoo code `2`).
    Report,
    /// Stockholders meeting (Yahoo code `11`).
    StockholdersMeeting,
    /// Any other Yahoo event code, preserved verbatim.
    Other(String),
}

impl EarningsEventType {
    pub(crate) fn parse(raw: &str) -> Self {
        match raw.trim() {
            "1" => Self::Call,
            "2" => Self::Report,
            "11" => Self::StockholdersMeeting,
            other => Self::Other(other.to_string()),
        }
    }
}

/// One row of the market-wide earnings calendar
/// ([`CalendarsBuilder::earnings`](crate::calendars::CalendarsBuilder::earnings)).
///
/// Yahoo's earnings calendar is filtered to the US region and does not send a
/// currency per row; EPS and market cap are therefore reported in USD.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarningsEvent {
    /// Ticker symbol.
    pub symbol: String,
    /// Company short name.
    pub company_name: Option<String>,
    /// Intraday market capitalization (USD).
    pub market_cap: Option<Money>,
    /// Event title, for example `Q3 2025 Earnings Call`.
    pub event_name: Option<String>,
    /// Scheduled event start (UTC).
    pub start: DateTime<Utc>,
    /// Session timing (before open / after close / not supplied).
    pub timing: Option<EarningsCallTiming>,
    /// Consensus EPS estimate (USD).
    pub eps_estimate: Option<Money>,
    /// Reported EPS (USD); `None` until the company reports.
    pub eps_actual: Option<Money>,
    /// EPS surprise in percentage points (for example `4.5` = 4.5 %).
    pub surprise_percent: Option<Decimal>,
}

/// One row of the IPO calendar ([`CalendarsBuilder::ipos`](crate::calendars::CalendarsBuilder::ipos)).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpoEvent {
    /// Ticker symbol.
    pub symbol: String,
    /// Company short name.
    pub company_name: Option<String>,
    /// Short exchange name, for example `NASDAQ`.
    pub exchange: Option<String>,
    /// Date the IPO was filed.
    pub filing_date: Option<NaiveDate>,
    /// Expected or actual listing date (UTC).
    pub date: Option<DateTime<Utc>>,
    /// Date the filing was last amended.
    pub amended_date: Option<NaiveDate>,
    /// Lower bound of the marketed price range.
    pub price_from: Option<Money>,
    /// Upper bound of the marketed price range.
    pub price_to: Option<Money>,
    /// Final offer price, once priced.
    pub offer_price: Option<Money>,
    /// Number of shares offered.
    pub shares: Option<u64>,
    /// Yahoo deal status, for example `Expected`, `Priced`, `Withdrawn`.
    pub deal_type: Option<String>,
}

/// One row of the stock-splits calendar ([`CalendarsBuilder::splits`](crate::calendars::CalendarsBuilder::splits)).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockSplitEvent {
    /// Ticker symbol.
    pub symbol: String,
    /// Company short name.
    pub company_name: Option<String>,
    /// Date the split is payable (UTC).
    pub payable_on: DateTime<Utc>,
    /// Whether the security is optionable.
    pub optionable: Option<bool>,
    /// Shares held before the split (the `old_share_worth` side of the ratio).
    pub old_shares: Option<Decimal>,
    /// Shares held after the split (the `share_worth` side of the ratio).
    pub new_shares: Option<Decimal>,
}

/// One row of the economic-events calendar
/// ([`CalendarsBuilder::economic_events`](crate::calendars::CalendarsBuilder::economic_events)).
///
/// Values are in the release's own unit (percent, index points, counts, …), which
/// Yahoo does not send, so they are exposed as plain decimals.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicEvent {
    /// Release name, for example `Nonfarm Payrolls`.
    pub event: String,
    /// Country or region code, for example `US`.
    pub country_code: Option<String>,
    /// Scheduled release time (UTC).
    pub time: DateTime<Utc>,
    /// Reference period, for example `Oct`.
    pub period: Option<String>,
    /// Actual value after release.
    pub actual: Option<Decimal>,
    /// Market consensus expectation.
    pub expected: Option<Decimal>,
    /// Prior release value.
    pub prior: Option<Decimal>,
    /// Originally reported value of the prior release, when revised.
    pub revised_from: Option<Decimal>,
}

/// One per-symbol earnings date ([`EarningsDatesBuilder`](crate::calendars::EarningsDatesBuilder)).
///
/// Future rows have no reported EPS or surprise.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarningsDate {
    /// Event start (UTC).
    pub date: DateTime<Utc>,
    /// Exchange timezone abbreviation sent by Yahoo, for example `EDT`.
    pub timezone_short_name: Option<String>,
    /// Session timing (before open / after close / not supplied).
    pub timing: Option<EarningsCallTiming>,
    /// Event title, for example `Q3 2025 Earnings Release`.
    pub event_name: Option<String>,
    /// Event kind (call, report, stockholders meeting).
    pub event_type: Option<EarningsEventType>,
    /// Consensus EPS estimate in the resolved estimate currency.
    pub eps_estimate: Option<Money>,
    /// Reported EPS in the resolved estimate currency; `None` for future events.
    pub eps_actual: Option<Money>,
    /// EPS surprise in percentage points; `None` for future events.
    pub surprise_percent: Option<Decimal>,
}
