//! Per-symbol past and future earnings dates (`entityIdType` `earnings`).

use chrono::{DateTime, Utc};
use paft::money::Currency;
use url::Url;

use super::api::{
    VizRequest, VizRow, gmt_offset_ms, opt_decimal, opt_money, opt_string, post_visualization,
    required_datetime, rows, validate_limit,
};
use super::model::{EarningsDate, EarningsEventType};
use super::project::{col, timing};
use super::query::VizQuery;
use super::wire;
use crate::core::client::normalize_symbol;
use crate::core::currency_resolver::{
    AnalystEstimateCurrencyEvidence, CurrencyPurpose, ResolvedCurrencyUnit,
    project_currency_resolution,
};
use crate::core::{CallOptions, ProjectionContext, ProjectionIssue, YfClient, YfError, YfResponse};

/// Default number of earnings dates (about the next 4 and last 8 quarters).
pub const DEFAULT_EARNINGS_DATES_LIMIT: u32 = 12;
const FEATURE: &str = "earnings_dates";
const ITEM: &str = "earnings_date";
const FIXTURE_ENDPOINT: &str = "earnings_dates_api_visualization";

const EARNINGS_DATES_FIELDS: &[&str] = &[
    col::TICKER,
    col::COMPANY,
    col::EVENT_NAME,
    col::START,
    col::START_TYPE,
    col::TZ_SHORT,
    col::GMT_OFFSET,
    col::EPS_ESTIMATE,
    col::EPS_ACTUAL,
    col::EPS_SURPRISE,
    col::EVENT_TYPE,
];

/// Builder for one symbol's past and future earnings dates from Yahoo's
/// visualization API (no HTML scraping).
///
/// Rows are returned newest first, at most `limit` of them (default 12, maximum 100).
/// EPS amounts use the symbol's analyst-estimate currency, resolved from Yahoo
/// metadata unless overridden with [`currency`](Self::currency).
#[derive(Debug, Clone)]
pub struct EarningsDatesBuilder {
    client: YfClient,
    symbol: String,
    base: Option<Url>,
    limit: u32,
    override_currency: Option<Currency>,
    options: CallOptions,
}

