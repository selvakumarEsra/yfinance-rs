//! Shared transport and cell projection for the visualization API.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeDelta, Utc};
use paft::Decimal;
use paft::money::Money;
use rust_decimal::prelude::ToPrimitive;
use serde_json::{Map, Value};
use url::Url;

use super::query::VizQuery;
use super::wire::{VizTable, validate_body};
use crate::core::client::CacheEndpoint;
use crate::core::currency_resolver::ResolvedCurrencyUnit;
use crate::core::net::{AuthFetchConfig, AuthMode, fetch_text_with_auth_retry};
use crate::core::wire::decimal_from_json_value;
use crate::core::{CallOptions, ProjectionContext, ProjectionIssue, YfClient, YfError};

/// Yahoo caps the visualization page size at 100 rows.
pub(super) const MAX_PAGE_SIZE: u32 = 100;

/// Parts of a visualization POST body.
pub(super) struct VizRequest {
    pub(super) entity_id_type: &'static str,
    pub(super) sort_field: &'static str,
    pub(super) include_fields: &'static [&'static str],
    pub(super) size: u32,
    pub(super) offset: Option<u32>,
    pub(super) query: VizQuery,
}

impl VizRequest {
    pub(super) fn into_body(self) -> Value {
        let mut body = Map::new();
        body.insert("sortType".into(), Value::String("DESC".into()));
        body.insert(
            "entityIdType".into(),
            Value::String(self.entity_id_type.into()),
        );
        body.insert("sortField".into(), Value::String(self.sort_field.into()));
        body.insert(
            "includeFields".into(),
            Value::Array(
                self.include_fields
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
        body.insert("size".into(), Value::from(self.size));
        if let Some(offset) = self.offset {
            body.insert("offset".into(), Value::from(offset));
        }
        body.insert("query".into(), self.query.into_wire_value());
        Value::Object(body)
    }
}

pub(super) fn validate_limit(limit: u32) -> Result<u32, YfError> {
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(YfError::InvalidParams(format!(
            "visualization limit must be between 1 and {MAX_PAGE_SIZE}, got {limit}"
        )));
    }
    Ok(limit)
}

/// POSTs a visualization request through the crumb-auth path (`core::net`).
pub(super) async fn post_visualization(
    client: &YfClient,
    base: &Url,
    options: &CallOptions,
    body: &Value,
    endpoint: &'static str,
    fixture_key: &str,
) -> Result<String, YfError> {
    let mut url = base.clone();
    url.query_pairs_mut()
        .append_pair("lang", "en-US")
        .append_pair("region", "US");

    let body_json = serde_json::to_string(body).map_err(YfError::json)?;
    let (text, _) = fetch_text_with_auth_retry(
        client,
        url,
        AuthFetchConfig {
            auth_mode: AuthMode::RequiredCrumb,
            cache_endpoint: CacheEndpoint::Visualization,
            options,
            cache_body: Some(&body_json),
            endpoint,
            fixture_key,
            ext: "json",
            retry_on_invalid_crumb_body: true,
            cache_validator: Some(validate_body),
        },
        |url| {
            client
                .http()
                .post(url)
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .body(body_json.clone())
        },
    )
    .await?;
    Ok(text)
}

/// One decoded row, addressed by column id.
pub(super) struct VizRow<'a> {
    table: &'a VizTable,
    cells: &'a [Value],
    key: String,
}

impl VizRow<'_> {
    pub(super) const fn key(&self) -> &str {
        self.key.as_str()
    }

    /// Returns the non-null cell for a column id.
    pub(super) fn cell(&self, id: &str) -> Option<&Value> {
        let idx = self.table.column(id)?;
        self.cells.get(idx).filter(|value| !value.is_null())
    }
}

/// Splits the table into rows; non-array rows are dropped with warnings.
pub(super) fn rows<'a>(
    ctx: &mut ProjectionContext,
    table: &'a VizTable,
    item: &'static str,
    key_column: &'static str,
) -> Result<Vec<VizRow<'a>>, YfError> {
    let key_idx = table.column(key_column);
    let mut out = Vec::with_capacity(table.rows.len());
    for (idx, row) in table.rows.iter().enumerate() {
        let Value::Array(cells) = row else {
            ctx.dropped_item(
                item,
                Some(&format!("rows[{idx}]")),
                ProjectionIssue::InvalidField {
                    field: "rows",
                    details: "expected an array of cells".into(),
                },
            )?;
            continue;
        };
        let key = key_idx
            .and_then(|i| cells.get(i))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map_or_else(|| format!("rows[{idx}]"), str::to_string);
        out.push(VizRow { table, cells, key });
    }
    Ok(out)
}

fn invalid(field: &'static str, value: &Value, expected: &str) -> ProjectionIssue {
    ProjectionIssue::InvalidField {
        field,
        details: format!("expected {expected}, got {value}"),
    }
}

