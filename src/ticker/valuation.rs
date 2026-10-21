//! `Ticker` convenience methods for trailing-twelve-month statements and valuation measures.

use paft::fundamentals::statements::{CashflowRow, IncomeStatementRow};
use paft::money::Currency;

use super::Ticker;
use crate::core::{YfError, YfResponse};
use crate::fundamentals::{ValuationBuilder, ValuationRow};

impl Ticker {
    /// Returns a [`ValuationBuilder`] configured with this ticker's call options.
    #[must_use]
    pub fn valuation_builder(&self) -> ValuationBuilder {
        ValuationBuilder::new(&self.client, &self.symbol)
            .cache_mode(self.options.cache_mode())
            .data_quality(self.options.data_quality())
            .retry_policy(self.options.retry_override().cloned())
    }

    /// Fetches the trailing-twelve-month (TTM) income statement.
    ///
    /// Uses Yahoo's `trailing*` timeseries keys; normally a single row is returned.
    /// Provide `Some(currency)` to override the auto-resolved reporting currency for this call;
    /// pass `None` to enrich currency metadata from Yahoo and infer only when needed.
    ///
    /// # Errors
    ///
    /// This method will return an error if the request fails or the response cannot be parsed.
    pub async fn ttm_income_stmt(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<Vec<IncomeStatementRow>, YfError> {
        Ok(self
            .ttm_income_stmt_with_diagnostics(override_currency)
            .await?
            .into_data())
    }

    /// Fetches the trailing-twelve-month (TTM) income statement with projection diagnostics.
    ///
    /// # Errors
    ///
    /// This method will return an error if the request fails or strict data-quality mode
    /// rejects a projection issue.
    pub async fn ttm_income_stmt_with_diagnostics(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<YfResponse<Vec<IncomeStatementRow>>, YfError> {
        self.fundamentals_builder()
            .ttm_income_statement_with_diagnostics(override_currency)
            .await
    }

    /// Fetches the trailing-twelve-month (TTM) cash flow statement.
    ///
    /// Uses Yahoo's `trailing*` timeseries keys; normally a single row is returned.
    /// Provide `Some(currency)` to override the auto-resolved reporting currency for this call;
    /// pass `None` to enrich currency metadata from Yahoo and infer only when needed.
    ///
    /// # Errors
    ///
    /// This method will return an error if the request fails or the response cannot be parsed.
    pub async fn ttm_cashflow(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<Vec<CashflowRow>, YfError> {
        Ok(self
            .ttm_cashflow_with_diagnostics(override_currency)
            .await?
            .into_data())
    }

    /// Fetches the trailing-twelve-month (TTM) cash flow statement with projection diagnostics.
    ///
    /// # Errors
    ///
    /// This method will return an error if the request fails or strict data-quality mode
    /// rejects a projection issue.
    pub async fn ttm_cashflow_with_diagnostics(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<YfResponse<Vec<CashflowRow>>, YfError> {
        self.fundamentals_builder()
            .ttm_cashflow_with_diagnostics(override_currency)
            .await
    }

    /// Fetches quarterly valuation measures (market cap, enterprise value, trailing/forward
    /// P/E, PEG, P/S, P/B, EV/revenue, EV/EBITDA), one row per as-of date, newest first.
    ///
    /// Provide `Some(currency)` to override the currency of market cap and enterprise value;
    /// pass `None` to use Yahoo's `currencyCode`, falling back to the trading currency.
    /// Use [`Ticker::valuation_builder`] for other frequencies.
    ///
    /// # Errors
    ///
    /// This method will return an error if the request fails or the response cannot be parsed.
    pub async fn valuation_measures(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<Vec<ValuationRow>, YfError> {
        Ok(self
            .valuation_measures_with_diagnostics(override_currency)
            .await?
            .into_data())
    }

    /// Fetches quarterly valuation measures with projection diagnostics, newest first.
    ///
    /// # Errors
    ///
    /// This method will return an error if the request fails or strict data-quality mode
    /// rejects a projection issue.
    pub async fn valuation_measures_with_diagnostics(
        &self,
        override_currency: Option<Currency>,
    ) -> Result<YfResponse<Vec<ValuationRow>>, YfError> {
        self.valuation_builder()
            .override_currency(override_currency)
            .fetch_with_diagnostics()
            .await
    }
}
