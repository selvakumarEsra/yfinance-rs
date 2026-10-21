use std::collections::BTreeMap;

use paft::Decimal;

use crate::core::{
    CallOptions, DataQuality, ProjectionContext, ProjectionIssue, YfClient, YfError, YfResponse,
    diagnostics::{WireProjection, nonempty_string, optional_decimal_f64},
    quotesummary,
    wire::{BufferedWireValue, WireField, WireValue},
};

use super::model::{
    AssetClasses, BondHoldings, BondHoldingsValues, BondRatingWeight, EquityHoldings,
    EquityHoldingsValues, FundHolding, FundOperations, FundOperationsValues, FundOverview,
    FundSector, FundsData, SectorWeighting,
};
use super::wire::{FeesNode, HoldingNode, RawF64, TopHoldingsNode, V10Result};

pub(super) const MODULES: &str = "quoteType,summaryProfile,topHoldings,fundProfile";

pub(super) async fn fetch_funds_data(
    client: &YfClient,
    symbol: &str,
    options: &CallOptions,
) -> Result<YfResponse<FundsData>, YfError> {
    let root: V10Result =
        quotesummary::fetch_module_result(client, symbol, MODULES, "funds", options).await?;
    map_funds_data(symbol, &root, options.data_quality())
}

fn module_node<'a, T>(
    ctx: &mut ProjectionContext,
    feature: &'static str,
    value: &'a BufferedWireValue<T>,
) -> Result<Option<&'a T>, YfError> {
    if let Some(details) = value.invalid_details() {
        ctx.provider_feature_unavailable(
            feature,
            ProjectionIssue::InvalidField {
                field: feature,
                details: details.to_string(),
            },
        )?;
        return Ok(None);
    }
    let node = WireField::as_ref(value);
    if node.is_none() {
        ctx.unavailable_feature(feature)?;
    }
    Ok(node)
}

fn optional_string(
    ctx: &mut ProjectionContext,
    path: &'static str,
    key: Option<&str>,
    value: &WireValue<String>,
) -> Result<Option<String>, YfError> {
    Ok(nonempty_string(
        value.optional_cloned_field(ctx, path, key, path)?,
    ))
}

fn fraction(
    ctx: &mut ProjectionContext,
    path: &'static str,
    key: Option<&str>,
    value: &RawF64,
) -> Result<Option<Decimal>, YfError> {
    let raw = value.optional_raw_field(ctx, path, key, path)?;
    optional_decimal_f64(ctx, path, key, raw, "fund value")
}

fn map_funds_data(
    symbol: &str,
    root: &V10Result,
    data_quality: DataQuality,
) -> Result<YfResponse<FundsData>, YfError> {
    let mut ctx = ProjectionContext::new("funds_data", data_quality);
    let key = Some(symbol);

    let quote_type = match WireField::as_ref(&root.quote_type) {
        Some(node) => optional_string(&mut ctx, "quoteType.quoteType", key, &node.quote_type)?,
        None => None,
    };

    // A symbol without the `topHoldings` module is not a fund: fail regardless of mode.
    if matches!(root.top_holdings, BufferedWireValue::Missing) {
        return Err(YfError::MissingData(format!(
            "{symbol} has no fund data (quote type {}): Yahoo returned no topHoldings module",
            quote_type.as_deref().unwrap_or("unknown")
        )));
    }

    let mut data = FundsData {
        quote_type,
        ..FundsData::default()
    };

    if let Some(profile) = module_node(&mut ctx, "summaryProfile", &root.summary_profile)? {
        data.description = optional_string(
            &mut ctx,
            "summaryProfile.longBusinessSummary",
            key,
            &profile.long_business_summary,
        )?;
    }

    if let Some(profile) = module_node(&mut ctx, "fundProfile", &root.fund_profile)? {
        data.overview = FundOverview {
            category: optional_string(
                &mut ctx,
                "fundProfile.categoryName",
                key,
                &profile.category_name,
            )?,
            family: optional_string(&mut ctx, "fundProfile.family", key, &profile.family)?,
            legal_type: optional_string(
                &mut ctx,
                "fundProfile.legalType",
                key,
                &profile.legal_type,
            )?,
        };
        let fees = profile.fees.optional_ref_field(
            &mut ctx,
            "fundProfile.feesExpensesInvestment",
            key,
            "feesExpensesInvestment",
        )?;
        let fees_cat = profile.fees_cat.optional_ref_field(
            &mut ctx,
            "fundProfile.feesExpensesInvestmentCat",
            key,
            "feesExpensesInvestmentCat",
        )?;
        data.operations = FundOperations {
            fund: map_fees(&mut ctx, key, fees, FeesPaths::FUND)?,
            category_average: map_fees(&mut ctx, key, fees_cat, FeesPaths::CATEGORY)?,
        };
    }

    if let Some(top) = module_node(&mut ctx, "topHoldings", &root.top_holdings)? {
        map_top_holdings(&mut ctx, key, top, &mut data)?;
    }

    Ok(ctx.finish(data))
}