pub(super) fn opt_string(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    id: &'static str,
) -> Result<Option<String>, YfError> {
    match row.cell(id) {
        None => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            Ok((!value.is_empty()).then(|| value.to_string()))
        }
        Some(Value::Number(value)) => Ok(Some(value.to_string())),
        Some(other) => {
            ctx.omitted_present_field(id, Some(row.key()), invalid(id, other, "string"))?;
            Ok(None)
        }
    }
}

/// A required string; a missing value drops the row.
pub(super) fn required_string(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    item: &'static str,
    id: &'static str,
) -> Result<Option<String>, YfError> {
    let value = opt_string(ctx, row, id)?;
    if value.is_none() {
        ctx.dropped_item(
            item,
            Some(row.key()),
            ProjectionIssue::MissingRequiredField { field: id },
        )?;
    }
    Ok(value)
}

pub(super) fn opt_decimal(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    id: &'static str,
) -> Result<Option<Decimal>, YfError> {
    let Some(value) = row.cell(id) else {
        return Ok(None);
    };
    if value
        .as_str()
        .is_some_and(|s| s.trim().is_empty() || s.trim() == "-")
    {
        return Ok(None);
    }
    match decimal_from_json_value(value.clone()) {
        Ok(decimal) => Ok(Some(decimal)),
        Err(details) => {
            ctx.omitted_present_field(
                id,
                Some(row.key()),
                ProjectionIssue::InvalidField { field: id, details },
            )?;
            Ok(None)
        }
    }
}

pub(super) fn opt_u64(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    id: &'static str,
) -> Result<Option<u64>, YfError> {
    let Some(decimal) = opt_decimal(ctx, row, id)? else {
        return Ok(None);
    };
    if decimal.fract().is_zero()
        && let Some(value) = decimal.to_u64()
    {
        return Ok(Some(value));
    }
    ctx.omitted_present_field(
        id,
        Some(row.key()),
        ProjectionIssue::ConversionFailed {
            target: "non-negative integer",
        },
    )?;
    Ok(None)
}

pub(super) fn opt_bool(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    id: &'static str,
) -> Result<Option<bool>, YfError> {
    let Some(value) = row.cell(id) else {
        return Ok(None);
    };
    let parsed = match value {
        Value::Bool(value) => Some(*value),
        Value::Number(n) => match n.as_i64() {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "yes" | "true" | "y" | "1" => Some(true),
            "no" | "false" | "n" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    };
    if parsed.is_none() {
        ctx.omitted_present_field(id, Some(row.key()), invalid(id, value, "boolean"))?;
    }
    Ok(parsed)
}

/// Converts a provider amount into money, recording missing/invalid currency.
pub(super) fn opt_money(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    id: &'static str,
    unit: Option<&ResolvedCurrencyUnit>,
    currency_issue: Option<&ProjectionIssue>,
) -> Result<Option<Money>, YfError> {
    let Some(decimal) = opt_decimal(ctx, row, id)? else {
        return Ok(None);
    };
    let Some(unit) = unit else {
        let issue = currency_issue
            .cloned()
            .unwrap_or(ProjectionIssue::CurrencyUnresolved);
        ctx.omitted_present_field(id, Some(row.key()), issue)?;
        return Ok(None);
    };
    let money = unit.money_from_provider_decimal(decimal);
    if money.is_none() {
        ctx.omitted_present_field(
            id,
            Some(row.key()),
            ProjectionIssue::ConversionFailed { target: "money" },
        )?;
    }
    Ok(money)
}

/// Result of reading a date-like cell.
enum DateCell<T> {
    Missing,
    Parsed(T),
    Invalid(String),
}

fn parse_datetime_value(value: &Value, gmt_offset_ms: Option<i64>) -> DateCell<DateTime<Utc>> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .and_then(|raw| {
                // Heuristic: values above 1e11 are epoch milliseconds.
                if raw.abs() > 100_000_000_000 {
                    DateTime::from_timestamp_millis(raw)
                } else {
                    DateTime::from_timestamp(raw, 0)
                }
            })
            .map_or_else(
                || DateCell::Invalid(format!("unsupported timestamp {n}")),
                DateCell::Parsed,
            ),
        Value::String(raw) => {
            let raw = raw.trim();
            if raw.is_empty() {
                return DateCell::Missing;
            }
            parse_datetime_str(raw, gmt_offset_ms).map_or_else(
                || DateCell::Invalid(format!("unparseable date {raw:?}")),
                DateCell::Parsed,
            )
        }
        other => DateCell::Invalid(format!("expected date string, got {other}")),
    }
}

