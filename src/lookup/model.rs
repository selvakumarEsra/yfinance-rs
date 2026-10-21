use paft::Decimal;
use paft::domain::{AssetKind, Exchange, Instrument};
use serde::{Deserialize, Serialize};

/// Asset-type filter for Yahoo's `v1/finance/lookup` endpoint.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LookupKind {
    /// All instrument types.
    #[default]
    All,
    /// Stocks.
    Equity,
    /// Mutual funds.
    MutualFund,
    /// Exchange-traded funds.
    ETF,
    /// Indices.
    Index,
    /// Futures.
    Future,
    /// Currencies.
    Currency,
    /// Cryptocurrencies.
    Cryptocurrency,
}

impl LookupKind {
    /// Every supported kind, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::All,
        Self::Equity,
        Self::MutualFund,
        Self::ETF,
        Self::Index,
        Self::Future,
        Self::Currency,
        Self::Cryptocurrency,
    ];

    /// Yahoo's `type` query-parameter value for this kind (for example `etf`).
    #[must_use]
    pub const fn yahoo_code(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Equity => "equity",
            Self::MutualFund => "mutualfund",
            Self::ETF => "etf",
            Self::Index => "index",
            Self::Future => "future",
            Self::Currency => "currency",
            Self::Cryptocurrency => "cryptocurrency",
        }
    }
}

/// One instrument returned by a symbol lookup.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookupResult {
    /// Yahoo symbol.
    pub symbol: String,
    /// Provider-agnostic instrument identity, when the symbol and quote type are valid.
    pub instrument: Option<Instrument>,
    /// Display name.
    pub name: Option<String>,
    /// Asset kind derived from Yahoo's quote type.
    pub kind: Option<AssetKind>,
    /// Raw Yahoo quote type token (for example `equity`).
    pub quote_type: Option<String>,
    /// Provider-agnostic exchange, when parsable.
    pub exchange: Option<Exchange>,
    /// Raw Yahoo exchange code (for example `NMS`).
    pub raw_exchange: Option<String>,
    /// Industry name, when Yahoo includes it.
    pub industry: Option<String>,
    /// Last regular-market price, when Yahoo includes pricing data.
    pub last_price: Option<Decimal>,
    /// Absolute regular-market change, when Yahoo includes pricing data.
    pub change: Option<Decimal>,
    /// Regular-market change, in percentage points.
    pub change_percent: Option<Decimal>,
}