impl EarningsDatesBuilder {
    /// Creates a builder for `symbol` with the default limit of 12.
    pub fn new(client: &YfClient, symbol: impl Into<String>) -> Self {
        Self {
            client: client.clone(),
            symbol: symbol.into(),
            base: None,
            limit: DEFAULT_EARNINGS_DATES_LIMIT,
            override_currency: None,
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!(
        strict_doc =
            "Fails when Yahoo earnings-date data cannot be projected losslessly or is unavailable."
    );

    /// Sets the maximum number of rows (1..=100; validated when fetching). Default: 12.
    #[must_use]
    pub const fn limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    /// Overrides the currency used for EPS amounts instead of resolving it from Yahoo.
    #[must_use]
    pub fn currency(mut self, currency: Currency) -> Self {
        self.override_currency = Some(currency);
        self
    }

    /// Overrides the visualization endpoint URL (defaults to the client's
    /// `base_visualization`).
    #[must_use]
    pub fn visualization_base(mut self, base: Url) -> Self {
        self.base = Some(base);
        self
    }

    /// Fetches earnings dates, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`YfError::InvalidParams`] for an invalid symbol or a limit outside
    /// 1..=100 (before any request is sent); otherwise an error if the request fails,
    /// Yahoo returns an API error, or (strict mode) Yahoo returns no rows.
    pub async fn fetch(&self) -> Result<Vec<EarningsDate>, YfError> {
        Ok(self.fetch_with_diagnostics().await?.into_data())
    }

    /// Fetches earnings dates with projection diagnostics.
    ///
    /// An empty Yahoo result yields an empty list plus a
    /// [`YfWarning::ProviderFeatureUnavailable`](crate::YfWarning::ProviderFeatureUnavailable)
    /// warning (the data source may be stale); strict mode turns it into an error.
    ///
    /// # Errors
    ///
    /// As [`fetch`](Self::fetch); additionally fails in strict mode when a row cannot
    /// be projected losslessly.
    pub async fn fetch_with_diagnostics(&self) -> Result<YfResponse<Vec<EarningsDate>>, YfError> {
        let limit = validate_limit(self.limit)?;
        let symbol = normalize_symbol(&self.symbol)?;
        let mut ctx = ProjectionContext::new("earnings_dates", self.options.data_quality());

        let body = VizRequest {
            entity_id_type: "earnings",
            sort_field: col::START,
            include_fields: EARNINGS_DATES_FIELDS,
            size: limit,
            offset: None,
            query: VizQuery::eq_str(col::TICKER, &symbol),
        }
        .into_body();
        let base = self
            .base
            .clone()
            .unwrap_or_else(|| self.client.base_visualization().clone());
        let text = post_visualization(
            &self.client,
            &base,
            &self.options,
            &body,
            FIXTURE_ENDPOINT,
            &symbol,
        )
        .await?;
        let table = wire::parse_table(&text)?;

        if table.rows.is_empty() {
            ctx.provider_feature_unavailable(
                FEATURE,
                ProjectionIssue::ProviderUnavailable { feature: FEATURE },
            )?;
            return Ok(ctx.finish(Vec::new()));
        }

        let mut dated = Vec::with_capacity(table.rows.len());
        for row in rows(&mut ctx, &table, ITEM, col::START)? {
            let offset = gmt_offset_ms(&mut ctx, &row, col::GMT_OFFSET)?;
            if let Some(date) = required_datetime(&mut ctx, &row, ITEM, col::START, offset)? {
                dated.push((row, date));
            }
        }

        let has_eps = dated.iter().any(|(row, _)| {
            row.cell(col::EPS_ESTIMATE).is_some() || row.cell(col::EPS_ACTUAL).is_some()
        });
        let (unit, issue) = if has_eps {
            self.resolve_currency(&mut ctx, &symbol).await?
        } else {
            (None, None)
        };
        let currency = EpsCurrency {
            unit: unit.as_ref(),
            issue: issue.as_ref(),
        };

        let now = Utc::now();
        let mut out = Vec::with_capacity(dated.len());
        for (row, date) in dated {
            out.push(project_row(&mut ctx, &row, date, currency, now)?);
        }
        out.sort_by_key(|row| std::cmp::Reverse(row.date));
        out.truncate(limit as usize);
        Ok(ctx.finish(out))
    }

    async fn resolve_currency(
        &self,
        ctx: &mut ProjectionContext,
        symbol: &str,
    ) -> Result<(Option<ResolvedCurrencyUnit>, Option<ProjectionIssue>), YfError> {
        let resolution = self
            .client
            .resolve_analyst_estimate_currency(
                symbol,
                self.override_currency.clone(),
                AnalystEstimateCurrencyEvidence::Earnings(None),
                &self.options,
            )
            .await;
        let projected = project_currency_resolution(
            ctx,
            symbol,
            CurrencyPurpose::AnalystEstimate,
            None,
            resolution,
        )?;
        let issue = projected.issue().cloned();
        Ok((projected.into_unit(), issue))
    }
}

#[derive(Clone, Copy)]
struct EpsCurrency<'a> {
    unit: Option<&'a ResolvedCurrencyUnit>,
    issue: Option<&'a ProjectionIssue>,
}

fn project_row(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
    date: DateTime<Utc>,
    currency: EpsCurrency<'_>,
    now: DateTime<Utc>,
) -> Result<EarningsDate, YfError> {
    let mut eps_actual = opt_money(ctx, row, col::EPS_ACTUAL, currency.unit, currency.issue)?;
    let mut surprise_percent = opt_decimal(ctx, row, col::EPS_SURPRISE)?;
    if date > now {
        // Yahoo pads not-yet-reported events with zero placeholders.
        if eps_actual.as_ref().is_some_and(|m| m.amount().is_zero()) {
            eps_actual = None;
        }
        if surprise_percent.is_some_and(|s| s.is_zero()) {
            surprise_percent = None;
        }
    }

    Ok(EarningsDate {
        date,
        timezone_short_name: opt_string(ctx, row, col::TZ_SHORT)?,
        timing: timing(ctx, row)?,
        event_name: opt_string(ctx, row, col::EVENT_NAME)?,
        event_type: opt_string(ctx, row, col::EVENT_TYPE)?
            .as_deref()
            .map(EarningsEventType::parse),
        eps_estimate: opt_money(ctx, row, col::EPS_ESTIMATE, currency.unit, currency.issue)?,
        eps_actual,
        surprise_percent,
    })
}
