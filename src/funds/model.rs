//! Public fund/ETF models.
//!
//! **Units:** every weight, position and percentage in this module (holding weights,
//! asset-class positions, sector and bond-rating weightings, expense ratio, turnover,
//! earnings growth) is a **fraction in `0.0..=1.0`** — e.g. `0.0712` means 7.12 %.
//! This is the unit Yahoo uses for the `raw` value of these fields.

use paft::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Fund and ETF data assembled from Yahoo's `quoteType`, `summaryProfile`,
/// `fundProfile` and `topHoldings` quoteSummary modules.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FundsData {
    /// Yahoo quote type of the instrument, such as `ETF` or `MUTUALFUND`.
    pub quote_type: Option<String>,
    /// Long description of the fund's strategy.
    pub description: Option<String>,
    /// Category, family and legal type.
    pub overview: FundOverview,
    /// Expense ratio, turnover and net assets for the fund and its category.
    pub operations: FundOperations,
    /// Asset-class mix (fractions).
    pub asset_classes: AssetClasses,
    /// Largest holdings, in Yahoo's order.
    pub top_holdings: Vec<FundHolding>,
    /// Equity holding statistics for the fund and its category.
    pub equity_holdings: EquityHoldings,
    /// Bond holding statistics for the fund and its category.
    pub bond_holdings: BondHoldings,
    /// Weight per bond credit rating, in Yahoo's order.
    pub bond_ratings: Vec<BondRatingWeight>,
    /// Weight per equity sector, in Yahoo's order.
    pub sector_weightings: Vec<SectorWeighting>,
}

/// Fund overview from Yahoo's `fundProfile` module.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FundOverview {
    /// Morningstar-style category, such as `Large Blend`.
    pub category: Option<String>,
    /// Fund family, such as `SPDR State Street Global Advisors`.
    pub family: Option<String>,
    /// Legal type, such as `Exchange Traded Fund`.
    pub legal_type: Option<String>,
}

/// Fund operations for the fund itself and its category average.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FundOperations {
    /// Values for this fund.
    pub fund: FundOperationsValues,
    /// Category-average values.
    pub category_average: FundOperationsValues,
}

/// Fund operations values.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FundOperationsValues {
    /// Annual report expense ratio (fraction).
    pub expense_ratio: Option<Decimal>,
    /// Annual holdings turnover (fraction).
    pub turnover: Option<Decimal>,
    /// Total net assets exactly as Yahoo reports them. Yahoo does not state the
    /// currency or scale of this figure, so it is not exposed as money.
    pub total_net_assets: Option<Decimal>,
}

/// Asset-class mix (fractions of the portfolio).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AssetClasses {
    /// Cash position.
    pub cash: Option<Decimal>,
    /// Stock position.
    pub stock: Option<Decimal>,
    /// Bond position.
    pub bond: Option<Decimal>,
    /// Preferred stock position.
    pub preferred: Option<Decimal>,
    /// Convertible position.
    pub convertible: Option<Decimal>,
    /// Other position.
    pub other: Option<Decimal>,
}

/// A single top holding of a fund.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundHolding {
    /// Holding ticker symbol.
    pub symbol: String,
    /// Holding name.
    pub name: Option<String>,
    /// Portfolio weight (fraction).
    pub weight: Option<Decimal>,
}

/// Equity holding statistics for the fund and its category average.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EquityHoldings {
    /// Values for this fund.
    pub fund: EquityHoldingsValues,
    /// Category-average values.
    pub category_average: EquityHoldingsValues,
}

/// Equity holding statistics, exactly as Yahoo reports them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EquityHoldingsValues {
    /// Price/earnings.
    pub price_to_earnings: Option<Decimal>,
    /// Price/book.
    pub price_to_book: Option<Decimal>,
    /// Price/sales.
    pub price_to_sales: Option<Decimal>,
    /// Price/cash flow.
    pub price_to_cashflow: Option<Decimal>,
    /// Median market capitalisation as Yahoo reports it (currency/scale not stated by Yahoo).
    pub median_market_cap: Option<Decimal>,
    /// Three-year earnings growth (fraction).
    pub three_year_earnings_growth: Option<Decimal>,
}

/// Bond holding statistics for the fund and its category average.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BondHoldings {
    /// Values for this fund.
    pub fund: BondHoldingsValues,
    /// Category-average values.
    pub category_average: BondHoldingsValues,
}

/// Bond holding statistics, exactly as Yahoo reports them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BondHoldingsValues {
    /// Average duration (years).
    pub duration: Option<Decimal>,
    /// Average maturity (years).
    pub maturity: Option<Decimal>,
    /// Average credit quality as reported by Yahoo.
    pub credit_quality: Option<Decimal>,
}

/// Weight of one bond credit rating bucket.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BondRatingWeight {
    /// Yahoo's rating key, such as `aaa`, `bbb`, `below_b` or `us_government`.
    pub rating: String,
    /// Portfolio weight (fraction).
    pub weight: Decimal,
}

/// Weight of one equity sector.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectorWeighting {
    /// Sector.
    pub sector: FundSector,
    /// Portfolio weight (fraction).
    pub weight: Decimal,
}

/// Equity sector used in fund sector weightings.
///
/// Known Yahoo keys map to named variants; any other key is preserved verbatim in
/// [`FundSector::Other`] so no weighting is ever dropped.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FundSector {
    /// Basic materials (`basic_materials`).
    BasicMaterials,
    /// Communication services (`communication_services`).
    CommunicationServices,
    /// Consumer cyclical (`consumer_cyclical`).
    ConsumerCyclical,
    /// Consumer defensive (`consumer_defensive`).
    ConsumerDefensive,
    /// Energy (`energy`).
    Energy,
    /// Financial services (`financial_services`).
    FinancialServices,
    /// Healthcare (`healthcare`).
    Healthcare,
    /// Industrials (`industrials`).
    Industrials,
    /// Real estate (`realestate`).
    RealEstate,
    /// Technology (`technology`).
    Technology,
    /// Utilities (`utilities`).
    Utilities,
    /// A sector key this crate does not know, preserved verbatim.
    Other(String),
}

impl FundSector {
    /// Maps a Yahoo sector-weighting key to a sector, preserving unknown keys.
    #[must_use]
    pub fn from_yahoo_key(key: &str) -> Self {
        match key
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "basic_materials" => Self::BasicMaterials,
            "communication_services" => Self::CommunicationServices,
            "consumer_cyclical" => Self::ConsumerCyclical,
            "consumer_defensive" => Self::ConsumerDefensive,
            "energy" => Self::Energy,
            "financial_services" => Self::FinancialServices,
            "healthcare" => Self::Healthcare,
            "industrials" => Self::Industrials,
            "realestate" | "real_estate" => Self::RealEstate,
            "technology" => Self::Technology,
            "utilities" => Self::Utilities,
            _ => Self::Other(key.to_string()),
        }
    }

    /// Stable snake-case name of the sector (the original key for [`FundSector::Other`]).
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::BasicMaterials => "basic_materials",
            Self::CommunicationServices => "communication_services",
            Self::ConsumerCyclical => "consumer_cyclical",
            Self::ConsumerDefensive => "consumer_defensive",
            Self::Energy => "energy",
            Self::FinancialServices => "financial_services",
            Self::Healthcare => "healthcare",
            Self::Industrials => "industrials",
            Self::RealEstate => "real_estate",
            Self::Technology => "technology",
            Self::Utilities => "utilities",
            Self::Other(key) => key,
        }
    }
}

impl fmt::Display for FundSector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
