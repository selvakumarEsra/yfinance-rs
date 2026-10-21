use chrono::NaiveDate;

use crate::core::{
    CallOptions, DataQuality, ProjectionContext, ProjectionIssue, YfClient, YfError, YfResponse,
    conversions::i64_to_date,
    diagnostics::{WireProjection, nonempty_string},
    quotesummary,
    wire::{BufferedWireValue, WireField, WireValue},
};

use super::model::{SecFiling, SecFilingExhibit};
use super::wire::{ExhibitNode, FilingNode, V10Result};

pub(super) const MODULE: &str = "secFilings";

pub(super) async fn fetch_sec_filings(
    client: &YfClient,
    symbol: &str,
    options: &CallOptions,
) -> Result<YfResponse<Vec<SecFiling>>, YfError> {
    let root: V10Result =
        quotesummary::fetch_module_result(client, symbol, MODULE, "filings", options).await?;
    map_sec_filings(symbol, &root, options.data_quality())
}

fn map_sec_filings(
    symbol: &str,
    root: &V10Result,
    data_quality: DataQuality,
) -> Result<YfResponse<Vec<SecFiling>>, YfError> {
    let mut ctx = ProjectionContext::new("sec_filings", data_quality);

    let Some(node) = module_node(&mut ctx, &root.sec_filings)? else {
        ctx.unavailable_feature(MODULE)?;
        return Ok(ctx.finish(Vec::new()));
    };

    let Some(rows) =
        node.filings
            .optional_ref_field(&mut ctx, "secFilings.filings", Some(symbol), "filings")?
    else {
        return Ok(ctx.finish(Vec::new()));
    };

    let mut filings = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(row) = required_item(&mut ctx, "sec_filing", None, row)? else {
            continue;
        };
        if let Some(filing) = map_filing(&mut ctx, row)? {
            filings.push(filing);
        }
    }

    Ok(ctx.finish(filings))
}

fn module_node<'a, T>(
    ctx: &mut ProjectionContext,
    value: &'a BufferedWireValue<T>,
) -> Result<Option<&'a T>, YfError> {
    if let Some(details) = value.invalid_details() {
        ctx.provider_feature_unavailable(
            MODULE,
            ProjectionIssue::InvalidField {
                field: MODULE,
                details: details.to_string(),
            },
        )?;
        return Ok(None);
    }
    Ok(WireField::as_ref(value))
}

fn required_item<'a, T>(
    ctx: &mut ProjectionContext,
    item: &'static str,
    key: Option<&str>,
    value: &'a BufferedWireValue<T>,
) -> Result<Option<&'a T>, YfError> {
    if let Some(details) = value.invalid_details() {
        ctx.dropped_item(
            item,
            key,
            ProjectionIssue::InvalidField {
                field: item,
                details: details.to_string(),
            },
        )?;
        return Ok(None);
    }
    Ok(WireField::as_ref(value))
}

fn optional_string(
    ctx: &mut ProjectionContext,
    path: &'static str,
    key: Option<&str>,
    field: &'static str,
    value: &WireValue<String>,
) -> Result<Option<String>, YfError> {
    Ok(nonempty_string(
        value.optional_cloned_field(ctx, path, key, field)?,
    ))
}

