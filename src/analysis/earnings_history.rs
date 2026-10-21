//! Projection of Yahoo's `earningsHistory` quoteSummary module.

use std::collections::BTreeMap;

use paft::money::Currency;

use crate::core::{
    CallOptions, ProjectionContext, ProjectionIssue, YfClient, YfError, YfResponse,
    currency_resolver::{
        AnalystEstimateCurrencyEvidence, CurrencyPurpose, ResolvedCurrencyUnit,
        project_currency_resolution,
    },
    diagnostics::{
        WireProjection, nonempty_string, optional_decimal_f64,
        optional_price_f64_with_currency_issue, required_wire_date,
    },
    wire::WireField,
};

use super::fetch::{fetch_modules_body, parse_modules};
use super::model::EarningsHistoryRow;
use super::wire::EarningsHistoryItemNode;

const MODULE: &str = "earningsHistory";

struct ProjectedRowCurrency {
    unit: Option<ResolvedCurrencyUnit>,
    issue: Option<ProjectionIssue>,
}

struct PendingRow<'a> {
    node: &'a EarningsHistoryItemNode,
    quarter: chrono::NaiveDate,
    key: String,
    currency: Option<String>,
    eps_actual: Option<f64>,
    eps_estimate: Option<f64>,
    eps_difference: Option<f64>,
}

impl PendingRow<'_> {
    const fn has_money(&self) -> bool {
        self.eps_actual.is_some() || self.eps_estimate.is_some() || self.eps_difference.is_some()
    }
}

#[allow(clippy::too_many_lines)]
pub(super) async fn earnings_history(
    client: &YfClient,
    symbol: &str,
    override_currency: Option<Currency>,
    options: &CallOptions,
) -> Result<YfResponse<Vec<EarningsHistoryRow>>, YfError> {
    let mut ctx = ProjectionContext::new("earnings_history", options.data_quality());
    let body = fetch_modules_body(client, symbol, MODULE, options).await?;
    let root = parse_modules(&body)?;

    if let Some(details) = root.earnings_history.invalid_details() {
        ctx.provider_feature_unavailable(
            MODULE,
            ProjectionIssue::InvalidField {
                field: MODULE,
                details: details.to_string(),
            },
        )?;
        return Ok(ctx.finish(Vec::new()));
    }
    let Some(module) = WireField::as_ref(&root.earnings_history) else {
        ctx.unavailable_feature(MODULE)?;
        return Ok(ctx.finish(Vec::new()));
    };

    let Some(items) = module.history.optional_ref_field(
        &mut ctx,
        "earningsHistory.history",
        Some(symbol),
        "history",
    )?
    else {
        return Ok(ctx.finish(Vec::new()));
    };

    let mut pending = Vec::with_capacity(items.len());
    for item in items {
        if let Some(details) = item.invalid_details() {
            ctx.dropped_item(
                "earnings_history_row",
                None,
                ProjectionIssue::InvalidField {
                    field: "history[]",
                    details: details.to_string(),
                },
            )?;
            continue;
        }
        let Some(node) = WireField::as_ref(item) else {
            continue;
        };
        let period_label = node.period.as_ref().cloned();
        let Some(quarter) = required_wire_date(
            &mut ctx,
            "earnings_history_row",
            period_label.as_deref(),
            "quarter",
            &node.quarter,
        )?
        else {
            continue;
        };
        let key = quarter.to_string();
        let k = Some(key.as_str());
        let currency = nonempty_string(node.currency.optional_cloned_field(
            &mut ctx,
            "earningsHistory.history[].currency",
            k,
            "currency",
        )?);
        let eps_actual = node.eps_actual.optional_raw_field(
            &mut ctx,
            "earningsHistory.history[].epsActual",
            k,
            "epsActual",
        )?;
        let eps_estimate = node.eps_estimate.optional_raw_field(
            &mut ctx,
            "earningsHistory.history[].epsEstimate",
            k,
            "epsEstimate",
        )?;
        let eps_difference = node.eps_difference.optional_raw_field(
            &mut ctx,
            "earningsHistory.history[].epsDifference",
            k,
            "epsDifference",
        )?;
        pending.push(PendingRow {
            node,
            quarter,
            key,
            currency,
            eps_actual,
            eps_estimate,
            eps_difference,
        });
    }

    // Resolve the currency once per distinct provider code among rows carrying EPS values.
    let mut currencies: BTreeMap<Option<String>, ProjectedRowCurrency> = BTreeMap::new();
    for row in pending.iter().filter(|row| row.has_money()) {
        let group = row.currency.clone();
        if currencies.contains_key(&group) {
            continue;
        }
        let projected = project_currency_resolution(
            &mut ctx,
            symbol,
            CurrencyPurpose::AnalystEstimate,
            group.as_deref(),
            client
                .resolve_analyst_estimate_currency(
                    symbol,
                    override_currency.clone(),
                    AnalystEstimateCurrencyEvidence::Earnings(group.as_deref()),
                    options,
                )
                .await,
        )?;
        let issue = projected.issue().cloned();
        currencies.insert(
            group,
            ProjectedRowCurrency {
                unit: projected.into_unit(),
                issue,
            },
        );
    }

    let mut rows = Vec::with_capacity(pending.len());
    for row in pending {
        let k = Some(row.key.as_str());
        let currency = currencies.get(&row.currency);
        let unit = currency.and_then(|c| c.unit.as_ref());
        let issue = currency.and_then(|c| c.issue.as_ref());

        let surprise_raw = row.node.surprise_percent.optional_raw_field(
            &mut ctx,
            "earningsHistory.history[].surprisePercent",
            k,
            "surprisePercent",
        )?;
        let period = nonempty_string(row.node.period.optional_cloned_field(
            &mut ctx,
            "earningsHistory.history[].period",
            k,
            "period",
        )?);

        rows.push(EarningsHistoryRow {
            quarter: row.quarter,
            period,
            eps_actual: optional_price_f64_with_currency_issue(
                &mut ctx,
                "earningsHistory.history[].epsActual",
                k,
                unit,
                issue,
                row.eps_actual,
                "earnings history EPS",
            )?,
            eps_estimate: optional_price_f64_with_currency_issue(
                &mut ctx,
                "earningsHistory.history[].epsEstimate",
                k,
                unit,
                issue,
                row.eps_estimate,
                "earnings history EPS",
            )?,
            eps_difference: optional_price_f64_with_currency_issue(
                &mut ctx,
                "earningsHistory.history[].epsDifference",
                k,
                unit,
                issue,
                row.eps_difference,
                "earnings history EPS",
            )?,
            surprise_percent: optional_decimal_f64(
                &mut ctx,
                "earningsHistory.history[].surprisePercent",
                k,
                surprise_raw,
                "earnings surprise",
            )?,
        });
    }

    rows.sort_by_key(|row| row.quarter);
    Ok(ctx.finish(rows))
}
