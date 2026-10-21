mod api;
mod model;
mod wire;

pub use model::{SecFiling, SecFilingExhibit};

use crate::{YfClient, YfError, YfResponse, core::CallOptions};

/// A builder for fetching a symbol's SEC filings from Yahoo's `secFilings` quoteSummary module.
///
/// # Example
///
/// ```no_run
/// # use yfinance_rs::{SecFilingsBuilder, YfClient};
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = YfClient::default();
/// let filings = SecFilingsBuilder::new(&client, "AAPL").fetch().await?;
/// for filing in filings.iter().take(3) {
///     println!("{} {} {:?}", filing.date, filing.filing_type, filing.title);
/// }
/// # Ok(())
/// # }
/// ```
pub struct SecFilingsBuilder {
    client: YfClient,
    symbol: String,
    options: CallOptions,
}

impl SecFilingsBuilder {
    /// Creates a new `SecFilingsBuilder` for a given symbol.
    pub fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            symbol: symbol.into(),
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!();

    /// Fetches the symbol's SEC filings.
    ///
    /// # Errors
    ///
    /// Returns a `YfError` if the request fails, Yahoo returns an error, or strict
    /// data-quality mode rejects a projection issue.
    pub async fn fetch(&self) -> Result<Vec<SecFiling>, YfError> {
        Ok(self.fetch_with_diagnostics().await?.into_data())
    }

    /// Fetches the symbol's SEC filings with projection diagnostics.
    ///
    /// In best-effort mode a response without the `secFilings` module yields an empty
    /// list plus a `ProviderFeatureUnavailable` warning, and filings with an unparseable
    /// date are dropped with a `DroppedItem` warning while their siblings are kept.
    ///
    /// # Errors
    ///
    /// Returns a `YfError` if the request fails, Yahoo returns an error, or strict
    /// data-quality mode rejects a projection issue (including a missing module).
    pub async fn fetch_with_diagnostics(&self) -> Result<YfResponse<Vec<SecFiling>>, YfError> {
        api::fetch_sec_filings(&self.client, &self.symbol, &self.options).await
    }
}