#[derive(Clone, Copy)]
struct FeesPaths {
    expense_ratio: &'static str,
    turnover: &'static str,
    total_net_assets: &'static str,
}

impl FeesPaths {
    const FUND: Self = Self {
        expense_ratio: "fundProfile.feesExpensesInvestment.annualReportExpenseRatio",
        turnover: "fundProfile.feesExpensesInvestment.annualHoldingsTurnover",
        total_net_assets: "fundProfile.feesExpensesInvestment.totalNetAssets",
    };
    const CATEGORY: Self = Self {
        expense_ratio: "fundProfile.feesExpensesInvestmentCat.annualReportExpenseRatio",
        turnover: "fundProfile.feesExpensesInvestmentCat.annualHoldingsTurnover",
        total_net_assets: "fundProfile.feesExpensesInvestmentCat.totalNetAssets",
    };
}

fn map_fees(
    ctx: &mut ProjectionContext,
    key: Option<&str>,
    fees: Option<&FeesNode>,
    paths: FeesPaths,
) -> Result<FundOperationsValues, YfError> {
    let Some(fees) = fees else {
        return Ok(FundOperationsValues::default());
    };
    Ok(FundOperationsValues {
        expense_ratio: fraction(
            ctx,
            paths.expense_ratio,
            key,
            &fees.annual_report_expense_ratio,
        )?,
        turnover: fraction(ctx, paths.turnover, key, &fees.annual_holdings_turnover)?,
        total_net_assets: fraction(ctx, paths.total_net_assets, key, &fees.total_net_assets)?,
    })
}

#[allow(clippy::too_many_lines)]
fn map_top_holdings(
    ctx: &mut ProjectionContext,
    key: Option<&str>,
    top: &TopHoldingsNode,
    data: &mut FundsData,
) -> Result<(), YfError> {
    data.asset_classes = AssetClasses {
        cash: fraction(ctx, "topHoldings.cashPosition", key, &top.cash_position)?,
        stock: fraction(ctx, "topHoldings.stockPosition", key, &top.stock_position)?,
        bond: fraction(ctx, "topHoldings.bondPosition", key, &top.bond_position)?,
        preferred: fraction(
            ctx,
            "topHoldings.preferredPosition",
            key,
            &top.preferred_position,
        )?,
        convertible: fraction(
            ctx,
            "topHoldings.convertiblePosition",
            key,
            &top.convertible_position,
        )?,
        other: fraction(ctx, "topHoldings.otherPosition", key, &top.other_position)?,
    };

    if let Some(holdings) =
        top.holdings
            .optional_ref_field(ctx, "topHoldings.holdings", key, "holdings")?
    {
        for holding in holdings {
            if let Some(details) = holding.invalid_details() {
                ctx.dropped_item(
                    "fund_holding",
                    key,
                    ProjectionIssue::InvalidField {
                        field: "holdings[]",
                        details: details.to_string(),
                    },
                )?;
                continue;
            }
            if let Some(holding) = WireField::as_ref(holding)
                && let Some(holding) = map_holding(ctx, holding)?
            {
                data.top_holdings.push(holding);
            }
        }
    }

    if let Some(eq) = top.equity_holdings.optional_ref_field(
        ctx,
        "topHoldings.equityHoldings",
        key,
        "equityHoldings",
    )? {
        data.equity_holdings = EquityHoldings {
            fund: EquityHoldingsValues {
                price_to_earnings: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToEarnings",
                    key,
                    &eq.price_to_earnings,
                )?,
                price_to_book: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToBook",
                    key,
                    &eq.price_to_book,
                )?,
                price_to_sales: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToSales",
                    key,
                    &eq.price_to_sales,
                )?,
                price_to_cashflow: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToCashflow",
                    key,
                    &eq.price_to_cashflow,
                )?,
                median_market_cap: fraction(
                    ctx,
                    "topHoldings.equityHoldings.medianMarketCap",
                    key,
                    &eq.median_market_cap,
                )?,
                three_year_earnings_growth: fraction(
                    ctx,
                    "topHoldings.equityHoldings.threeYearEarningsGrowth",
                    key,
                    &eq.three_year_earnings_growth,
                )?,
            },
            category_average: EquityHoldingsValues {
                price_to_earnings: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToEarningsCat",
                    key,
                    &eq.price_to_earnings_cat,
                )?,
                price_to_book: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToBookCat",
                    key,
                    &eq.price_to_book_cat,
                )?,
                price_to_sales: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToSalesCat",
                    key,
                    &eq.price_to_sales_cat,
                )?,
                price_to_cashflow: fraction(
                    ctx,
                    "topHoldings.equityHoldings.priceToCashflowCat",
                    key,
                    &eq.price_to_cashflow_cat,
                )?,
                median_market_cap: fraction(
                    ctx,
                    "topHoldings.equityHoldings.medianMarketCapCat",
                    key,
                    &eq.median_market_cap_cat,
                )?,
                three_year_earnings_growth: fraction(
                    ctx,
                    "topHoldings.equityHoldings.threeYearEarningsGrowthCat",
                    key,
                    &eq.three_year_earnings_growth_cat,
                )?,
            },
        };
    }

    if let Some(bond) = top.bond_holdings.optional_ref_field(
        ctx,
        "topHoldings.bondHoldings",
        key,
        "bondHoldings",
    )? {
        data.bond_holdings = BondHoldings {
            fund: BondHoldingsValues {
                duration: fraction(
                    ctx,
                    "topHoldings.bondHoldings.duration",
                    key,
                    &bond.duration,
                )?,
                maturity: fraction(
                    ctx,
                    "topHoldings.bondHoldings.maturity",
                    key,
                    &bond.maturity,
                )?,
                credit_quality: fraction(
                    ctx,
                    "topHoldings.bondHoldings.creditQuality",
                    key,
                    &bond.credit_quality,
                )?,
            },
            category_average: BondHoldingsValues {
                duration: fraction(
                    ctx,
                    "topHoldings.bondHoldings.durationCat",
                    key,
                    &bond.duration_cat,
                )?,
                maturity: fraction(
                    ctx,
                    "topHoldings.bondHoldings.maturityCat",
                    key,
                    &bond.maturity_cat,
                )?,
                credit_quality: fraction(
                    ctx,
                    "topHoldings.bondHoldings.creditQualityCat",
                    key,
                    &bond.credit_quality_cat,
                )?,
            },
        };
    }

    if let Some(ratings) =
        top.bond_ratings
            .optional_ref_field(ctx, "topHoldings.bondRatings", key, "bondRatings")?
    {
        for (rating, weight) in keyed_weights(ctx, "fund_bond_rating", ratings)? {
            data.bond_ratings.push(BondRatingWeight { rating, weight });
        }
    }

    if let Some(sectors) = top.sector_weightings.optional_ref_field(
        ctx,
        "topHoldings.sectorWeightings",
        key,
        "sectorWeightings",
    )? {
        for (sector, weight) in keyed_weights(ctx, "fund_sector_weighting", sectors)? {
            data.sector_weightings.push(SectorWeighting {
                sector: FundSector::from_yahoo_key(&sector),
                weight,
            });
        }
    }

    Ok(())
}

