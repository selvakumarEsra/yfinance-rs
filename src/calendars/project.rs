//! Projection of visualization tables into the public calendar models.

use paft::money::Currency;

use super::api::{
    RowValue, VizRow, opt_bool, opt_decimal, opt_money, opt_string, opt_u64, optional_date,
    optional_datetime, required_datetime, required_string, rows,
};
use super::model::{EarningsCallTiming, EarningsEvent, EconomicEvent, IpoEvent, StockSplitEvent};
use super::wire::VizTable;
use crate::core::currency_resolver::ResolvedCurrencyUnit;
use crate::core::{ProjectionContext, ProjectionIssue, YfError};

/// Column ids (Yahoo `includeFields`) shared by several calendars.
pub(super) mod col {
    pub const TICKER: &str = "ticker";
    pub const COMPANY: &str = "companyshortname";
    pub const MARKET_CAP: &str = "intradaymarketcap";
    pub const EVENT_NAME: &str = "eventname";
    pub const START: &str = "startdatetime";
    pub const START_TYPE: &str = "startdatetimetype";
    pub const EPS_ESTIMATE: &str = "epsestimate";
    pub const EPS_ACTUAL: &str = "epsactual";
    pub const EPS_SURPRISE: &str = "epssurprisepct";
    pub const EVENT_TYPE: &str = "eventtype";
    pub const TZ_SHORT: &str = "timeZoneShortName";
    pub const GMT_OFFSET: &str = "gmtOffsetMilliSeconds";
    pub const EXCHANGE: &str = "exchange_short_name";
    pub const FILING_DATE: &str = "filingdate";
    pub const AMENDED_DATE: &str = "amendeddate";
    pub const PRICE_FROM: &str = "pricefrom";
    pub const PRICE_TO: &str = "priceto";
    pub const OFFER_PRICE: &str = "offerprice";
    pub const CURRENCY: &str = "currencyname";
    pub const SHARES: &str = "shares";
    pub const DEAL_TYPE: &str = "dealtype";
    pub const OPTIONABLE: &str = "optionable";
    pub const OLD_SHARE_WORTH: &str = "old_share_worth";
    pub const SHARE_WORTH: &str = "share_worth";
    pub const ECON_RELEASE: &str = "econ_release";
    pub const COUNTRY: &str = "country_code";
    pub const PERIOD: &str = "period";
    pub const ACTUAL: &str = "after_release_actual";
    pub const CONSENSUS: &str = "consensus_estimate";
    pub const PRIOR: &str = "prior_release_actual";
    pub const ORIGINAL: &str = "originally_reported_actual";
}

pub(super) const EARNINGS_FIELDS: &[&str] = &[
    col::TICKER,
    col::COMPANY,
    col::MARKET_CAP,
    col::EVENT_NAME,
    col::START,
    col::START_TYPE,
    col::EPS_ESTIMATE,
    col::EPS_ACTUAL,
    col::EPS_SURPRISE,
];

pub(super) const IPO_FIELDS: &[&str] = &[
    col::TICKER,
    col::COMPANY,
    col::EXCHANGE,
    col::FILING_DATE,
    col::START,
    col::AMENDED_DATE,
    col::PRICE_FROM,
    col::PRICE_TO,
    col::OFFER_PRICE,
    col::CURRENCY,
    col::SHARES,
    col::DEAL_TYPE,
];

pub(super) const SPLITS_FIELDS: &[&str] = &[
    col::TICKER,
    col::COMPANY,
    col::START,
    col::OPTIONABLE,
    col::OLD_SHARE_WORTH,
    col::SHARE_WORTH,
];

pub(super) const ECONOMIC_FIELDS: &[&str] = &[
    col::ECON_RELEASE,
    col::COUNTRY,
    col::START,
    col::PERIOD,
    col::ACTUAL,
    col::CONSENSUS,
    col::PRIOR,
    col::ORIGINAL,
];

pub(super) fn timing(
    ctx: &mut ProjectionContext,
    row: &VizRow<'_>,
) -> Result<Option<EarningsCallTiming>, YfError> {
    Ok(opt_string(ctx, row, col::START_TYPE)?
        .as_deref()
        .map(EarningsCallTiming::parse))
}

pub(super) fn earnings(
    ctx: &mut ProjectionContext,
    table: &VizTable,
) -> Result<Vec<EarningsEvent>, YfError> {
    const ITEM: &str = "earnings_event";
    // The earnings calendar is filtered to region=us and carries no currency column.
    let usd = ResolvedCurrencyUnit::from_currency(Currency::Iso(paft::money::IsoCurrency::USD));
    let mut out = Vec::new();
    for row in rows(ctx, table, ITEM, col::TICKER)? {
        let Some(symbol) = required_string(ctx, &row, ITEM, col::TICKER)? else {
            continue;
        };
        let Some(start) = required_datetime(ctx, &row, ITEM, col::START, None)? else {
            continue;
        };
        out.push(EarningsEvent {
            symbol,
            company_name: opt_string(ctx, &row, col::COMPANY)?,
            market_cap: opt_money(ctx, &row, col::MARKET_CAP, Some(&usd), None)?,
            event_name: opt_string(ctx, &row, col::EVENT_NAME)?,
            start,
            timing: timing(ctx, &row)?,
            eps_estimate: opt_money(ctx, &row, col::EPS_ESTIMATE, Some(&usd), None)?,
            eps_actual: opt_money(ctx, &row, col::EPS_ACTUAL, Some(&usd), None)?,
            surprise_percent: opt_decimal(ctx, &row, col::EPS_SURPRISE)?,
        });
    }
    Ok(out)
}

