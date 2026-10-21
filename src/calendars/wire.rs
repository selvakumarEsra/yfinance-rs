//! Private serde shapes for the `v1/finance/visualization` response.
//!
//! Yahoo returns a columnar table:
//! `finance.result[0].documents[0].{columns: [{id,label,type}], rows: [[cell,...]]}`.
//! Cells are matched to columns by the column `id`, never by position.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::YfError;

#[derive(Debug, Deserialize)]
pub(super) struct WireEnvelope {
    pub(super) finance: WireFinance,
}

#[derive(Debug, Deserialize)]
pub(super) struct WireFinance {
    #[serde(default)]
    pub(super) result: Option<Vec<WireResult>>,
    #[serde(default)]
    pub(super) error: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WireResult {
    #[serde(default)]
    pub(super) documents: Option<Vec<WireDocument>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WireDocument {
    #[serde(default)]
    pub(super) columns: Option<Vec<WireColumn>>,
    #[serde(default)]
    pub(super) rows: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WireColumn {
    #[serde(default)]
    pub(super) id: Option<String>,
}

/// A decoded visualization table: column-id index plus raw rows.
#[derive(Debug, Default)]
pub(super) struct VizTable {
    columns: HashMap<String, usize>,
    pub(super) rows: Vec<Value>,
}

impl VizTable {
    pub(super) fn column(&self, id: &str) -> Option<usize> {
        self.columns.get(id).copied()
    }
}

/// Parses the envelope, rejecting Yahoo API errors.
///
/// A response without `result[0].documents[0]` decodes to an empty table.
pub(super) fn parse_table(body: &str) -> Result<VizTable, YfError> {
    let env: WireEnvelope = serde_json::from_str(body).map_err(YfError::json)?;
    reject_error(&env)?;

    let Some(document) = env
        .finance
        .result
        .and_then(|result| result.into_iter().next())
        .and_then(|result| result.documents)
        .and_then(|documents| documents.into_iter().next())
    else {
        return Ok(VizTable::default());
    };

    let columns = document
        .columns
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(idx, column)| column.id.map(|id| (id, idx)))
        .collect();

    Ok(VizTable {
        columns,
        rows: document.rows.unwrap_or_default(),
    })
}

pub(super) fn validate_body(body: &str) -> Result<(), YfError> {
    let env: WireEnvelope = serde_json::from_str(body).map_err(YfError::json)?;
    reject_error(&env)
}

fn reject_error(env: &WireEnvelope) -> Result<(), YfError> {
    match env.finance.error.as_ref() {
        Some(error) if !error.is_null() => Err(YfError::Api(error.to_string())),
        _ => Ok(()),
    }
}
