//! Symbol lookup by asset type.

mod model;
mod wire;

pub use model::{LookupKind, LookupResult};

use url::Url;

use crate::core::client::CacheEndpoint;
use crate::core::net::{AuthFetchConfig, AuthMode, fetch_text_with_auth_retry};
use crate::core::{CallOptions, ProjectionContext};
use crate::{YfClient, YfError, YfResponse};

const DEFAULT_BASE_LOOKUP: &str = "https://query1.finance.yahoo.com/v1/finance/lookup";
const DEFAULT_COUNT: u32 = 25;

/// A builder for Yahoo's `v1/finance/lookup` symbol lookup, filtered by asset type.
///
/// Unlike [`crate::SearchBuilder`], lookup supports paging (`offset`/`count`) and
/// filtering by [`LookupKind`].
#[derive(Debug)]
pub struct LookupBuilder {
    client: YfClient,
    base: Url,
    query: String,
    kind: LookupKind,
    count: u32,
    offset: u32,
    options: CallOptions,
}

impl LookupBuilder {
    /// Creates a new `LookupBuilder` for a query. Defaults: [`LookupKind::All`],
    /// 25 results, offset 0.
    ///
    /// # Panics
    ///
    /// Panics if the hardcoded default base URL is invalid, which indicates a bug
    /// within the crate itself.
    pub fn new(client: &YfClient, query: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            base: Url::parse(DEFAULT_BASE_LOOKUP).unwrap(),
            query: query.into(),
            kind: LookupKind::All,
            count: DEFAULT_COUNT,
            offset: 0,
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!();

    /// (For testing) Overrides the base URL for the lookup API.
    #[must_use]
    pub fn lookup_base(mut self, base: Url) -> Self {
        self.base = base;
        self
    }

    /// Restricts results to one asset type.
    #[must_use]
    pub const fn kind(mut self, kind: LookupKind) -> Self {
        self.kind = kind;
        self
    }

    /// Sets the maximum number of results (Yahoo's `count`). Must be at least 1.
    #[must_use]
    pub const fn count(mut self, n: u32) -> Self {
        self.count = n;
        self
    }

    /// Sets the zero-based index of the first result (Yahoo's `start`).
    #[must_use]
    pub const fn offset(mut self, n: u32) -> Self {
        self.offset = n;
        self
    }

    /// Executes the lookup.
    ///
    /// # Errors
    ///
    /// Returns `YfError::InvalidParams` for an empty query or a zero count, and
    /// an error if the request fails, Yahoo reports an error, or the response
    /// cannot be parsed.
    pub async fn fetch(&self) -> Result<Vec<LookupResult>, YfError> {
        Ok(self.fetch_with_diagnostics().await?.into_data())
    }

    /// Executes the lookup with projection diagnostics.
    ///
    /// Documents without a symbol are dropped and reported as warnings in
    /// best-effort mode.
    ///
    /// # Errors
    ///
    /// Returns `YfError::InvalidParams` for an empty query or a zero count
    /// (before any request is sent), and an error if the request fails, Yahoo
    /// reports an error, or strict data-quality mode rejects a projection issue.
    pub async fn fetch_with_diagnostics(&self) -> Result<YfResponse<Vec<LookupResult>>, YfError> {
        let query = self.query.trim();
        if query.is_empty() {
            return Err(YfError::InvalidParams(
                "lookup query cannot be empty".into(),
            ));
        }
        if self.count == 0 {
            return Err(YfError::InvalidParams(
                "lookup count must be at least 1".into(),
            ));
        }

        let mut url = self.base.clone();
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("type", self.kind.yahoo_code())
            .append_pair("start", &self.offset.to_string())
            .append_pair("count", &self.count.to_string())
            .append_pair("formatted", "false")
            .append_pair("fetchPricingData", "true")
            .append_pair("lang", "en-US")
            .append_pair("region", "US");

        let fixture_key = format!("{}_{query}", self.kind.yahoo_code());
        let (body, _) = fetch_text_with_auth_retry(
            &self.client,
            url,
            AuthFetchConfig {
                auth_mode: AuthMode::OptionalCrumb,
                cache_endpoint: CacheEndpoint::Lookup,
                options: &self.options,
                cache_body: None,
                endpoint: "lookup_v1",
                fixture_key: &fixture_key,
                ext: "json",
                retry_on_invalid_crumb_body: true,
                cache_validator: Some(wire::validate_lookup_body),
            },
            |url| {
                self.client
                    .http()
                    .get(url)
                    .header("accept", "application/json")
            },
        )
        .await?;

        let mut ctx = ProjectionContext::new("lookup", self.options.data_quality());
        let results = wire::parse_lookup_body(&body, &mut ctx)?;
        Ok(ctx.finish(results))
    }
}
