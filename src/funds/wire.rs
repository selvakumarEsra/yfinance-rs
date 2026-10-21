use serde::Deserialize;
use std::collections::BTreeMap;

use crate::core::wire::{BufferedWireValue, RawNum, WireValue};

pub type RawF64 = WireValue<RawNum<f64>>;

#[derive(Deserialize)]
pub struct V10Result {
    #[serde(rename = "quoteType", default)]
    pub(crate) quote_type: BufferedWireValue<QuoteTypeNode>,
    #[serde(rename = "summaryProfile", default)]
    pub(crate) summary_profile: BufferedWireValue<SummaryProfileNode>,
    #[serde(rename = "fundProfile", default)]
    pub(crate) fund_profile: BufferedWireValue<FundProfileNode>,
    #[serde(rename = "topHoldings", default)]
    pub(crate) top_holdings: BufferedWireValue<TopHoldingsNode>,
}

#[derive(Deserialize)]
pub struct QuoteTypeNode {
    #[serde(rename = "quoteType", default)]
    pub(crate) quote_type: WireValue<String>,
}

#[derive(Deserialize)]
pub struct SummaryProfileNode {
    #[serde(rename = "longBusinessSummary", default)]
    pub(crate) long_business_summary: WireValue<String>,
}

#[derive(Deserialize)]
pub struct FundProfileNode {
    #[serde(rename = "categoryName", default)]
    pub(crate) category_name: WireValue<String>,
    #[serde(default)]
    pub(crate) family: WireValue<String>,
    #[serde(rename = "legalType", default)]
    pub(crate) legal_type: WireValue<String>,
    #[serde(rename = "feesExpensesInvestment", default)]
    pub(crate) fees: BufferedWireValue<FeesNode>,
    #[serde(rename = "feesExpensesInvestmentCat", default)]
    pub(crate) fees_cat: BufferedWireValue<FeesNode>,
}

#[derive(Deserialize)]
pub struct FeesNode {
    #[serde(rename = "annualReportExpenseRatio", default)]
    pub(crate) annual_report_expense_ratio: RawF64,
    #[serde(rename = "annualHoldingsTurnover", default)]
    pub(crate) annual_holdings_turnover: RawF64,
    #[serde(rename = "totalNetAssets", default)]
    pub(crate) total_net_assets: RawF64,
}

#[derive(Deserialize)]
pub struct TopHoldingsNode {
    #[serde(rename = "cashPosition", default)]
    pub(crate) cash_position: RawF64,
    #[serde(rename = "stockPosition", default)]
    pub(crate) stock_position: RawF64,
    #[serde(rename = "bondPosition", default)]
    pub(crate) bond_position: RawF64,
    #[serde(rename = "preferredPosition", default)]
    pub(crate) preferred_position: RawF64,
    #[serde(rename = "convertiblePosition", default)]
    pub(crate) convertible_position: RawF64,
    #[serde(rename = "otherPosition", default)]
    pub(crate) other_position: RawF64,
    #[serde(default)]
    pub(crate) holdings: BufferedWireValue<Vec<BufferedWireValue<HoldingNode>>>,
    #[serde(rename = "equityHoldings", default)]
    pub(crate) equity_holdings: BufferedWireValue<EquityHoldingsNode>,
    #[serde(rename = "bondHoldings", default)]
    pub(crate) bond_holdings: BufferedWireValue<BondHoldingsNode>,
    #[serde(rename = "bondRatings", default)]
    pub(crate) bond_ratings: BufferedWireValue<Vec<BTreeMap<String, RawF64>>>,
    #[serde(rename = "sectorWeightings", default)]
    pub(crate) sector_weightings: BufferedWireValue<Vec<BTreeMap<String, RawF64>>>,
}

#[derive(Deserialize)]
pub struct HoldingNode {
    #[serde(default)]
    pub(crate) symbol: WireValue<String>,
    #[serde(rename = "holdingName", default)]
    pub(crate) holding_name: WireValue<String>,
    #[serde(rename = "holdingPercent", default)]
    pub(crate) holding_percent: RawF64,
}

#[derive(Deserialize)]
pub struct EquityHoldingsNode {
    #[serde(rename = "priceToEarnings", default)]
    pub(crate) price_to_earnings: RawF64,
    #[serde(rename = "priceToBook", default)]
    pub(crate) price_to_book: RawF64,
    #[serde(rename = "priceToSales", default)]
    pub(crate) price_to_sales: RawF64,
    #[serde(rename = "priceToCashflow", default)]
    pub(crate) price_to_cashflow: RawF64,
    #[serde(rename = "medianMarketCap", default)]
    pub(crate) median_market_cap: RawF64,
    #[serde(rename = "threeYearEarningsGrowth", default)]
    pub(crate) three_year_earnings_growth: RawF64,
    #[serde(rename = "priceToEarningsCat", default)]
    pub(crate) price_to_earnings_cat: RawF64,
    #[serde(rename = "priceToBookCat", default)]
    pub(crate) price_to_book_cat: RawF64,
    #[serde(rename = "priceToSalesCat", default)]
    pub(crate) price_to_sales_cat: RawF64,
    #[serde(rename = "priceToCashflowCat", default)]
    pub(crate) price_to_cashflow_cat: RawF64,
    #[serde(rename = "medianMarketCapCat", default)]
    pub(crate) median_market_cap_cat: RawF64,
    #[serde(rename = "threeYearEarningsGrowthCat", default)]
    pub(crate) three_year_earnings_growth_cat: RawF64,
}

#[derive(Deserialize)]
pub struct BondHoldingsNode {
    #[serde(default)]
    pub(crate) duration: RawF64,
    #[serde(default)]
    pub(crate) maturity: RawF64,
    #[serde(rename = "creditQuality", default)]
    pub(crate) credit_quality: RawF64,
    #[serde(rename = "durationCat", default)]
    pub(crate) duration_cat: RawF64,
    #[serde(rename = "maturityCat", default)]
    pub(crate) maturity_cat: RawF64,
    #[serde(rename = "creditQualityCat", default)]
    pub(crate) credit_quality_cat: RawF64,
}
