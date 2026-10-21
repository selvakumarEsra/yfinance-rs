//! `Ticker` convenience methods for SEC filings, fund data and earnings history.

use paft::money::Currency;

use super::Ticker;
use crate::{
    analysis::EarningsHistoryRow,
    core::{YfError, YfResponse},
    filings::{SecFiling, SecFilingsBuilder},
    funds::{FundsData, FundsDataBuilder},
};

impl Ticker {
    fn sec_filings_builder(&self) -> SecFilingsBuilder {
        SecFilingsBuilder::new(&self.client, &self.symbol)
            .cache_mode(self.options.cache_mode())
            .data_quality(self.options.data_quality())
            .retry_policy(self.options.retry_override().cloned())
    }

    fn funds_data_builder(&self) -> FundsDataBuilder {
        FundsDataBuilder::new(&self.client, &self.symbol)
            .cache_mode(self.options.cache_mode())
            .data_quality(self.options.data_quality())
            .retry_policy(self.options.retry_override().cloned())
    }

    /// Fetches the issuer's SEC filings (date, form type, title, EDGAR URL, exhibits).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or strict data-quality mode rejects a projection issue.
    pub async fn sec_filings(&self) -> Result<Vec<SecFiling>, YfError> {
        self.sec_filings_builder().fetch().await
    }

    /// Fetches the issuer's SEC filings with projection diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or strict data-quality mode rejects a projection issue.
    pub async fn sec_filings_with_diagnostics(
        &self,
    ) -> Result<YfResponse<Vec<SecFiling>>, YfError> {
        self.sec_filings_builder().fetch_with_diagnostics().await
    }

    /// Fetches ETF / mutual-fund data: description, overview, operations, asset classes,
    /// top holdings, equity/bond statistics, bond ratings and sector weightings.
    ///
    /// All weights and percentages are fractions in `0.0..=1.0`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the symbol is not a fund, or strict
    /// data-quality mode rejects a projection issue.
    pub async fn funds_data(&self) -> Result<FundsData, YfError> {
        self.funds_data_builder().fetch().await
    }

    /// Fetches ETF / mutual-fund data with projection diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the symbol is not a fund, or strict
    /// data-quality mode rejects a projection issue.
    pub async fn funds_data_with_diagnostics(&self) -> Result<YfResponse<FundsData>, YfError> {
        self.funds_data_builder().fetch_with_diagnostics().await
    }

    /// Fetches past quarters' EPS actual, estimate, difference and surprise, oldest first.
    ///
    /// Provide `Some(currency)` to override the auto-resolved currency for this call;
    /// pass `None` to use Yahoo's reported currency and infer only when needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or strict data-quality mode rejects a projection issue.
    pub async fn earnings_history(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<Vec<EarningsHistoryRow>, YfError> {
        self.analysis_builder()
            .earnings_history(override_currency)
            .await
    }

    /// Fetches earnings history with projection diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or strict data-quality mode rejects a projection issue.
    pub async fn earnings_history_with_diagnostics(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<YfResponse<Vec<EarningsHistoryRow>>, YfError> {
        self.analysis_builder()
            .earnings_history_with_diagnostics(override_currency)
            .await
    }
}