fn parse_datetime_str(raw: &str, gmt_offset_ms: Option<i64>) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    let naive = ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|fmt| NaiveDateTime::parse_from_str(raw, fmt).ok())
        .or_else(|| {
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
        })?;
    // Offset-less timestamps are local exchange time when Yahoo sends a GMT offset,
    // otherwise UTC.
    let offset = TimeDelta::try_milliseconds(gmt_offset_ms.unwrap_or(0))?;
    naive.checked_sub_signed(offset).map(|dt| dt.and_utc())
}

fn parse_date_value(value: &Value) -> DateCell<NaiveDate> {
    if let Value::String(raw) = value
        && let Ok(date) = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
    {
        return DateCell::Parsed(date);
    }
    match parse_datetime_value(value, None) {
        DateCell::Parsed(dt) => DateCell::Parsed(dt.date_naive()),
        DateCell::Missing => DateCell::Missing,
        DateCell::Invalid(details) => DateCell::Invalid(details),
    }
}

/// Outcome of reading a date cell whose failure drops the whole row.
pub(super) enum RowValue<T> {
    /// Keep the row with this (possibly absent) value.
    Keep(Option<T>),
    /// The row was dropped and a warning recorded.
    Dropped,
}

fn drop_for_date<T>(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    item: &'static str,
    id: &'static str,
    cell: DateCell<T>,
    required: bool,
) -> Result<RowValue<T>, YfError> {
    let issue = match cell {
        DateCell::Parsed(value) => return Ok(RowValue::Keep(Some(value))),
        DateCell::Missing if !required => return Ok(RowValue::Keep(None)),
        DateCell::Missing => ProjectionIssue::MissingRequiredField { field: id },
        DateCell::Invalid(details) => ProjectionIssue::InvalidField { field: id, details },
    };
    ctx.dropped_item(item, Some(row.key()), issue)?;
    Ok(RowValue::Dropped)
}

/// Reads a required timestamp. `Ok(None)` means the row was dropped (with a warning).
pub(super) fn required_datetime(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    item: &'static str,
    id: &'static str,
    gmt_offset_ms: Option<i64>,
) -> Result<Option<DateTime<Utc>>, YfError> {
    let cell = row.cell(id).map_or(DateCell::Missing, |value| {
        parse_datetime_value(value, gmt_offset_ms)
    });
    Ok(match drop_for_date(ctx, row, item, id, cell, true)? {
        RowValue::Keep(value) => value,
        RowValue::Dropped => None,
    })
}

/// Reads an optional timestamp; an unparseable present value drops the row.
pub(super) fn optional_datetime(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    item: &'static str,
    id: &'static str,
) -> Result<RowValue<DateTime<Utc>>, YfError> {
    let cell = row
        .cell(id)
        .map_or(DateCell::Missing, |value| parse_datetime_value(value, None));
    drop_for_date(ctx, row, item, id, cell, false)
}

/// Reads an optional calendar date; an unparseable present value drops the row.
pub(super) fn optional_date(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    item: &'static str,
    id: &'static str,
) -> Result<RowValue<NaiveDate>, YfError> {
    let cell = row.cell(id).map_or(DateCell::Missing, parse_date_value);
    drop_for_date(ctx, row, item, id, cell, false)
}

/// Reads Yahoo's `gmtOffsetMilliSeconds` column, when sent.
pub(super) fn gmt_offset_ms(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    id: &'static str,
) -> Result<Option<i64>, YfError> {
    let Some(decimal) = opt_decimal(ctx, row, id)? else {
        return Ok(None);
    };
    match decimal.to_i64() {
        Some(value) if decimal.fract().is_zero() => Ok(Some(value)),
        _ => {
            ctx.omitted_present_field(
                id,
                Some(row.key()),
                ProjectionIssue::ConversionFailed {
                    target: "milliseconds",
                },
            )?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_parser_accepts_yahoo_shapes() {
        let expected = DateTime::parse_from_rfc3339("2025-10-30T20:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        for value in [
            Value::from("2025-10-30T20:30:00Z"),
            Value::from("2025-10-30T20:30:00.000Z"),
            Value::from("2025-10-30T16:30:00-04:00"),
            Value::from(1_761_856_200_000_i64),
            Value::from(1_761_856_200_i64),
        ] {
            match parse_datetime_value(&value, None) {
                DateCell::Parsed(dt) => assert_eq!(dt, expected, "{value}"),
                _ => panic!("failed to parse {value}"),
            }
        }
        match parse_datetime_value(&Value::from("2025-10-30T16:30:00"), Some(-14_400_000)) {
            DateCell::Parsed(dt) => assert_eq!(dt, expected),
            _ => panic!("naive local time with offset"),
        }
        assert!(matches!(
            parse_datetime_value(&Value::from("not-a-date"), None),
            DateCell::Invalid(_)
        ));
    }

    #[test]
    fn limit_validation_bounds() {
        assert!(validate_limit(0).is_err());
        assert!(validate_limit(101).is_err());
        assert_eq!(validate_limit(100).unwrap(), 100);
    }
}
