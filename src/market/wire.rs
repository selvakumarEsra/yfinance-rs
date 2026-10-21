use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use serde_json::Value;

use super::model::{MarketStatus, MarketSummaryEntry};
use crate::YfError;
use crate::core::conversions::i64_to_datetime;
use crate::core::diagnostics::{WireProjection, optional_projected, required_wire_value};
use crate::core::wire::{JsonDecimal, WireValue};
use crate::core::yahoo_vocab::parse_yahoo_quote_type;
use crate::core::{ProjectionContext, ProjectionIssue};

/* ---------------- markettime ---------------- */

#[derive(Deserialize)]
struct MarketTimeEnvelope {
    finance: Option<MarketTimeFinance>,
}

#[derive(Deserialize)]
struct MarketTimeFinance {
    #[serde(rename = "marketTimes")]
    market_times: Option<Vec<MarketTimesGroup>>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct MarketTimesGroup {
    #[serde(rename = "marketTime")]
    market_time: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct WireMarketTime {
    #[serde(default)]
    id: WireValue<String>,
    #[serde(default)]
    name: WireValue<String>,
    #[serde(default)]
    status: WireValue<String>,
    #[serde(default)]
    message: WireValue<String>,
    #[serde(default)]
    open: WireValue<String>,
    #[serde(default)]
    close: WireValue<String>,
    #[serde(default)]
    time: WireValue<String>,
    #[serde(default)]
    timezone: Option<Value>,
}

fn first_market_time(body: &str) -> Result<Value, YfError> {
    let env: MarketTimeEnvelope = serde_json::from_str(body).map_err(YfError::json)?;
    let finance = env
        .finance
        .ok_or_else(|| YfError::MissingData("markettime finance missing".into()))?;
    if let Some(error) = finance.error.as_ref().filter(|error| !error.is_null()) {
        return Err(YfError::Api(format!("markettime error: {error}")));
    }
    finance
        .market_times
        .and_then(|groups| groups.into_iter().next())
        .and_then(|group| group.market_time)
        .and_then(|times| times.into_iter().next())
        .ok_or_else(|| YfError::MissingData("markettime marketTime missing".into()))
}

pub(super) fn validate_market_time_body(body: &str) -> Result<(), YfError> {
    first_market_time(body).map(|_| ())
}

/// Returns `true` when the market time entry describes the U.S. market (`id == "us"`).
pub(super) fn market_time_is_us(body: &str) -> Result<bool, YfError> {
    let raw = first_market_time(body)?;
    Ok(raw
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| id.trim().eq_ignore_ascii_case("us")))
}

pub(super) fn parse_market_time_body(
    body: &str,
    ctx: &mut ProjectionContext,
) -> Result<MarketStatus, YfError> {
    let raw = first_market_time(body)?;
    let wire: WireMarketTime = serde_json::from_value(raw).map_err(YfError::json)?;
    let key = wire.id.as_ref().cloned();
    let key = key.as_deref();

    let open = wire.open.optional_cloned(ctx, "marketTime.open", key)?;
    let close = wire.close.optional_cloned(ctx, "marketTime.close", key)?;
    let time = wire.time.optional_cloned(ctx, "marketTime.time", key)?;
    let open = optional_projected(ctx, "marketTime.open", key, open, |v| {
        parse_iso_datetime("open", &v)
    })?;
    let close = optional_projected(ctx, "marketTime.close", key, close, |v| {
        parse_iso_datetime("close", &v)
    })?;
    let as_of = optional_projected(ctx, "marketTime.time", key, time, |v| {
        parse_iso_datetime("time", &v)
    })?;

    let tz = project_timezone(ctx, key, wire.timezone)?;

    Ok(MarketStatus {
        market_id: wire.id.optional_cloned(ctx, "marketTime.id", key)?,
        name: wire.name.optional_cloned(ctx, "marketTime.name", key)?,
        status: wire.status.optional_cloned(ctx, "marketTime.status", key)?,
        message: wire
            .message
            .optional_cloned(ctx, "marketTime.message", key)?,
        open,
        close,
        timezone: tz.timezone,
        timezone_abbreviation: tz.abbreviation,
        utc_offset_seconds: tz.utc_offset_seconds,
        as_of,
    })
}

fn parse_iso_datetime(field: &'static str, value: &str) -> Result<DateTime<Utc>, ProjectionIssue> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| ProjectionIssue::InvalidField {
            field,
            details: format!("{value:?}: {err}"),
        })
}

