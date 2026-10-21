//! Valuation measures (market cap, enterprise value, P/E, P/S, P/B, EV multiples)
//! from Yahoo's fundamentals-timeseries endpoint.

use chrono::NaiveDate;
use paft::Decimal;
use paft::money::{Currency, Money};
use serde::{Deserialize, Serialize};

use crate::core::{CallOptions, YfClient, YfError, YfResponse};

use super::api;

/// Period granularity of a valuation-measures series.
///
/// Each variant maps to a Yahoo fundamentals-timeseries key prefix
/// (`quarterlyMarketCap`, `monthlyPeRatio`, `annualPsRatio`, `trailingPegRatio`, ...).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ValuationFrequency {
    /// Quarter-end values (Yahoo `quarterly*` keys). This is the default, matching
    /// Python yfinance's `Ticker.valuation`.
    #[default]
    Quarterly,
    /// Month-end values (Yahoo `monthly*` keys).
    Monthly,
    /// Fiscal-year-end values (Yahoo `annual*` keys).
    Annual,
    /// Latest trailing values (Yahoo `trailing*` keys); the "Current" column on Yahoo's
    /// key-statistics page.
    Trailing,
}

impl ValuationFrequency {
    /// Returns the Yahoo fundamentals-timeseries key prefix for this frequency.
    #[must_use]
    pub const fn timeseries_prefix(self) -> &'static str {
        match self {
            Self::Quarterly => "quarterly",
            Self::Monthly => "monthly",
            Self::Annual => "annual",
            Self::Trailing => "trailing",
        }
    }
}

/// Valuation measures for one as-of date.
///
/// Every measure is optional: Yahoo omits measures that are not meaningful for a
/// security or period (for example PEG or EV/EBITDA for some financials).
/// Ratios are unitless decimals; monetary values carry their currency.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValuationRow {
    /// The date these measures were taken (Yahoo `asOfDate`).
    pub as_of: NaiveDate,
    /// Market capitalization (`MarketCap`).
    pub market_cap: Option<Money>,
    /// Enterprise value (`EnterpriseValue`).
    pub enterprise_value: Option<Money>,
    /// Trailing price/earnings ratio (`PeRatio`).
    pub trailing_pe: Option<Decimal>,
    /// Forward price/earnings ratio (`ForwardPeRatio`).
    pub forward_pe: Option<Decimal>,
    /// PEG ratio, 5-year expected (`PegRatio`).
    pub peg_ratio: Option<Decimal>,
    /// Price/sales ratio (`PsRatio`).
    pub price_to_sales: Option<Decimal>,
    /// Price/book ratio (`PbRatio`).
    pub price_to_book: Option<Decimal>,
    /// Enterprise value/revenue (`EnterprisesValueRevenueRatio`).
    pub enterprise_value_to_revenue: Option<Decimal>,
    /// Enterprise value/EBITDA (`EnterprisesValueEBITDARatio`).
    pub enterprise_value_to_ebitda: Option<Decimal>,
}

/// A builder for fetching valuation measures history for a symbol.
///
/// Rows are returned one per as-of date, newest first.
pub struct ValuationBuilder {
    client: YfClient,
    symbol: String,
    frequency: ValuationFrequency,
    override_currency: Option<Currency>,
    chunk_size: usize,
    options: CallOptions,
}

impl ValuationBuilder {
    /// Creates a new `ValuationBuilder` for a given symbol (quarterly frequency).
    pub fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            symbol: symbol.into(),
            frequency: ValuationFrequency::default(),
            override_currency: None,
            chunk_size: api::TIMESERIES_MAX_KEYS_PER_REQUEST,
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!();

    /// Sets the period granularity (default: [`ValuationFrequency::Quarterly`]).
    #[must_use]
    pub const fn frequency(mut self, frequency: ValuationFrequency) -> Self {
        self.frequency = frequency;
        self
    }

    /// Overrides the currency of monetary measures (market cap, enterprise value).
    ///
    /// Pass `None` (the default) to use Yahoo's `currencyCode`, falling back to the
    /// symbol's resolved trading currency.
    #[must_use]
    pub fn override_currency(mut self, currency: Option<Currency>) -> Self {
        self.override_currency = currency;
        self
    }

    /// (Internal testing only) Overrides the maximum number of keys per timeseries request.
    #[cfg(any(test, feature = "test-mode"))]
    #[doc(hidden)]
    #[must_use]
    pub const fn _timeseries_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    /// Fetches valuation measures, newest first.
    ///
    /// # Errors
    ///
    /// Returns a `YfError` if the network request fails or the API response cannot be parsed.
    pub async fn fetch(&self) -> Result<Vec<ValuationRow>, YfError> {
        Ok(self.fetch_with_diagnostics().await?.into_data())
    }

    /// Fetches valuation measures with projection diagnostics, newest first.
    ///
    /// # Errors
    ///
    /// Returns a `YfError` if the request fails or strict data-quality mode rejects a projection issue.
    pub async fn fetch_with_diagnostics(&self) -> Result<YfResponse<Vec<ValuationRow>>, YfError> {
        api::valuation_measures(
            &self.client,
            &self.symbol,
            self.frequency,
            self.override_currency.clone(),
            &self.options,
            self.chunk_size,
        )
        .await
    }
}
