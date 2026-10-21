//! Market-wide status and summary per region.

mod model;
mod wire;

pub use model::{MarketRegion, MarketStatus, MarketSummaryEntry};

use url::Url;

use crate::core::client::CacheEndpoint;
use crate::core::net::{AuthFetchConfig, AuthMode, fetch_text_with_auth_retry};
use crate::core::{CallOptions, ProjectionContext, ProjectionIssue};
use crate::{YfClient, YfError, YfResponse};

const DEFAULT_BASE_MARKET_TIME: &str = "https://query1.finance.yahoo.com/v6/finance/markettime";
const DEFAULT_BASE_MARKET_SUMMARY: &str =
    "https://query1.finance.yahoo.com/v6/finance/quote/marketSummary";

/// Fields requested from the market summary endpoint.
const SUMMARY_FIELDS: &str = "shortName,longName,regularMarketPrice,regularMarketChange,regularMarketChangePercent,regularMarketTime";

/// A builder for market-wide status and index summaries for one [`MarketRegion`].
#[derive(Debug)]
pub struct MarketBuilder {
    client: YfClient,
    region: MarketRegion,
    market_time_base: Url,
    summary_base: Url,
    options: CallOptions,
}

impl MarketBuilder {
    /// Creates a new `MarketBuilder` for the given region.
    ///
    /// # Panics
    ///
    /// Panics if the hardcoded default base URLs are invalid, which indicates a
    /// bug within the crate itself.
    #[must_use]
    pub fn new(client: &YfClient, region: MarketRegion) -> Self {
        Self {
            client: client.clone(),
            region,
            market_time_base: Url::parse(DEFAULT_BASE_MARKET_TIME).unwrap(),
            summary_base: Url::parse(DEFAULT_BASE_MARKET_SUMMARY).unwrap(),
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!();

    /// (For testing) Overrides the base URL for the `v6/finance/markettime` endpoint.
    #[must_use]
    pub fn market_time_base(mut self, base: Url) -> Self {
        self.market_time_base = base;
        self
    }

    /// (For testing) Overrides the base URL for the `v6/finance/quote/marketSummary` endpoint.
    #[must_use]
    pub fn summary_base(mut self, base: Url) -> Self {
        self.summary_base = base;
        self
    }

    /// Returns the configured region.
    #[must_use]
    pub const fn region(&self) -> MarketRegion {
        self.region
    }

    /// Fetches the market's open/closed state, session times and timezone.
    ///
    /// Yahoo's `markettime` endpoint is known to ignore the `market` parameter
    /// and return U.S. market time regardless of the requested region. When a
    /// non-US region is requested and Yahoo answers with U.S. data (`id == "us"`),
    /// this returns `Ok(None)` rather than labelling U.S. hours as that region's
    /// status. [`MarketBuilder::status_with_diagnostics`] exposes the
    /// `ProviderFeatureUnavailable` warning; in strict mode it is an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, Yahoo reports an error, the
    /// response has no market time entry, or strict data-quality mode rejects a
    /// projection issue (including U.S. data returned for a non-US region).
    pub async fn status(&self) -> Result<Option<MarketStatus>, YfError> {
        Ok(self.status_with_diagnostics().await?.into_data())
    }

    /// Fetches the market status with projection diagnostics.
    ///
    /// See [`MarketBuilder::status`] for the regional caveat: `data` is `None`,
    /// with a `ProviderFeatureUnavailable` warning for `market_status`, when
    /// Yahoo returns U.S. market time for a non-US region.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, Yahoo reports an error, the
    /// response has no market time entry, or strict data-quality mode rejects a
    /// projection issue (including U.S. data returned for a non-US region).
    pub async fn status_with_diagnostics(
        &self,
    ) -> Result<YfResponse<Option<MarketStatus>>, YfError> {
        let mut url = self.market_time_base.clone();
        url.query_pairs_mut()
            .append_pair("formatted", "true")
            .append_pair("key", "finance")
            .append_pair("lang", "en-US")
            .append_pair("market", self.region.yahoo_code());

        let body = self
            .fetch(url, "market_time", wire::validate_market_time_body)
            .await?;
        let mut ctx = ProjectionContext::new("market_time", self.options.data_quality());
        if self.region != MarketRegion::US && wire::market_time_is_us(&body)? {
            ctx.provider_feature_unavailable(
                "market_status",
                ProjectionIssue::ProviderError {
                    message: format!(
                        "Yahoo markettime returned U.S. market time for market={}",
                        self.region.yahoo_code()
                    ),
                },
            )?;
            return Ok(ctx.finish(None));
        }
        let status = wire::parse_market_time_body(&body, &mut ctx)?;
        Ok(ctx.finish(Some(status)))
    }

    /// Fetches index and benchmark quotes for the region.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, Yahoo reports an error, the
    /// response has no result list, or strict data-quality mode rejects a
    /// malformed entry.
    pub async fn summary(&self) -> Result<Vec<MarketSummaryEntry>, YfError> {
        Ok(self.summary_with_diagnostics().await?.into_data())
    }

    /// Fetches the market summary with projection diagnostics.
    ///
    /// Malformed entries (no symbol or no price) are dropped and reported as
    /// warnings in best-effort mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, Yahoo reports an error, the
    /// response has no result list, or strict data-quality mode rejects a
    /// malformed entry.
    pub async fn summary_with_diagnostics(
        &self,
    ) -> Result<YfResponse<Vec<MarketSummaryEntry>>, YfError> {
        let mut url = self.summary_base.clone();
        url.query_pairs_mut()
            .append_pair("fields", SUMMARY_FIELDS)
            .append_pair("formatted", "false")
            .append_pair("lang", "en-US")
            .append_pair("market", self.region.yahoo_code());

        let body = self
            .fetch(url, "market_summary", wire::validate_summary_body)
            .await?;
        let mut ctx = ProjectionContext::new("market_summary", self.options.data_quality());
        let entries = wire::parse_summary_body(&body, &mut ctx)?;
        Ok(ctx.finish(entries))
    }

    async fn fetch(
        &self,
        url: Url,
        endpoint: &'static str,
        validator: fn(&str) -> Result<(), YfError>,
    ) -> Result<String, YfError> {
        let (body, _) = fetch_text_with_auth_retry(
            &self.client,
            url,
            AuthFetchConfig {
                auth_mode: AuthMode::OptionalCrumb,
                cache_endpoint: CacheEndpoint::Market,
                options: &self.options,
                cache_body: None,
                endpoint,
                fixture_key: self.region.yahoo_code(),
                ext: "json",
                retry_on_invalid_crumb_body: true,
                cache_validator: Some(validator),
            },
            |url| {
                self.client
                    .http()
                    .get(url)
                    .header("accept", "application/json")
            },
        )
        .await?;
        Ok(body)
    }
}
