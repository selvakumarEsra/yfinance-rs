use paft::domain::Instrument;
use serde::Deserialize;
use serde_json::Value;

use super::model::LookupResult;
use crate::YfError;
use crate::core::diagnostics::{WireProjection, optional_projected, required_wire_value};
use crate::core::wire::{JsonDecimal, WireValue};
use crate::core::yahoo_vocab::{parse_yahoo_exchange, parse_yahoo_quote_type};
use crate::core::{ProjectionContext, ProjectionIssue};

#[derive(Deserialize)]
struct LookupEnvelope {
    finance: Option<LookupFinance>,
}

#[derive(Deserialize)]
struct LookupFinance {
    result: Option<Vec<LookupResultNode>>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct LookupResultNode {
    documents: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct WireDocument {
    #[serde(default)]
    symbol: WireValue<String>,
    #[serde(rename = "shortName", default)]
    short_name: WireValue<String>,
    #[serde(rename = "longName", default)]
    long_name: WireValue<String>,
    #[serde(rename = "quoteType", default)]
    quote_type: WireValue<String>,
    #[serde(default)]
    exchange: WireValue<String>,
    #[serde(rename = "industryName", default)]
    industry_name: WireValue<String>,
    #[serde(rename = "regularMarketPrice", default)]
    price: WireValue<JsonDecimal>,
    #[serde(rename = "regularMarketChange", default)]
    change: WireValue<JsonDecimal>,
    #[serde(rename = "regularMarketPercentChange", default)]
    change_percent: WireValue<JsonDecimal>,
}

/// Returns the raw documents, or an empty list when Yahoo returns no result node.
fn documents(body: &str) -> Result<Vec<Value>, YfError> {
    let env: LookupEnvelope = serde_json::from_str(body).map_err(YfError::json)?;
    let finance = env
        .finance
        .ok_or_else(|| YfError::MissingData("lookup finance missing".into()))?;
    if let Some(error) = finance.error.as_ref().filter(|error| !error.is_null()) {
        return Err(YfError::Api(format!("lookup error: {error}")));
    }
    Ok(finance
        .result
        .and_then(|result| result.into_iter().next())
        .and_then(|node| node.documents)
        .unwrap_or_default())
}

pub(super) fn validate_lookup_body(body: &str) -> Result<(), YfError> {
    documents(body).map(|_| ())
}

pub(super) fn parse_lookup_body(
    body: &str,
    ctx: &mut ProjectionContext,
) -> Result<Vec<LookupResult>, YfError> {
    let docs = documents(body)?;
    let mut out = Vec::with_capacity(docs.len());
    for (idx, raw) in docs.into_iter().enumerate() {
        let raw_key = diag_key(&raw, idx);
        let wire = match serde_json::from_value::<WireDocument>(raw) {
            Ok(wire) => wire,
            Err(err) => {
                ctx.dropped_item(
                    "lookup_result",
                    Some(raw_key.as_str()),
                    ProjectionIssue::InvalidField {
                        field: "document",
                        details: err.to_string(),
                    },
                )?;
                continue;
            }
        };
        if let Some(result) = project_document(ctx, &raw_key, &wire)? {
            out.push(result);
        }
    }
    Ok(out)
}

fn project_document(
    ctx: &mut ProjectionContext,
    raw_key: &str,
    wire: &WireDocument,
) -> Result<Option<LookupResult>, YfError> {
    const ITEM: &str = "lookup_result";
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

    let quote_type = nonempty(wire.quote_type.optional_cloned(ctx, "quoteType", key)?);
    let kind = optional_projected(ctx, "quoteType", key, quote_type.clone(), |value| {
        parse_yahoo_quote_type(&value).map_err(|err| ProjectionIssue::InvalidField {
            field: "quoteType",
            details: err.to_string(),
        })
    })?;
    let raw_exchange = nonempty(wire.exchange.optional_cloned(ctx, "exchange", key)?);
    let exchange = optional_projected(ctx, "exchange", key, raw_exchange.clone(), |value| {
        parse_yahoo_exchange(&value).map_err(|err| ProjectionIssue::InvalidField {
            field: "exchange",
            details: err.to_string(),
        })
    })?;

    let instrument = match kind.clone() {
        Some(kind) => {
            let built = match exchange.clone() {
                Some(exchange) => Instrument::from_symbol_and_exchange(&symbol, exchange, kind),
                None => Instrument::from_symbol(&symbol, kind),
            };
            optional_projected(ctx, "instrument", key, Some(built), |built| {
                built.map_err(|err| ProjectionIssue::InvalidField {
                    field: "symbol",
                    details: err.to_string(),
                })
            })?
        }
        None => None,
    };

    let short_name = nonempty(wire.short_name.optional_cloned(ctx, "shortName", key)?);
    let long_name = nonempty(wire.long_name.optional_cloned(ctx, "longName", key)?);

    Ok(Some(LookupResult {
        instrument,
        name: short_name.or(long_name),
        kind,
        quote_type,
        exchange,
        raw_exchange,
        industry: nonempty(
            wire.industry_name
                .optional_cloned(ctx, "industryName", key)?,
        ),
        last_price: wire.price.optional_copied_map(
            ctx,
            "regularMarketPrice",
            key,
            JsonDecimal::into_decimal,
        )?,
        change: wire.change.optional_copied_map(
            ctx,
            "regularMarketChange",
            key,
            JsonDecimal::into_decimal,
        )?,
        change_percent: wire.change_percent.optional_copied_map(
            ctx,
            "regularMarketPercentChange",
            key,
            JsonDecimal::into_decimal,
        )?,
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
        .map_or_else(|| format!("documents[{idx}]"), ToString::to_string)
}
