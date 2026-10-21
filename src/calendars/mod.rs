//! Market-wide event calendars and per-symbol earnings dates from Yahoo's
//! `v1/finance/visualization` POST API.
//!
//! Request and response shapes follow Python `yfinance` (`calendars.py` and
//! `base.py::_get_earnings_dates_using_screener`). Yahoo returns a columnar table
//! whose cells are matched to fields by column id.

mod api;
mod earnings_dates;
mod model;
mod project;
mod query;
mod wire;

pub(crate) use earnings_dates::DEFAULT_EARNINGS_DATES_LIMIT;
pub use earnings_dates::EarningsDatesBuilder;
pub use model::{
    EarningsCallTiming, EarningsDate, EarningsEvent, EarningsEventType, EconomicEvent, IpoEvent,
    StockSplitEvent,
};

use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use url::Url;

use api::{VizRequest, post_visualization, validate_limit};
use project::col;
use query::VizQuery;

use crate::core::{CallOptions, ProjectionContext, YfClient, YfError, YfResponse};
use crate::screener::{PredefinedScreener, ScreenerBuilder, ScreenerCount};

const DEFAULT_LIMIT: u32 = 12;
const DEFAULT_WINDOW_DAYS: u64 = 7;
const MOST_ACTIVE_COUNT: u32 = 200;
const FIXTURE_ENDPOINT: &str = "calendars_api_visualization";

mod sealed {
    pub trait Sealed {}
    impl Sealed for chrono::NaiveDate {}
    impl<Tz: chrono::TimeZone> Sealed for chrono::DateTime<Tz> {}
}

/// A calendar window bound: a [`NaiveDate`] or any [`DateTime`].
///
/// Date-times are reduced to their calendar date in their own timezone, because
/// Yahoo filters calendars by `YYYY-MM-DD` dates.
pub trait CalendarDate: sealed::Sealed {
    /// Returns the calendar date sent to Yahoo.
    fn calendar_date(self) -> NaiveDate;
}

impl CalendarDate for NaiveDate {
    fn calendar_date(self) -> NaiveDate {
        self
    }
}

impl<Tz: TimeZone> CalendarDate for DateTime<Tz> {
    fn calendar_date(self) -> NaiveDate {
        self.date_naive()
    }
}

/// Builder for Yahoo's market-wide earnings, IPO, stock-split and economic-event calendars.
///
/// The default window is today (UTC) through today + 7 days, the default page size
/// is 12 rows, and the page size may not exceed 100.
///
/// ```no_run
/// # use yfinance_rs::{YfClient, calendars::CalendarsBuilder};
/// # #[tokio::main]
/// # async fn main() -> Result<(), yfinance_rs::YfError> {
/// let client = YfClient::default();
/// let earnings = CalendarsBuilder::new(&client)
///     .limit(50)
///     .min_market_cap(10_000_000_000)
///     .earnings()
///     .await?;
/// for event in earnings {
///     println!("{} reports at {}", event.symbol, event.start);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CalendarsBuilder {
    client: YfClient,
    base: Option<Url>,
    screener_base: Option<Url>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    limit: u32,
    offset: Option<u32>,
    min_market_cap: Option<u64>,
    most_active_only: bool,
    options: CallOptions,
}

impl CalendarsBuilder {
    /// Creates a calendars builder with the default window and page size.
    #[must_use]
    pub fn new(client: &YfClient) -> Self {
        Self {
            client: client.clone(),
            base: None,
            screener_base: None,
            start: None,
            end: None,
            limit: DEFAULT_LIMIT,
            offset: None,
            min_market_cap: None,
            most_active_only: false,
            options: CallOptions::default(),
        }
    }

    crate::core::impl_call_option_setters!(
        strict_doc = "Fails when Yahoo calendar data cannot be projected losslessly."
    );

    /// Overrides the visualization endpoint URL (defaults to the client's
    /// `base_visualization`).
    #[must_use]
    pub fn visualization_base(mut self, base: Url) -> Self {
        self.base = Some(base);
        self
    }

    /// Overrides the predefined-screener URL used by [`most_active_only`](Self::most_active_only).
    #[must_use]
    pub fn most_active_screener_base(mut self, base: Url) -> Self {
        self.screener_base = Some(base);
        self
    }

    /// Sets the inclusive date window (`start` and `end` may be dates or date-times).
    #[must_use]
    pub fn between(mut self, start: impl CalendarDate, end: impl CalendarDate) -> Self {
        self.start = Some(start.calendar_date());
        self.end = Some(end.calendar_date());
        self
    }

    /// Sets the page size (1..=100; validated when fetching). Default: 12.
    #[must_use]
    pub const fn limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    /// Sets the row offset for paging. Passed to Yahoo as `offset`.
    #[must_use]
    pub const fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Earnings calendar only: keeps companies whose intraday market cap is at
    /// least `usd` US dollars.
    #[must_use]
    pub const fn min_market_cap(mut self, usd: u64) -> Self {
        self.min_market_cap = Some(usd);
        self
    }

