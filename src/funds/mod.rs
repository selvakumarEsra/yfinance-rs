mod api;
mod model;
mod wire;

pub use model::{
    AssetClasses, BondHoldings, BondHoldingsValues, BondRatingWeight, EquityHoldings,
    EquityHoldingsValues, FundHolding, FundOperations, FundOperationsValues, FundOverview,
    FundSector, FundsData, SectorWeighting,
};

use crate::{YfClient, YfError, YfResponse, core::CallOptions};

/// A builder for fetching ETF and mutual-fund data (holdings, sector weightings,
/// asset classes, operations and overview) for a fund symbol.
///
/// All weights and percentages in [`FundsData`] are fractions in `0.0..=1.0`.
///
/// # Example
///
/// ```no_run
/// # use yfinance_rs::{FundsDataBuilder, YfClient};
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = YfClient::default();
/// let fund = FundsDataBuilder::new(&client, "SPY").fetch().await?;
/// for holding in &fund.top_holdings {
///     println!("{} {:?}", holding.symbol, holding.weight);
/// }
/// # Ok(())
/// # }
/// ```
pub struct FundsDataBuilder {
    client: YfClient,
    symbol: String,
    options: CallOptions,
}

impl FundsDataBuilder {
    /// Creates a new `FundsDataBuilder` for a given fund symbol.
    pub fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            symbol: symbol.into(),
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!();

    /// Fetches the fund data.
    ///
    /// # Errors
    ///
    /// Returns a `YfError` if the request fails, the symbol is not a fund (Yahoo returned
    /// no `topHoldings` module), or strict data-quality mode rejects a projection issue.
    pub async fn fetch(&self) -> Result<FundsData, YfError> {
        Ok(self.fetch_with_diagnostics().await?.into_data())
    }

    /// Fetches the fund data with projection diagnostics.
    ///
    /// # Errors
    ///
    /// Returns `YfError::MissingData` naming the symbol and its quote type when the
    /// symbol is not a fund (no `topHoldings` module), in either data-quality mode.
    /// Also returns an error if the request fails or strict data-quality mode rejects
    /// a projection issue.
    pub async fn fetch_with_diagnostics(&self) -> Result<YfResponse<FundsData>, YfError> {
        api::fetch_funds_data(&self.client, &self.symbol, &self.options).await
    }
}
