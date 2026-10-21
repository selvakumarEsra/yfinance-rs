use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use paft::Decimal;
use paft::domain::AssetKind;
use serde::{Deserialize, Serialize};

/// Market regions accepted by Yahoo's market time and market summary endpoints.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegion {
    /// United States equity markets.
    US,
    /// United Kingdom markets.
    GB,
    /// Asian markets.
    Asia,
    /// European markets.
    Europe,
    /// Interest rates (treasury yields).
    Rates,
    /// Commodity futures.
    Commodities,
    /// Currency pairs.
    Currencies,
    /// Cryptocurrencies.
    Cryptocurrencies,
}

impl MarketRegion {
    /// Every supported region, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::US,
        Self::GB,
        Self::Asia,
        Self::Europe,
        Self::Rates,
        Self::Commodities,
        Self::Currencies,
        Self::Cryptocurrencies,
    ];

    /// Yahoo's `market` query-parameter code for this region (for example `US` or `EUROPE`).
    #[must_use]
    pub const fn yahoo_code(self) -> &'static str {
        match self {
            Self::US => "US",
            Self::GB => "GB",
            Self::Asia => "ASIA",
            Self::Europe => "EUROPE",
            Self::Rates => "RATES",
            Self::Commodities => "COMMODITIES",
            Self::Currencies => "CURRENCIES",
            Self::Cryptocurrencies => "CRYPTOCURRENCIES",
        }
    }
}

/// Open/closed state of a market, from Yahoo's `v6/finance/markettime` endpoint.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketStatus {
    /// Yahoo market identifier (for example `us`).
    pub market_id: Option<String>,
    /// Display name (for example `U.S. markets`).
    pub name: Option<String>,
    /// Yahoo market state token (for example `open` or `closed`).
    pub status: Option<String>,
    /// Human-readable status message.
    pub message: Option<String>,
    /// Session open time, in UTC.
    pub open: Option<DateTime<Utc>>,
    /// Session close time, in UTC.
    pub close: Option<DateTime<Utc>>,
    /// Exchange timezone, when Yahoo sends a valid IANA name.
    pub timezone: Option<Tz>,
    /// Timezone abbreviation (for example `EDT`).
    pub timezone_abbreviation: Option<String>,
    /// Offset from UTC in seconds at the reported time.
    pub utc_offset_seconds: Option<i32>,
    /// Server time at which Yahoo produced this status.
    pub as_of: Option<DateTime<Utc>>,
}

impl MarketStatus {
    /// Returns `Some(true)` when Yahoo reports the market as open, `Some(false)`
    /// when it reports any other state, and `None` when the state is missing.
    #[must_use]
    pub fn is_open(&self) -> Option<bool> {
        self.status
            .as_deref()
            .map(|status| status.trim().eq_ignore_ascii_case("open"))
    }
}

/// One index or benchmark quote from Yahoo's `v6/finance/quote/marketSummary` endpoint.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketSummaryEntry {
    /// Yahoo symbol (for example `^GSPC`).
    pub symbol: String,
    /// Display name.
    pub name: Option<String>,
    /// Yahoo exchange code (for example `SNP`).
    pub exchange: Option<String>,
    /// Full exchange display name.
    pub exchange_name: Option<String>,
    /// Asset kind derived from Yahoo's quote type.
    pub kind: Option<AssetKind>,
    /// Yahoo market state token (for example `REGULAR` or `CLOSED`).
    pub market_state: Option<String>,
    /// Last regular-market price or level, in the instrument's own unit.
    pub price: Decimal,
    /// Absolute regular-market change.
    pub change: Option<Decimal>,
    /// Regular-market change, in percentage points.
    pub change_percent: Option<Decimal>,
    /// Quote currency code, when Yahoo includes it.
    pub currency: Option<String>,
    /// Time of the last regular-market price.
    pub market_time: Option<DateTime<Utc>>,
}