fn filing_date(
    ctx: &mut ProjectionContext,
    key: Option<&str>,
    row: &FilingNode,
) -> Result<Option<NaiveDate>, YfError> {
    let invalid = |details: String| ProjectionIssue::InvalidField {
        field: "date",
        details,
    };

    if let Some(details) = row.date.invalid_details() {
        ctx.dropped_item("sec_filing", key, invalid(details.to_string()))?;
        return Ok(None);
    }

    if let Some(text) = row
        .date
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return match NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            Ok(date) => Ok(Some(date)),
            Err(err) => {
                ctx.dropped_item(
                    "sec_filing",
                    key,
                    invalid(format!("`{text}` is not a YYYY-MM-DD date: {err}")),
                )?;
                Ok(None)
            }
        };
    }

    // Fall back to the epoch timestamp when the date string is absent.
    match &row.epoch_date {
        WireValue::Valid(epoch) => match i64_to_date(*epoch) {
            Ok(date) => Ok(Some(date)),
            Err(err) => {
                ctx.dropped_item(
                    "sec_filing",
                    key,
                    ProjectionIssue::InvalidField {
                        field: "epochDate",
                        details: err.to_string(),
                    },
                )?;
                Ok(None)
            }
        },
        WireValue::Invalid(details) => {
            ctx.dropped_item(
                "sec_filing",
                key,
                ProjectionIssue::InvalidField {
                    field: "epochDate",
                    details: details.message().to_string(),
                },
            )?;
            Ok(None)
        }
        WireValue::Missing => {
            ctx.dropped_item(
                "sec_filing",
                key,
                ProjectionIssue::MissingRequiredField { field: "date" },
            )?;
            Ok(None)
        }
    }
}

fn map_filing(ctx: &mut ProjectionContext, row: &FilingNode) -> Result<Option<SecFiling>, YfError> {
    let edgar_url = optional_string(
        ctx,
        "secFilings.filings[].edgarUrl",
        None,
        "edgarUrl",
        &row.edgar_url,
    )?;
    let key = edgar_url.clone().or_else(|| row.date.as_ref().cloned());
    let key = key.as_deref();

    let Some(date) = filing_date(ctx, key, row)? else {
        return Ok(None);
    };

    let filing_type = match &row.filing_type {
        WireValue::Valid(value) if !value.trim().is_empty() => value.trim().to_string(),
        WireValue::Invalid(details) => {
            ctx.dropped_item(
                "sec_filing",
                key,
                ProjectionIssue::InvalidField {
                    field: "type",
                    details: details.message().to_string(),
                },
            )?;
            return Ok(None);
        }
        _ => {
            ctx.dropped_item(
                "sec_filing",
                key,
                ProjectionIssue::MissingRequiredField { field: "type" },
            )?;
            return Ok(None);
        }
    };

    let title = optional_string(ctx, "secFilings.filings[].title", key, "title", &row.title)?;

    let mut exhibits = Vec::new();
    if let Some(nodes) =
        row.exhibits
            .optional_ref_field(ctx, "secFilings.filings[].exhibits", key, "exhibits")?
    {
        for node in nodes {
            let Some(node) = required_item(ctx, "sec_filing_exhibit", key, node)? else {
                continue;
            };
            if let Some(exhibit) = map_exhibit(ctx, key, node)? {
                exhibits.push(exhibit);
            }
        }
    }

    Ok(Some(SecFiling {
        date,
        filing_type,
        title,
        edgar_url,
        exhibits,
    }))
}

fn map_exhibit(
    ctx: &mut ProjectionContext,
    key: Option<&str>,
    node: &ExhibitNode,
) -> Result<Option<SecFilingExhibit>, YfError> {
    let exhibit_type = optional_string(
        ctx,
        "secFilings.filings[].exhibits[].type",
        key,
        "type",
        &node.exhibit_type,
    )?;
    let url = optional_string(
        ctx,
        "secFilings.filings[].exhibits[].url",
        key,
        "url",
        &node.url,
    )?;
    let download_url = optional_string(
        ctx,
        "secFilings.filings[].exhibits[].downloadUrl",
        key,
        "downloadUrl",
        &node.download_url,
    )?;

    match (exhibit_type, url) {
        (Some(exhibit_type), Some(url)) => Ok(Some(SecFilingExhibit {
            exhibit_type,
            url,
            download_url,
        })),
        (exhibit_type, url) => {
            let mut fields = Vec::new();
            if exhibit_type.is_none() {
                fields.push("type");
            }
            if url.is_none() {
                fields.push("url");
            }
            ctx.dropped_item(
                "sec_filing_exhibit",
                key,
                ProjectionIssue::MissingRequiredFields { fields },
            )?;
            Ok(None)
        }
    }
}