    /// Earnings calendar only: restricts results to Yahoo's 200 most-active
    /// stocks (one extra predefined-screener request). Default: `false`.
    ///
    /// Yahoo rejects this filter together with a non-zero offset, so that
    /// combination fails with [`YfError::InvalidParams`].
    #[must_use]
    pub const fn most_active_only(mut self, enabled: bool) -> Self {
        self.most_active_only = enabled;
        self
    }

    fn base_url(&self) -> Url {
        self.base
            .clone()
            .unwrap_or_else(|| self.client.base_visualization().clone())
    }

    fn window(&self) -> Result<(NaiveDate, NaiveDate), YfError> {
        let start = self.start.unwrap_or_else(|| Utc::now().date_naive());
        let end = match self.end {
            Some(end) => end,
            None => start
                .checked_add_days(Days::new(DEFAULT_WINDOW_DAYS))
                .ok_or_else(|| YfError::InvalidParams("calendar window overflows".into()))?,
        };
        if start > end {
            return Err(YfError::InvalidParams(format!(
                "calendar start {start} is after end {end}"
            )));
        }
        Ok((start, end))
    }

    fn validated(&self) -> Result<(u32, NaiveDate, NaiveDate), YfError> {
        let limit = validate_limit(self.limit)?;
        let (start, end) = self.window()?;
        Ok((limit, start, end))
    }