pub(super) fn ipos(
    ctx: &mut ProjectionContext,
    table: &VizTable,
) -> Result<Vec<IpoEvent>, YfError> {
    const ITEM: &str = "ipo_event";
    let mut out = Vec::new();
    for row in rows(ctx, table, ITEM, col::TICKER)? {
        let Some(symbol) = required_string(ctx, &row, ITEM, col::TICKER)? else {
            continue;
        };
        let RowValue::Keep(filing_date) = optional_date(ctx, &row, ITEM, col::FILING_DATE)? else {
            continue;
        };
        let RowValue::Keep(date) = optional_datetime(ctx, &row, ITEM, col::START)? else {
            continue;
        };
        let RowValue::Keep(amended_date) = optional_date(ctx, &row, ITEM, col::AMENDED_DATE)?
        else {
            continue;
        };

        let currency_code = opt_string(ctx, &row, col::CURRENCY)?;
        let unit = currency_code
            .as_deref()
            .and_then(ResolvedCurrencyUnit::from_code);
        let currency_issue = match (&currency_code, &unit) {
            (Some(code), None) => Some(ProjectionIssue::InvalidCurrency { code: code.clone() }),
            _ => None,
        };

        out.push(IpoEvent {
            symbol,
            company_name: opt_string(ctx, &row, col::COMPANY)?,
            exchange: opt_string(ctx, &row, col::EXCHANGE)?,
            filing_date,
            date,
            amended_date,
            price_from: opt_money(
                ctx,
                &row,
                col::PRICE_FROM,
                unit.as_ref(),
                currency_issue.as_ref(),
            )?,
            price_to: opt_money(
                ctx,
                &row,
                col::PRICE_TO,
                unit.as_ref(),
                currency_issue.as_ref(),
            )?,
            offer_price: opt_money(
                ctx,
                &row,
                col::OFFER_PRICE,
                unit.as_ref(),
                currency_issue.as_ref(),
            )?,
            shares: opt_u64(ctx, &row, col::SHARES)?,
            deal_type: opt_string(ctx, &row, col::DEAL_TYPE)?,
        });
    }
    Ok(out)
}

pub(super) fn splits(
    ctx: &mut ProjectionContext,
    table: &VizTable,
) -> Result<Vec<StockSplitEvent>, YfError> {
    const ITEM: &str = "split_event";
    let mut out = Vec::new();
    for row in rows(ctx, table, ITEM, col::TICKER)? {
        let Some(symbol) = required_string(ctx, &row, ITEM, col::TICKER)? else {
            continue;
        };
        let Some(payable_on) = required_datetime(ctx, &row, ITEM, col::START, None)? else {
            continue;
        };
        out.push(StockSplitEvent {
            symbol,
            company_name: opt_string(ctx, &row, col::COMPANY)?,
            payable_on,
            optionable: opt_bool(ctx, &row, col::OPTIONABLE)?,
            old_shares: opt_decimal(ctx, &row, col::OLD_SHARE_WORTH)?,
            new_shares: opt_decimal(ctx, &row, col::SHARE_WORTH)?,
        });
    }
    Ok(out)
}

pub(super) fn economic_events(
    ctx: &mut ProjectionContext,
    table: &VizTable,
) -> Result<Vec<EconomicEvent>, YfError> {
    const ITEM: &str = "economic_event";
    let mut out = Vec::new();
    for row in rows(ctx, table, ITEM, col::ECON_RELEASE)? {
        let Some(event) = required_string(ctx, &row, ITEM, col::ECON_RELEASE)? else {
            continue;
        };
        let Some(time) = required_datetime(ctx, &row, ITEM, col::START, None)? else {
            continue;
        };
        out.push(EconomicEvent {
            event,
            country_code: opt_string(ctx, &row, col::COUNTRY)?,
            time,
            period: opt_string(ctx, &row, col::PERIOD)?,
            actual: opt_decimal(ctx, &row, col::ACTUAL)?,
            expected: opt_decimal(ctx, &row, col::CONSENSUS)?,
            prior: opt_decimal(ctx, &row, col::PRIOR)?,
            revised_from: opt_decimal(ctx, &row, col::ORIGINAL)?,
        });
    }
    Ok(out)
}
