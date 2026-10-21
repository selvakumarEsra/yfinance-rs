//! Private query DSL for Yahoo's `v1/finance/visualization` POST API.
//!
//! The screener query types in [`crate::screener::query`] are phantom-typed per
//! screener universe (equity / fund / ETF), their field constructors are tied to
//! the screener vocabulary, and their sealed value types only admit numbers and
//! screener enums. Visualization queries need ISO date strings, symbol strings
//! and a three-operand `GTELT` range operator, so reusing them would require new
//! public universe markers and value types. A tiny private builder keeps the
//! visualization wire format out of the public API instead.

use chrono::NaiveDate;
use serde_json::{Value, json};

/// A visualization query node: `{ "operator": OP, "operands": [...] }`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct VizQuery {
    operator: &'static str,
    operands: Vec<VizOperand>,
}

#[derive(Debug, Clone, PartialEq)]
enum VizOperand {
    Value(Value),
    Query(VizQuery),
}

impl VizQuery {
    pub(super) fn and(queries: impl IntoIterator<Item = Self>) -> Self {
        Self::logical("AND", queries)
    }

    pub(super) fn or(queries: impl IntoIterator<Item = Self>) -> Self {
        Self::logical("OR", queries)
    }

    fn logical(operator: &'static str, queries: impl IntoIterator<Item = Self>) -> Self {
        Self {
            operator,
            operands: queries.into_iter().map(VizOperand::Query).collect(),
        }
    }

    pub(super) fn push(&mut self, query: Self) {
        self.operands.push(VizOperand::Query(query));
    }

    fn comparison(operator: &'static str, field: &'static str, values: Vec<Value>) -> Self {
        let mut operands = Vec::with_capacity(values.len() + 1);
        operands.push(VizOperand::Value(Value::String(field.to_string())));
        operands.extend(values.into_iter().map(VizOperand::Value));
        Self { operator, operands }
    }

    pub(super) fn eq_str(field: &'static str, value: &str) -> Self {
        Self::comparison("EQ", field, vec![Value::String(value.to_string())])
    }

    pub(super) fn gte_date(field: &'static str, date: NaiveDate) -> Self {
        Self::comparison("GTE", field, vec![date_value(date)])
    }

    pub(super) fn lte_date(field: &'static str, date: NaiveDate) -> Self {
        Self::comparison("LTE", field, vec![date_value(date)])
    }

    pub(super) fn gte_u64(field: &'static str, value: u64) -> Self {
        Self::comparison("GTE", field, vec![Value::from(value)])
    }

    /// Yahoo's `GTELT` range operator: `start <= field < end`.
    pub(super) fn gtelt_date(field: &'static str, start: NaiveDate, end: NaiveDate) -> Self {
        Self::comparison("GTELT", field, vec![date_value(start), date_value(end)])
    }

    pub(super) fn into_wire_value(self) -> Value {
        let operands: Vec<Value> = self
            .operands
            .into_iter()
            .map(|operand| match operand {
                VizOperand::Value(value) => value,
                VizOperand::Query(query) => query.into_wire_value(),
            })
            .collect();
        json!({ "operator": self.operator, "operands": operands })
    }
}

fn date_value(date: NaiveDate) -> Value {
    Value::String(date.format("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_query_serializes_like_python_calendar_query() {
        let start = NaiveDate::from_ymd_opt(2025, 11, 3).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 11, 10).unwrap();
        let mut query = VizQuery::and([
            VizQuery::eq_str("region", "us"),
            VizQuery::gte_date("startdatetime", start),
            VizQuery::lte_date("startdatetime", end),
        ]);
        query.push(VizQuery::or([VizQuery::gtelt_date(
            "filingdate",
            start,
            end,
        )]));
        assert_eq!(
            query.into_wire_value(),
            json!({
                "operator": "AND",
                "operands": [
                    {"operator": "EQ", "operands": ["region", "us"]},
                    {"operator": "GTE", "operands": ["startdatetime", "2025-11-03"]},
                    {"operator": "LTE", "operands": ["startdatetime", "2025-11-10"]},
                    {"operator": "OR", "operands": [
                        {"operator": "GTELT", "operands": ["filingdate", "2025-11-03", "2025-11-10"]}
                    ]}
                ]
            })
        );
    }
}