#[derive(Default)]
struct ProjectedTimezone {
    timezone: Option<Tz>,
    abbreviation: Option<String>,
    utc_offset_seconds: Option<i32>,
}

/// Yahoo sends `timezone` as a one-element array of
/// `{"$text": "America/New_York", "short": "EDT", "gmtoffset": "-14400000", "dst": "true"}`.
fn project_timezone(
    ctx: &mut ProjectionContext,
    key: Option<&str>,
    value: Option<Value>,
) -> Result<ProjectedTimezone, YfError> {
    let node = match value {
        None | Some(Value::Null) => return Ok(ProjectedTimezone::default()),
        Some(Value::Array(items)) => items.into_iter().next(),
        Some(other) => Some(other),
    };
    let Some(Value::Object(node)) = node else {
        ctx.omitted_present_field(
            "marketTime.timezone",
            key,
            ProjectionIssue::InvalidField {
                field: "timezone",
                details: "expected timezone object".into(),
            },
        )?;
        return Ok(ProjectedTimezone::default());
    };

    let name = node.get("$text").and_then(Value::as_str).map(str::trim);
    let timezone = match name.filter(|name| !name.is_empty()) {
        Some(name) => match name.parse::<Tz>() {
            Ok(tz) => Some(tz),
            Err(err) => {
                ctx.omitted_present_field(
                    "marketTime.timezone",
                    key,
                    ProjectionIssue::InvalidField {
                        field: "timezone",
                        details: err.to_string(),
                    },
                )?;
                None
            }
        },
        None => None,
    };

    let abbreviation = node
        .get("short")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let utc_offset_seconds = optional_projected(
        ctx,
        "marketTime.timezone.gmtoffset",
        key,
        node.get("gmtoffset").filter(|v| !v.is_null()).cloned(),
        |value| {
            let millis = match &value {
                Value::String(s) => s.trim().parse::<i64>().ok(),
                Value::Number(n) => n.as_i64(),
                _ => None,
            };
            millis
                .and_then(|ms| i32::try_from(ms / 1000).ok())
                .ok_or_else(|| ProjectionIssue::InvalidField {
                    field: "gmtoffset",
                    details: format!("expected integer milliseconds, got {value}"),
                })
        },
    )?;

    Ok(ProjectedTimezone {
        timezone,
        abbreviation,
        utc_offset_seconds,
    })
}

/* ---------------- marketSummary ---------------- */

#[derive(Deserialize)]
struct SummaryEnvelope {
    #[serde(rename = "marketSummaryResponse")]
    response: Option<SummaryResponse>,
}

