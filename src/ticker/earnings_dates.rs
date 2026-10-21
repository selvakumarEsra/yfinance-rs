use super::Ticker;
use crate::calendars::{DEFAULT_EARNINGS_DATES_LIMIT, EarningsDate, EarningsDatesBuilder};
use crate::core::{YfError, YfResponse};

impl Ticker {
    fn earnings_dates_builder(&self, limit: Option<u32>) -> EarningsDatesBuilder {
        EarningsDatesBuilder::new(&self.client, &self.symbol)
            .limit(limit.unwrap_or(DEFAULT_EARNINGS_DATES_LIMIT))
            .cache_mode(self.options.cache_mode())
            .data_quality(self.options.data_quality())
            .retry_policy(self.options.retry_override().cloned())
    }

    /// Fetches past and future earnings dates for the ticker, newest first.
    ///
    /// `limit` caps the number of rows (default 12, maximum 100). Data comes from
    /// Yahoo's visualization API (`entityIdType=earnings`); there is no HTML-scraping
    /// fallback, so an empty Yahoo result yields an empty list.
    ///
    /// # Errors
    ///
    /// Returns [`YfError::InvalidParams`] for a limit of 0 or above 100 (before any
    /// request), or an error if the request fails or Yahoo returns an API error.
    pub async fn earnings_dates(&self, limit: Option<u32>) -> Result<Vec<EarningsDate>, YfError> {
        self.earnings_dates_builder(limit).fetch().await
    }

    /// Fetches earnings dates with projection diagnostics.
    ///
    /// An empty Yahoo result returns an empty list plus a provider-feature-unavailable
    /// warning; in strict mode it is an error.
    ///
    /// # Errors
    ///
    /// As [`earnings_dates`](Self::earnings_dates); additionally fails in strict mode
    /// when a row cannot be projected losslessly or Yahoo returns no rows.
    pub async fn earnings_dates_with_diagnostics(
        &self,
        limit: Option<u32>,
    ) -> Result<YfResponse<Vec<EarningsDate>>, YfError> {
        self.earnings_dates_builder(limit)
            .fetch_with_diagnostics()
            .await
    }
}