    async fn post(
        &self,
        entity_id_type: &'static str,
        sort_field: &'static str,
        include_fields: &'static [&'static str],
        size: u32,
        query: VizQuery,
    ) -> Result<wire::VizTable, YfError> {
        let body = VizRequest {
            entity_id_type,
            sort_field,
            include_fields,
            size,
            offset: Some(self.offset.unwrap_or(0)),
            query,
        }
        .into_body();
        let text = post_visualization(
            &self.client,
            &self.base_url(),
            &self.options,
            &body,
            FIXTURE_ENDPOINT,
            entity_id_type,
        )
        .await?;
        wire::parse_table(&text)
    }

    /// Fetches the earnings calendar (`entityIdType` `sp_earnings`, US region).
    ///
    /// # Errors
    ///
    /// Returns [`YfError::InvalidParams`] for a limit outside 1..=100, an inverted
    /// window, or `most_active_only` with an offset; otherwise an error if the
    /// request fails or Yahoo returns an API error.
    pub async fn earnings(&self) -> Result<Vec<EarningsEvent>, YfError> {
        Ok(self.earnings_with_diagnostics().await?.into_data())
    }

    /// Fetches the earnings calendar with projection diagnostics.
    ///
    /// # Errors
    ///
    /// As [`earnings`](Self::earnings); additionally fails in strict mode when a
    /// row cannot be projected losslessly.
    pub async fn earnings_with_diagnostics(
        &self,
    ) -> Result<YfResponse<Vec<EarningsEvent>>, YfError> {
        let (limit, start, end) = self.validated()?;
        if self.most_active_only && self.offset.is_some_and(|offset| offset > 0) {
            return Err(YfError::InvalidParams(
                "most_active_only cannot be combined with a non-zero offset".into(),
            ));
        }
        let mut ctx = ProjectionContext::new("calendars", self.options.data_quality());

        let mut query = VizQuery::and([
            VizQuery::eq_str("region", "us"),
            VizQuery::or([
                VizQuery::eq_str(col::EVENT_TYPE, "EAD"),
                VizQuery::eq_str(col::EVENT_TYPE, "ERA"),
            ]),
            VizQuery::gte_date(col::START, start),
            VizQuery::lte_date(col::START, end),
        ]);
        if let Some(cap) = self.min_market_cap {
            query.push(VizQuery::gte_u64(col::MARKET_CAP, cap));
        }
        if self.most_active_only {
            let symbols = self.most_active_symbols(&mut ctx).await?;
            if symbols.is_empty() {
                return Ok(ctx.finish(Vec::new()));
            }
            query.push(VizQuery::or(
                symbols
                    .iter()
                    .map(|symbol| VizQuery::eq_str(col::TICKER, symbol)),
            ));
        }

        let table = self
            .post(
                "sp_earnings",
                col::MARKET_CAP,
                project::EARNINGS_FIELDS,
                limit,
                query,
            )
            .await?;
        let rows = project::earnings(&mut ctx, &table)?;
        Ok(ctx.finish(rows))
    }

    async fn most_active_symbols(
        &self,
        ctx: &mut ProjectionContext,
    ) -> Result<Vec<String>, YfError> {
        let mut screener =
            ScreenerBuilder::predefined(&self.client, PredefinedScreener::MostActives)
                .count(ScreenerCount::new(MOST_ACTIVE_COUNT)?)
                .cache_mode(self.options.cache_mode())
                .data_quality(self.options.data_quality())
                .retry_policy(self.options.retry_override().cloned());
        if let Some(base) = &self.screener_base {
            screener = screener.predefined_screener_base(base.clone());
        }
        let response = screener.fetch_with_diagnostics().await?;
        ctx.extend(response.diagnostics);
        let results = response.data.results;
        if results.is_empty() {
            ctx.unavailable_feature("most_actives")?;
        }
        let min_cap = self.min_market_cap.map(Decimal::from);
        Ok(results
            .into_iter()
            .filter(|result| {
                min_cap.is_none_or(|cap| {
                    result
                        .market_cap
                        .as_ref()
                        .is_some_and(|market_cap| market_cap.amount() >= cap)
                })
            })
            .filter_map(|result| result.symbol)
            .collect())
    }

    /// Fetches the IPO calendar (`entityIdType` `ipo_info`). A row matches when its
    /// listing, filing or amendment date falls in the window.
    ///
    /// # Errors
    ///
    /// Returns [`YfError::InvalidParams`] for a limit outside 1..=100 or an inverted
    /// window; otherwise an error if the request fails or Yahoo returns an API error.
    pub async fn ipos(&self) -> Result<Vec<IpoEvent>, YfError> {
        Ok(self.ipos_with_diagnostics().await?.into_data())
    }

    /// Fetches the IPO calendar with projection diagnostics.
    ///
    /// # Errors
    ///
    /// As [`ipos`](Self::ipos); additionally fails in strict mode when a row cannot
    /// be projected losslessly.
    pub async fn ipos_with_diagnostics(&self) -> Result<YfResponse<Vec<IpoEvent>>, YfError> {
        let (limit, start, end) = self.validated()?;
        let mut ctx = ProjectionContext::new("calendars", self.options.data_quality());
        let query = VizQuery::or([
            VizQuery::gtelt_date(col::START, start, end),
            VizQuery::gtelt_date(col::FILING_DATE, start, end),
            VizQuery::gtelt_date(col::AMENDED_DATE, start, end),
        ]);
        let table = self
            .post("ipo_info", col::START, project::IPO_FIELDS, limit, query)
            .await?;
        let rows = project::ipos(&mut ctx, &table)?;
        Ok(ctx.finish(rows))
    }

    /// Fetches the stock-splits calendar (`entityIdType` `splits`).
    ///
    /// # Errors
    ///
    /// Returns [`YfError::InvalidParams`] for a limit outside 1..=100 or an inverted
    /// window; otherwise an error if the request fails or Yahoo returns an API error.
    pub async fn splits(&self) -> Result<Vec<StockSplitEvent>, YfError> {
        Ok(self.splits_with_diagnostics().await?.into_data())
    }

    /// Fetches the stock-splits calendar with projection diagnostics.
    ///
    /// # Errors
    ///
    /// As [`splits`](Self::splits); additionally fails in strict mode when a row
    /// cannot be projected losslessly.
    pub async fn splits_with_diagnostics(
        &self,
    ) -> Result<YfResponse<Vec<StockSplitEvent>>, YfError> {
        let (limit, start, end) = self.validated()?;
        let mut ctx = ProjectionContext::new("calendars", self.options.data_quality());
        let table = self
            .post(
                "splits",
                col::START,
                project::SPLITS_FIELDS,
                limit,
                start_window(start, end),
            )
            .await?;
        let rows = project::splits(&mut ctx, &table)?;
        Ok(ctx.finish(rows))
    }

    /// Fetches the economic-events calendar (`entityIdType` `economic_event`).
    ///
    /// # Errors
    ///
    /// Returns [`YfError::InvalidParams`] for a limit outside 1..=100 or an inverted
    /// window; otherwise an error if the request fails or Yahoo returns an API error.
    pub async fn economic_events(&self) -> Result<Vec<EconomicEvent>, YfError> {
        Ok(self.economic_events_with_diagnostics().await?.into_data())
    }

    /// Fetches the economic-events calendar with projection diagnostics.
    ///
    /// # Errors
    ///
    /// As [`economic_events`](Self::economic_events); additionally fails in strict
    /// mode when a row cannot be projected losslessly.
    pub async fn economic_events_with_diagnostics(
        &self,
    ) -> Result<YfResponse<Vec<EconomicEvent>>, YfError> {
        let (limit, start, end) = self.validated()?;
        let mut ctx = ProjectionContext::new("calendars", self.options.data_quality());
        let table = self
            .post(
                "economic_event",
                col::START,
                project::ECONOMIC_FIELDS,
                limit,
                start_window(start, end),
            )
            .await?;
        let rows = project::economic_events(&mut ctx, &table)?;
        Ok(ctx.finish(rows))
    }
}

fn start_window(start: NaiveDate, end: NaiveDate) -> VizQuery {
    VizQuery::and([
        VizQuery::gte_date(col::START, start),
        VizQuery::lte_date(col::START, end),
    ])
}