#[derive(Deserialize)]
struct SummaryResponse {
    result: Option<Vec<Value>>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct WireSummaryEntry {
    #[serde(default)]
    symbol: WireValue<String>,
    #[serde(rename = "shortName", default)]
    short_name: WireValue<String>,
    #[serde(rename = "longName", default)]
    long_name: WireValue<String>,
    #[serde(default)]
    exchange: WireValue<String>,
    #[serde(rename = "fullExchangeName", default)]
    full_exchange_name: WireValue<String>,
    #[serde(rename = "quoteType", default)]
    quote_type: WireValue<String>,
    #[serde(rename = "marketState", default)]
    market_state: WireValue<String>,
    #[serde(default)]
    currency: WireValue<String>,
    #[serde(rename = "regularMarketPrice", default)]
    price: WireValue<JsonDecimal>,
    #[serde(rename = "regularMarketChange", default)]
    change: WireValue<JsonDecimal>,
    #[serde(rename = "regularMarketChangePercent", default)]
    change_percent: WireValue<JsonDecimal>,
    #[serde(rename = "regularMarketTime", default)]
    market_time: WireValue<i64>,
}

fn summary_entries(body: &str) -> Result<Vec<Value>, YfError> {
    let env: SummaryEnvelope = serde_json::from_str(body).map_err(YfError::json)?;
    let response = env
        .response
        .ok_or_else(|| YfError::MissingData("marketSummaryResponse missing".into()))?;
    if let Some(error) = response.error.as_ref().filter(|error| !error.is_null()) {
        return Err(YfError::Api(format!("marketSummary error: {error}")));
    }
    response
        .result
        .ok_or_else(|| YfError::MissingData("marketSummary result missing".into()))
}

pub(super) fn validate_summary_body(body: &str) -> Result<(), YfError> {
    summary_entries(body).map(|_| ())
}

pub(super) fn parse_summary_body(
    body: &str,
    ctx: &mut ProjectionContext,
) -> Result<Vec<MarketSummaryEntry>, YfError> {
    let entries = summary_entries(body)?;
    let mut out = Vec::with_capacity(entries.len());
    for (idx, raw) in entries.into_iter().enumerate() {
        let raw_key = diag_key(&raw, idx);
        let wire = match serde_json::from_value::<WireSummaryEntry>(raw) {
            Ok(wire) => wire,
            Err(err) => {
                ctx.dropped_item(
                    "market_summary_entry",
                    Some(raw_key.as_str()),
                    ProjectionIssue::InvalidField {
                        field: "entry",
                        details: err.to_string(),
                    },
                )?;
                continue;
            }
        };
        if let Some(entry) = project_summary_entry(ctx, &raw_key, &wire)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn project_summary_entry(
    ctx: &mut ProjectionContext,
    raw_key: &str,
    wire: &WireSummaryEntry,
) -> Result<Option<MarketSummaryEntry>, YfError> {
    const ITEM: &str = "market_summary_entry";
    let Some(symbol) = required_wire_value(ctx, ITEM, Some(raw_key), "symbol", &wire.symbol)?
        .map(|s| s.trim().to_string())
    else {
        return Ok(None);
    };
    if symbol.is_empty() {
        ctx.dropped_item(
            ITEM,
            Some(raw_key),
            ProjectionIssue::MissingRequiredField { field: "symbol" },
        )?;
        return Ok(None);
    }
    let key = Some(symbol.as_str());
    let Some(price) =
        required_wire_value(ctx, ITEM, key, "regularMarketPrice", &wire.price)?.copied()
    else {
        return Ok(None);
    };

    let quote_type = wire.quote_type.optional_cloned(ctx, "quoteType", key)?;
    let kind = optional_projected(
        ctx,
        "quoteType",
        key,
        quote_type.filter(|s| !s.trim().is_empty()),
        |value| {
            parse_yahoo_quote_type(&value).map_err(|err| ProjectionIssue::InvalidField {
                field: "quoteType",
                details: err.to_string(),
            })
        },
    )?;
    let market_time = wire
        .market_time
        .optional_copied(ctx, "regularMarketTime", key)?;
    let market_time = optional_projected(ctx, "regularMarketTime", key, market_time, |ts| {
        i64_to_datetime(ts).map_err(|err| ProjectionIssue::InvalidField {
            field: "regularMarketTime",
            details: err.to_string(),
        })
    })?;

    let short_name = wire.short_name.optional_cloned(ctx, "shortName", key)?;
    let long_name = wire.long_name.optional_cloned(ctx, "longName", key)?;

    Ok(Some(MarketSummaryEntry {
        name: nonempty(short_name).or_else(|| nonempty(long_name)),
        exchange: nonempty(wire.exchange.optional_cloned(ctx, "exchange", key)?),
        exchange_name: nonempty(wire.full_exchange_name.optional_cloned(
            ctx,
            "fullExchangeName",
            key,
        )?),
        kind,
        market_state: nonempty(wire.market_state.optional_cloned(ctx, "marketState", key)?),
        price: price.into_decimal(),
        change: wire.change.optional_copied_map(
            ctx,
            "regularMarketChange",
            key,
            JsonDecimal::into_decimal,
        )?,
        change_percent: wire.change_percent.optional_copied_map(
            ctx,
            "regularMarketChangePercent",
            key,
            JsonDecimal::into_decimal,
        )?,
        currency: nonempty(wire.currency.optional_cloned(ctx, "currency", key)?),
        market_time,
        symbol,
    }))
}

fn nonempty(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn diag_key(value: &Value, idx: usize) -> String {
    value
        .get("symbol")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
        .map_or_else(|| format!("result[{idx}]"), ToString::to_string)
}