fn map_holding(
    ctx: &mut ProjectionContext,
    node: &HoldingNode,
) -> Result<Option<FundHolding>, YfError> {
    let symbol = match &node.symbol {
        WireValue::Valid(symbol) if !symbol.trim().is_empty() => symbol.trim().to_string(),
        WireValue::Invalid(details) => {
            ctx.dropped_item(
                "fund_holding",
                None,
                ProjectionIssue::InvalidField {
                    field: "symbol",
                    details: details.message().to_string(),
                },
            )?;
            return Ok(None);
        }
        _ => {
            ctx.dropped_item(
                "fund_holding",
                node.holding_name.as_ref().map(String::as_str),
                ProjectionIssue::MissingRequiredField { field: "symbol" },
            )?;
            return Ok(None);
        }
    };
    let key = Some(symbol.as_str());
    let name = optional_string(
        ctx,
        "topHoldings.holdings[].holdingName",
        key,
        &node.holding_name,
    )?;
    let weight = fraction(
        ctx,
        "topHoldings.holdings[].holdingPercent",
        key,
        &node.holding_percent,
    )?;
    Ok(Some(FundHolding {
        symbol,
        name,
        weight,
    }))
}

/// Flattens Yahoo's `[{ "<key>": { "raw": <weight> } }, ...]` lists, preserving order
/// and every key (unknown keys included).
fn keyed_weights(
    ctx: &mut ProjectionContext,
    item: &'static str,
    entries: &[BTreeMap<String, RawF64>],
) -> Result<Vec<(String, Decimal)>, YfError> {
    let mut out = Vec::new();
    for entry in entries {
        for (name, value) in entry {
            if name == "maxAge" {
                continue;
            }
            let issue = match value {
                WireValue::Valid(raw) => match raw.raw {
                    Some(raw) => match crate::core::conversions::decimal_from_f64(raw) {
                        Some(weight) => {
                            out.push((name.clone(), weight));
                            continue;
                        }
                        None => ProjectionIssue::ConversionFailed {
                            target: "fund weight",
                        },
                    },
                    None => ProjectionIssue::MissingRequiredField { field: "raw" },
                },
                WireValue::Invalid(details) => ProjectionIssue::InvalidField {
                    field: "raw",
                    details: details.message().to_string(),
                },
                WireValue::Missing => ProjectionIssue::MissingRequiredField { field: "raw" },
            };
            ctx.dropped_item(item, Some(name), issue)?;
        }
    }
    Ok(out)
}
