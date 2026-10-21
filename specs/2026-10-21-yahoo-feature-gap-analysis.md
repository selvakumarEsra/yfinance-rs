---
id: GAP-DOC
title: yfinance-rs feature-gap analysis vs Yahoo Finance endpoints and Python yfinance
owner: unassigned
priority: medium
version: 1
status: proposed
created: 2026-10-21
---

<!-- id: GAP-DOC -->
# yfinance-rs feature-gap analysis vs Yahoo Finance and Python yfinance

This document compares yfinance-rs v0.9.1 (commit `bad87d5`) with two references:
the unofficial Yahoo Finance HTTP/WebSocket endpoints, and Python `yfinance` 1.7.0
(released 2026-08-26). Each row in the coverage matrix comes from the crate's
source (`src/lib.rs` re-exports, `src/ticker/mod.rs`, `src/core/client/constants.rs`,
and module constants under `src/`). Rows marked "unverified" could not be confirmed
from the source alone.

<!-- id: GAP-DOC.CONTEXT -->
## Context and method

<!-- id: GAP-DOC.CONTEXT.ENDPOINTS -->
### Yahoo endpoints the crate calls today

- `query1 /v8/finance/chart/{sym}`: history, actions, metadata.
- `query1 /v7/finance/quote`: batch quotes and `fast_info`.
- `query1 /v10/finance/quoteSummary/{sym}`: info, profile, analysis, holders, ESG, earnings, calendar.
- `query1 /v7/finance/options/{sym}`: expirations and option chains.
- `query2 /ws/fundamentals-timeseries/v1/finance/timeseries/{sym}`: statements and shares.
- `query2 /v1/finance/search`: search, quotes only.
- `query1 /v1/finance/screener` and `/v1/finance/screener/predefined/saved`: screeners.
- `finance.yahoo.com /xhr/ncp`: news.
- `wss://streamer.finance.yahoo.com/?version=2`: protobuf streaming, with v7 polling as a fallback.
- `fc.yahoo.com/consent` and `query1 /v1/test/getcrumb`: cookie and crumb handling.
- `markets.businessinsider.com/ajax/SearchController_Suggest`: ISIN lookup (the same source Python uses).

<!-- id: GAP-DOC.CONTEXT.MODULES -->
### quoteSummary modules: used vs available

The table compares Python's `quote_summary_valid_modules` with grep results over `src/`.

| Module | Used by yfinance-rs | Where |
|---|---|---|
| summaryDetail | yes | `ticker/info.rs`, `core/quotes.rs` (key statistics) |
| defaultKeyStatistics | yes | `ticker/info.rs`, `core/quotes.rs` |
| assetProfile | yes | `profile/api.rs`, `ticker/info.rs` |
| fundProfile | yes | `profile/api.rs`, `ticker/info.rs` |
| quoteType | yes | `profile/api.rs`, `ticker/info.rs` |
| financialData | yes | `analysis/api.rs`, `ticker/info.rs` |
| calendarEvents | yes | `fundamentals/api.rs` (calendar) |
| earnings | yes | `fundamentals/api.rs` (earnings) |
| earningsTrend | yes | `analysis/api.rs` |
| recommendationTrend | yes | `analysis/api.rs` |
| upgradeDowngradeHistory | yes | `analysis/api.rs` |
| esgScores | yes | `esg/api.rs` |
| institutionOwnership, fundOwnership, majorHoldersBreakdown, insiderTransactions, insiderHolders, netSharePurchaseActivity | yes | `holders/api.rs` |
| incomeStatementHistory(+Quarterly) | wire struct only, `#[allow(dead_code)]` | statements come from the timeseries API instead |
| balanceSheetHistory(+Quarterly), cashFlowStatementHistory(+Quarterly) | no (timeseries is used instead) | n/a |
| summaryProfile | no | n/a |
| price | no (v7 quote is used instead) | n/a |
| secFilings | **no** | n/a |
| earningsHistory | **no** | n/a |
| topHoldings | **no** | n/a |
| majorDirectHolders | no (Python requests it but does not parse it) | n/a |
| industryTrend, sectorTrend, indexTrend | **no** | n/a |
| futuresChain | no | n/a |
| fundPerformance | no (Python skips it too) | n/a |

<!-- id: GAP-DOC.MATRIX -->
## Coverage matrix

Legend: ✅ full, 🟡 partial, ❌ missing.

| Capability | Yahoo endpoint/module | Python yfinance | yfinance-rs status | Notes |
|---|---|---|---|---|
| OHLCV history | v8 chart | `Ticker.history` | ✅ full | `HistoryBuilder` / `Ticker::history`; intraday window limits handled in `history/builder/fetch.rs` |
| Period strings / custom start-end | v8 chart `range`, `period1/2` | `period=`, `start/end` | ✅ full | `Range` enum (paft) plus `between()` |
| Pre/post-market bars | v8 chart `includePrePost` | `prepost=` | ✅ full | `HistoryBuilder::prepost`, `DownloadBuilder::prepost` |
| auto_adjust / back_adjust / rounding / actions | v8 chart `events=div,split,capitalGains` | same params | ✅ full | `auto_adjust`, `back_adjust`, `unadjusted`, `rounding`, `actions` |
| Price repair (100x, zero bars, bad dividends, splits, capital-gain double count) | client-side | `repair=True` | ❌ missing | The crate drops malformed OHLC rows (`DataQuality`); it does not reconstruct them |
| Keep NaN rows | client-side | `keepna=True` | ❌ missing | No option to keep empty rows |
| History metadata | v8 chart `meta` | `history_metadata` | ✅ full | `get_history_metadata` |
| Dividends / splits / capital gains | v8 chart events | `dividends`, `splits`, `capital_gains`, `actions` | ✅ full | Typed `Action` stream |
| Multi-symbol download | v8 chart × N | `yf.download` | ✅ full | `DownloadBuilder` with concurrency control; no `group_by` (Polars handles reshaping) |
| Multi-ticker object | n/a | `yf.Tickers` | ❌ missing | Users loop over `Ticker`s; no shared batch façade |
| Batch quotes | v7 quote | (via `Tickers`/`download`) | ✅ full | `QuotesBuilder`, `quotes()` |
| info | quoteSummary (5–8 modules) + v7 quote + timeseries `trailingPegRatio` | `info` | 🟡 partial | 8 modules; the `trailingPegRatio` timeseries call is not made (grep shows no use) |
| fast_info | v7 quote / chart | `fast_info` | ✅ full | `FastInfo` |
| Key statistics | summaryDetail + defaultKeyStatistics | (inside `info`) | ✅ full | `key_statistics()` |
| Valuation measures table | timeseries `quarterly/annual/trailing{PeRatio,MarketCap,PegRatio,...}` | `valuation` (1.3.0) | ❌ missing | Timeseries plumbing already exists |
| Annual/quarterly statements | timeseries | `income_stmt`, `balance_sheet`, `cashflow` (+quarterly) | ✅ full | 6 methods on `Ticker` |
| TTM statements | timeseries `trailing*` | `ttm_income_stmt`, `ttm_cashflow` | ❌ missing | No `trailing` prefix support (`fundamentals/api.rs` handles only annual and quarterly) |
| Shares outstanding (full) | timeseries `shares_out` | `get_shares_full` | ✅ full | `shares_between`, `quarterly_shares_between` |
| Earnings (yearly/quarterly charts) | quoteSummary `earnings` | `earnings`, `quarterly_earnings` | ✅ full | `earnings()` |
| Earnings history (EPS actual vs estimate, surprise) | quoteSummary `earningsHistory` | `earnings_history` | 🟡 partial | Only `earnings.earningsChart` quarters; `earningsHistory` is not requested |
| Earnings dates (past + future, with surprise %) | `finance.yahoo.com/calendar/earnings` HTML; `v1/finance/visualization` | `earnings_dates` | 🟡 partial | Only upcoming dates via `calendarEvents` in `calendar()` |
| Corporate calendar | quoteSummary `calendarEvents` | `calendar` | ✅ full | `calendar()` |
| earnings_estimate / revenue_estimate / eps_trend / eps_revisions | quoteSummary `earningsTrend` | four properties | ✅ full | All in `earnings_trend()` (`EarningsTrendRow` with `EpsTrend`, `EpsRevisions`); no per-table convenience methods |
| Growth estimates (stock vs industry/sector/index) | `earningsTrend` + `industryTrend/sectorTrend/indexTrend` | `growth_estimates` | 🟡 partial | Stock growth from earningsTrend only; peer/index columns missing |
| Analyst price targets | financialData | `analyst_price_targets` | ✅ full | `analyst_price_target()` |
| Recommendations / summary / upgrades-downgrades | recommendationTrend, upgradeDowngradeHistory | same | ✅ full | |
| Major / institutional / mutual-fund holders | holder modules | same | ✅ full | |
| Insider transactions / roster | insiderTransactions, insiderHolders | same | ✅ full | |
| Insider purchases summary | netSharePurchaseActivity | `insider_purchases` | ✅ full | `net_share_purchase_activity()` |
| Major direct holders | majorDirectHolders | requested, not parsed | ❌ missing | Low value; Python skips it too |
| ESG / sustainability | esgScores | `sustainability` | ✅ full | |
| News | `/xhr/ncp` | `news`, `get_news(count, tab)` | ✅ full | `NewsBuilder` with `count`/`tab` |
| Options expirations / chain | v7 options | `options`, `option_chain(date)` | ✅ full | |
| ISIN | Business Insider suggest | `isin` | ✅ full | Python caches ISIN (0.2.62); unverified whether the crate's `CacheMode` covers it |
| SEC filings | quoteSummary `secFilings` | `sec_filings` | ❌ missing | Module never requested |
| Fund/ETF data (holdings, sector weights, asset classes, bond ratings, operations) | quoteSummary `topHoldings`, `fundProfile`, `summaryProfile` | `funds_data` (`FundsData`) | 🟡 partial | `profile()` reads `fundProfile` (family/category); `topHoldings` is never requested |
| Search (quotes) | v1 search | `Search.quotes` | ✅ full | `SearchBuilder` (`quotes_count`, `lang`, `region`) |
| Search (news, lists, research, nav, fuzzy) | v1 search `newsCount`, `listsCount`, `enableFuzzyQuery`, ... | `Search.news/lists/research/nav` | 🟡 partial | Response validator checks only `quotes`; other blocks are dropped |
| Lookup by type (equity/etf/future/currency/crypto/index/mutualfund) | v1 finance/lookup | `Lookup` | ❌ missing | |
| Custom screener (equity/fund/ETF queries) | v1 finance/screener (POST) | `screen`, `EquityQuery`, `FundQuery`, `ETFQuery` | ✅ full | `ScreenerBuilder::{equity,fund,etf}`, field catalogs |
| Predefined screeners | v1 screener/predefined/saved | `PREDEFINED_SCREENER_QUERIES` (19) | ✅ full | `PredefinedScreener` has the same 19 variants; Python sends a POST when `offset` is set, and the crate's handling of that case is unverified |
| Sector / Industry | v1 finance/sectors/{key}, v1 finance/industries/{key} | `Sector`, `Industry` | ❌ missing | |
| Market status / summary | v6 finance/markettime, v6 finance/quote/marketSummary | `Market(...).status/.summary` | ❌ missing | |
| Earnings / IPO / splits / economic-events calendars | v1 finance/visualization (POST, `entityIdType`) | `Calendars` (1.0) | ❌ missing | |
| Trending tickers | v1 finance/trending/{region} | not in Python | ❌ missing | Yahoo-only; liveness unverified |
| Similar / recommended symbols | v6 finance/recommendationsbysymbol/{sym} | not in Python | ❌ missing | Yahoo-only; liveness unverified |
| Insights (technical events, valuation, research) | ws/insights/v2/finance/insights | not in Python | ❌ missing | Yahoo-only; liveness unverified |
| Futures chain | quoteSummary `futuresChain` | listed module, no public API | ❌ missing | Low value |
| Live WebSocket streaming | streamer.finance.yahoo.com (protobuf) | `WebSocket`, `AsyncWebSocket`, `Ticker.live` | ✅ full | `StreamBuilder`, with an HTTP-polling fallback that Python lacks |
| Login / authenticated session | Yahoo login | `Auth` (1.4.0) | ❌ missing | Probably out of scope (ToS, credential handling) |
| Per-request lang/region scoping | `lang`/`region` query params | Ticker/Sector/Industry scoping (1.4.0) | 🟡 partial | Search only; other builders rely on base-URL overrides |
| Rate-limit handling | HTTP 429 | `YfRateLimitError`, optional retries (1.0) | 🟡 partial | `YfError::RateLimited`, retries on 408/429/5xx with backoff; `Retry-After` is not honored (no match in `src/`); no client-side throttle |
| Response caching | n/a | tz/cookie/ISIN caches | ✅ full | `CacheMode`, `cache_ttl_for`, `clear_cache` |
| Proxy / custom HTTP client | n/a | `session=`, proxy config | ✅ full | `try_proxy`, `try_https_proxy`, `custom_client` |
| Synchronous / blocking API | n/a | Python is sync by default | ❌ missing | Async (tokio) only |
| DataFrame output | n/a | pandas | ✅ full | Polars via the `dataframe` feature |

**Totals (55 rows):** ✅ 30 full · 🟡 8 partial · ❌ 17 missing.

<!-- id: GAP-DOC.PROPOSALS -->
## Improvement proposals

Proposals are ranked by value divided by effort, highest first. All API sketches
follow existing crate idioms:

- `Ticker` methods, each paired with a `*_with_diagnostics` variant that returns `YfResponse<T>`.
- `*Builder` types that take `&YfClient` and end in `fetch()` / `fetch_with_diagnostics()`.
- Base-URL overrides on `YfClientBuilder` for testing with `httpmock`.
- Public model types in `paft` where a cross-provider shape makes sense. Otherwise, crate-local models.

<!-- id: REQ-GAP-001 -->
## REQ-GAP-001: SEC filings (`Ticker::sec_filings`)

**Motivation:** Python exposes `sec_filings`. The data comes from a single quoteSummary
module, and the crate already has a generic fetcher for that endpoint.

**API sketch:**
```rust
pub async fn sec_filings(&self) -> Result<Vec<SecFiling>, YfError>;
pub async fn sec_filings_with_diagnostics(&self) -> Result<YfResponse<Vec<SecFiling>>, YfError>;
// SecFiling { date: NaiveDate, epoch_date: DateTime<Utc>, kind: String /* 10-K, 8-K */, title: String,
//             edgar_url: Option<Url>, exhibits: Vec<(String, Url)> }
```

**Endpoint:** `v10/finance/quoteSummary/{sym}?modules=secFilings`.
**Effort:** S. **Priority:** P1.
**Risks:** Yahoo returns this module only for US filers. Non-US symbols should map to `unavailable_feature` rather than an error.

<!-- id: REQ-GAP-001.A1 -->
- [ ] `Ticker::sec_filings()` requests only the `secFilings` module through `core::quotesummary::fetch_body`.
<!-- id: REQ-GAP-001.A2 -->
- [ ] Exhibits and the EDGAR URL are parsed; a missing module yields an empty Vec plus a diagnostics warning, not an `Err`.
<!-- id: REQ-GAP-001.A3 -->
- [ ] An offline fixture test covers a US filer and a symbol with no filings.

<!-- id: REQ-GAP-002 -->
## REQ-GAP-002: Valuation measures and TTM statements (timeseries `trailing`/`monthly`)

**Motivation:** Python 1.3.0 added `valuation`, and 0.2.55 added the `ttm_*` statements.
Both use the fundamentals-timeseries endpoint the crate already calls. The work is to
add `trailing` and `monthly` prefixes next to the existing `annual` and `quarterly`
prefixes, plus a valuation key set. The same work enables `trailingPegRatio` for `info`.

**API sketch:**
```rust
pub async fn ttm_income_stmt(&self, override_currency: Option<Currency>) -> Result<Vec<IncomeStatementRow>, YfError>;
pub async fn ttm_cashflow(&self, override_currency: Option<Currency>) -> Result<Vec<CashflowRow>, YfError>;
pub async fn valuation_measures(&self, freq: ValuationFrequency /* Quarterly|Annual|Monthly */) -> Result<ValuationMeasures, YfError>;
// ValuationMeasures { current: ValuationRow, history: Vec<ValuationRow> }
// ValuationRow { as_of, market_cap, enterprise_value, trailing_pe, forward_pe, peg, ps, pb, ev_revenue, ev_ebitda }
```

**Endpoint:** `ws/fundamentals-timeseries/v1/finance/timeseries/{sym}?type=trailingPeRatio,quarterlyMarketCap,...`.
**Effort:** S–M. **Priority:** P1.
**Risks:** Long `type=` lists can time out. Python 1.5.1 added a chunked-request fallback for this. Yahoo leaves some TTM balance-sheet keys empty.

<!-- id: REQ-GAP-002.A1 -->
- [ ] The timeseries fetcher accepts a `trailing` prefix, and `ttm_income_stmt`/`ttm_cashflow` return a single TTM row.
<!-- id: REQ-GAP-002.A2 -->
- [ ] `valuation_measures` returns a current row from `trailing*` keys plus history for the chosen frequency.
<!-- id: REQ-GAP-002.A3 -->
- [ ] Key lists above a configurable length are split into chunked requests and the results merged.

<!-- id: REQ-GAP-003 -->
## REQ-GAP-003: Fund/ETF holdings (`Ticker::funds_data`)

**Motivation:** ETF and mutual-fund users need top holdings, sector weightings,
asset-class mix, equity/bond characteristics and bond ratings (Python's `FundsData`).
The crate's `profile()` reads only `fundProfile`.

**API sketch:**
```rust
pub async fn funds_data(&self) -> Result<FundsData, YfError>;
// FundsData { description, overview: FundOverview, operations: FundOperations /* expense ratio, turnover, net assets vs category */,
//             asset_classes: AssetClasses, top_holdings: Vec<FundHolding { symbol, name, weight }>,
//             equity_holdings: FundEquityStats, bond_holdings: FundBondStats,
//             bond_ratings: BTreeMap<String, Decimal>, sector_weightings: BTreeMap<EquitySector, Decimal> }
```

**Endpoint:** `quoteSummary?modules=quoteType,summaryProfile,topHoldings,fundProfile`.
**Effort:** M. **Priority:** P1.
**Risks:** Equity symbols return no `topHoldings` and should get a typed "not a fund" error. Sector keys are snake_case strings (`realestate`, `consumer_cyclical`) and need mapping to `EquitySector`. Percentages arrive as raw fractions, so the unit (fraction or `PercentPoints`) must be documented.

<!-- id: REQ-GAP-003.A1 -->
- [ ] An ETF fixture such as SPY populates top holdings, sector weightings and asset classes.
<!-- id: REQ-GAP-003.A2 -->
- [ ] An equity symbol returns `YfError::InvalidParams`/`MissingData` with a clear message, or an `unavailable_feature` diagnostic.
<!-- id: REQ-GAP-003.A3 -->
- [ ] Every weight uses one documented unit, and sector keys map to `EquitySector`, with unknown keys preserved.

<!-- id: REQ-GAP-004 -->
## REQ-GAP-004: Earnings history and full earnings dates

**Motivation:** `earnings_history` (EPS actual, estimate, surprise for about 4 quarters)
needs only one module. `earnings_dates` adds past and future report dates with surprise
percentages and is used heavily in post-earnings (PEAD) workflows. The crate currently
gets only upcoming dates from `calendarEvents`.

**API sketch:**
```rust
pub async fn earnings_history(&self) -> Result<Vec<EarningsHistoryRow>, YfError>; // quarter, eps_actual, eps_estimate, surprise_pct, currency
pub fn earnings_dates_builder(&self) -> EarningsDatesBuilder; // .limit(n).offset(n).fetch()
// EarningsDate { at: DateTime<Utc>, eps_estimate, eps_reported, surprise_pct, event_kind }
```

**Endpoint:** `quoteSummary?modules=earningsHistory`. For dates, use `v1/finance/visualization` with `entityIdType=earnings`, with HTML `finance.yahoo.com/calendar/earnings?symbol=` as a fallback.
**Effort:** S (history) + M (dates). **Priority:** P1.
**Risks:** Python notes that Yahoo stopped updating the visualization data around summer 2025 and switched back to HTML scraping. HTML parsing is fragile and adds a dependency (e.g. `scraper`). Put it behind a feature flag or ship the history part first.

<!-- id: REQ-GAP-004.A1 -->
- [ ] `earnings_history()` parses `earningsHistory.history[]`, currency-aware like `earnings()`.
<!-- id: REQ-GAP-004.A2 -->
- [ ] `EarningsDatesBuilder` returns past and future rows in descending order and honours `limit`.
<!-- id: REQ-GAP-004.A3 -->
- [ ] A data-source change (empty visualization result) surfaces as a diagnostics warning and triggers the fallback path.

<!-- id: REQ-GAP-005 -->
## REQ-GAP-005: Lookup and full search results

**Motivation:** `search()` keeps only `quotes`, but Yahoo also returns news, lists,
research and nav blocks, and supports fuzzy queries. Python's `Lookup` filters symbol
discovery by asset type (future, currency, crypto, index), which search does not do well.

**API sketch:**
```rust
SearchBuilder::new(&client, "nvda").news_count(5).lists_count(3).fuzzy(true).include_research(true).fetch()
// SearchResponse gains: news: Vec<NewsArticle>, lists: Vec<SearchList>, research: Vec<ResearchItem>, nav: Vec<NavLink>
LookupBuilder::new(&client, "gold").kind(LookupKind::Future).count(25).fetch() -> Vec<LookupResult>
```

**Endpoint:** `query2 v1/finance/search` (`newsCount`, `listsCount`, `enableFuzzyQuery`, `enableResearchReports`, `enableNavLinks`); `query1 v1/finance/lookup` (`type`, `start`, `count`, `fetchPricingData`).
**Effort:** S. **Priority:** P2.
**Risks:** `SearchResponse` is a paft type, so the extra fields need a paft change or a crate-local wrapper (semver impact).

<!-- id: REQ-GAP-005.A1 -->
- [ ] `SearchBuilder` exposes news/lists counts and fuzzy matching, and returns the non-quote blocks.
<!-- id: REQ-GAP-005.A2 -->
- [ ] `LookupBuilder` supports all 8 Python lookup types and pagination via `offset`.
<!-- id: REQ-GAP-005.A3 -->
- [ ] Existing `search()` callers compile unchanged.

<!-- id: REQ-GAP-006 -->
## REQ-GAP-006: Market status and summary

**Motivation:** Market open/closed state and index snapshots per region (US, GB, ASIA,
EUROPE, RATES, COMMODITIES, CURRENCIES, CRYPTOCURRENCIES) are useful for schedulers and
dashboards.

**API sketch:**
```rust
MarketBuilder::new(&client, MarketRegion::Us).status().await -> MarketStatus { open: bool, opens_at, closes_at, timezone, message }
MarketBuilder::new(&client, MarketRegion::Europe).summary().await -> Vec<Quote>
```

**Endpoint:** `v6/finance/markettime`, `v6/finance/quote/marketSummary`.
**Effort:** S. **Priority:** P2.
**Risks:** According to Python, `markettime` ignores `market` and always returns US data, so only the US status is reliable. v6 endpoints may be deprecated without notice.

<!-- id: REQ-GAP-006.A1 -->
- [ ] `status()` returns parsed open/close times for US; other regions return `None` with a documented reason.
<!-- id: REQ-GAP-006.A2 -->
- [ ] `summary()` maps entries onto existing `Quote` types for all 8 regions.

<!-- id: REQ-GAP-007 -->
## REQ-GAP-007: Rate-limit resilience (`Retry-After`, client-side throttle)

**Motivation:** Bulk users (`download`, screeners, per-symbol fundamentals) hit 429s.
The crate already maps 429 to `YfError::RateLimited` and retries with backoff, but it
ignores `Retry-After` and has no request budget.

**API sketch:**
```rust
YfClient::builder()
    .rate_limit(RateLimit::per_second(4).burst(8))  // token bucket shared across clones
    .retry_config(RetryConfig { respect_retry_after: true, ..Default::default() })
```

**Endpoint:** all endpoints.
**Effort:** S. **Priority:** P2.
**Risks:** A shared limiter adds a lock or semaphore on the hot path, so the default should stay off (unlimited).

<!-- id: REQ-GAP-007.A1 -->
- [ ] On 429 with `Retry-After`, the retry delay is at least the header value, capped by `max_delay`.
<!-- id: REQ-GAP-007.A2 -->
- [ ] An optional token-bucket limiter caps outbound requests per `YfClient`, including clones.
<!-- id: REQ-GAP-007.A3 -->
- [ ] Diagnostics record the rate-limit wait time.

<!-- id: REQ-GAP-008 -->
## REQ-GAP-008: Calendars (earnings / IPO / splits / economic events)

**Motivation:** Market-wide event calendars (Python 1.0 `Calendars`) feed earnings-week
screening and macro dashboards. Today users need a second data provider for these.

**API sketch:**
```rust
CalendarsBuilder::new(&client).between(start, end).limit(100)
    .earnings().min_market_cap(2_000_000_000).most_active_only(false).fetch() -> Vec<EarningsEvent>
    // also .ipos(), .splits(), .economic_events()
```

**Endpoint:** `POST v1/finance/visualization?lang=en-US&region=US` with `entityIdType` = `sp_earnings` | `ipo_info` | `splits` | `economic_event`.
**Effort:** M. **Priority:** P2.
**Risks:** The query DSL resembles the screener's, so it can reuse `ScreenerQuery` operators. Python reports partial data staleness on this endpoint (see REQ-GAP-004). The endpoint needs a crumb, and the request should go through the existing crumb-retry path.

<!-- id: REQ-GAP-008.A1 -->
- [ ] All four calendar kinds return typed rows for a date window (default today plus 7 days).
<!-- id: REQ-GAP-008.A2 -->
- [ ] Market-cap and limit filters are applied server-side; page size is at most 100 with offset pagination.
<!-- id: REQ-GAP-008.A3 -->
- [ ] The query builder shares operator types with `screener::query`.

<!-- id: REQ-GAP-009 -->
## REQ-GAP-009: Sector and Industry domain objects

**Motivation:** Python's `Sector`/`Industry` provide overview stats, top companies, top
ETFs and mutual funds, industries within a sector, and top performing and growth
companies. These are useful for sector-rotation tooling.

**API sketch:**
```rust
SectorBuilder::new(&client, EquitySector::Technology).fetch() -> Sector { overview, top_companies, top_etfs, top_mutual_funds, industries, research_reports }
IndustryBuilder::new(&client, "semiconductors").fetch() -> Industry { overview, top_companies, top_performing, top_growth, sector_key }
```

**Endpoint:** `query1 v1/finance/sectors/{key}`, `v1/finance/industries/{key}`.
**Effort:** M. **Priority:** P2.
**Risks:** Keys are Yahoo slugs (`consumer-cyclical`), so the crate needs a mapping table and should accept raw slugs as an escape hatch. Python 1.4.0 added region scoping. The schema is undocumented.

<!-- id: REQ-GAP-009.A1 -->
- [ ] All 11 sectors resolve from `EquitySector`, and industries are listed with market weight.
<!-- id: REQ-GAP-009.A2 -->
- [ ] `Industry` exposes top performing and top growth companies with typed numeric fields.
<!-- id: REQ-GAP-009.A3 -->
- [ ] Unknown slugs yield `YfError::NotFound`/`InvalidParams`.

<!-- id: REQ-GAP-010 -->
## REQ-GAP-010: Optional history price repair

**Motivation:** Yahoo data has known defects: 100x unit errors (GBp/GBP, ILA), zero or
missing bars, wrong dividend adjustments, phantom dividends, unapplied splits, and
capital gains double-counted in Adj Close. Python's `repair=True` fixes many of these.
The crate already drops malformed rows and emits `YfWarning::RepairedData`, which is a
natural place to plug repair in.

**API sketch:**
```rust
ticker.history_builder().range(Range::Y5).repair(RepairOptions::all()).fetch_with_diagnostics()
// RepairOptions { unit_mixups, zero_bars /* needs finer-interval refetch */, dividend_adjust, splits, capital_gains }
```

**Endpoint:** v8 chart, plus finer-interval refetches for bar reconstruction.
**Effort:** L. **Priority:** P2.
**Risks:** Heuristic and statistically fragile, with possible false positives. Reconstruction multiplies request count. Ship in stages, with unit-mixup and dividend checks first. Every repair must be visible in diagnostics.

<!-- id: REQ-GAP-010.A1 -->
- [ ] With repair off (the default), output is byte-identical to today's.
<!-- id: REQ-GAP-010.A2 -->
- [ ] A 100x unit mixup fixture (e.g. a `.L` ticker) is corrected, with one `RepairedData` warning per bar.
<!-- id: REQ-GAP-010.A3 -->
- [ ] A capital-gains double-count fixture on a fund produces corrected adjusted closes.

<!-- id: REQ-GAP-011 -->
## REQ-GAP-011: Growth estimates with industry/sector/index comparison

**Motivation:** Python's `growth_estimates` compares the stock's growth estimates against
its industry, sector and the S&P 500. The crate exposes only the stock row via `earnings_trend`.

**API sketch:** `pub async fn growth_estimates(&self) -> Result<Vec<GrowthEstimateRow>, YfError>; // period, stock, industry, sector, index`
**Endpoint:** `quoteSummary?modules=earningsTrend,industryTrend,sectorTrend,indexTrend`.
**Effort:** S. **Priority:** P3.
**Risks:** Yahoo often returns empty industry/sector trend nodes, so those columns must be `Option`.

<!-- id: REQ-GAP-011.A1 -->
- [ ] Rows cover `0q, +1q, 0y, +1y, +5y, -5y` where present, with `None` for missing peers.
<!-- id: REQ-GAP-011.A2 -->
- [ ] One quoteSummary request is made.

<!-- id: REQ-GAP-012 -->
## REQ-GAP-012: Trending tickers and similar symbols

**Motivation:** Yahoo-only endpoints that Python does not cover. They are cheap to add and
useful for discovery and idea generation, and would set the crate apart.

**API sketch:** `trending(&client, Region::Us, count) -> Vec<String>`; `ticker.recommended_symbols() -> Vec<(String, f64)>`.
**Endpoint:** `v1/finance/trending/{region}`, `v6/finance/recommendationsbysymbol/{sym}`.
**Effort:** S. **Priority:** P3.
**Risks:** Liveness was not verified for this analysis. Mark both as `#[doc = "unstable"]` and gate them behind integration tests that tolerate empty responses.

<!-- id: REQ-GAP-012.A1 -->
- [ ] Both endpoints parse their fixtures; an empty `finance.result` returns an empty Vec, not an error.
<!-- id: REQ-GAP-012.A2 -->
- [ ] A live smoke test (ignored by default) confirms the endpoints still respond.

<!-- id: REQ-GAP-013 -->
## REQ-GAP-013: `Tickers` multi-symbol façade

**Motivation:** Python's `yf.Tickers("AAPL MSFT")` gives a single entry point. In Rust,
the useful part is fan-out with bounded concurrency over any per-ticker call, reusing
`DownloadConcurrency`.

**API sketch:**
```rust
let ts = Tickers::new(&client, ["AAPL", "MSFT"]).concurrency(DownloadConcurrency::new(4)?);
let infos: BTreeMap<String, Result<Info, YfError>> = ts.map(|t| async move { t.info().await }).await;
let quotes = ts.quotes().await?; // single v7 call
```

**Endpoint:** n/a (composition).
**Effort:** M. **Priority:** P3.
**Risks:** API-design churn. Keep it a thin, generic wrapper, and report partial failures per symbol rather than failing the whole call.

<!-- id: REQ-GAP-013.A1 -->
- [ ] `Tickers::map` runs at most N concurrent futures and returns per-symbol results.
<!-- id: REQ-GAP-013.A2 -->
- [ ] `Tickers::quotes` uses one batched v7 request.

<!-- id: REQ-GAP-014 -->
## REQ-GAP-014: Blocking wrapper feature

**Motivation:** Scripts, CLIs and Python-style notebooks (evcxr) want synchronous calls
without setting up tokio by hand.

**API sketch:** `features = ["blocking"]` → `yfinance_rs::blocking::{YfClient, Ticker}`, which mirror the async methods and run each call on an internal current-thread runtime.
**Endpoint:** n/a.
**Effort:** M. **Priority:** P3.
**Risks:** Calling from inside an existing runtime panics, so it must be detected with a clear error. Hand-written mirroring means maintenance drift; prefer macro generation.

<!-- id: REQ-GAP-014.A1 -->
- [ ] `blocking::Ticker::history/info/quote` work without `#[tokio::main]`.
<!-- id: REQ-GAP-014.A2 -->
- [ ] Use inside a tokio runtime returns an error instead of panicking.

<!-- id: REQ-GAP-015 -->
## REQ-GAP-015: Small parity items (keepna, lang/region scoping, convenience accessors)

**Motivation:** These are low-cost gaps:
- `keepna` (keep empty bars for calendar alignment).
- Client-wide `lang`/`region` defaults applied to all builders (Python 1.4.0).
- Thin `eps_trend()`, `eps_revisions()`, `revenue_estimate()`, `earnings_estimate()` accessors over `earnings_trend()` for discoverability.
- Insights (`ws/insights/v2/finance/insights`, P3, liveness unverified).

**API sketch:**
- `HistoryBuilder::keep_empty(bool)`
- `YfClientBuilder::lang("en-GB").region("GB")`
- `Ticker::eps_trend()` and the other accessors

**Endpoint:** existing.
**Effort:** S. **Priority:** P3.
**Risks:** Keeping empty rows conflicts with `Candle` invariants (non-optional OHLC), so it may need an `Option` row type or a separate `Vec<Gap>`.

<!-- id: REQ-GAP-015.A1 -->
- [ ] Client-level `lang`/`region` are sent on search, screener, news, quoteSummary and lookup requests.
<!-- id: REQ-GAP-015.A2 -->
- [ ] The convenience accessors return slices of the `earnings_trend()` data without extra HTTP calls.
<!-- id: REQ-GAP-015.A3 -->
- [ ] The keepna design decision (Option row vs gap list) is recorded before implementation.

<!-- id: GAP-DOC.OUTOFSCOPE -->
## Explicitly out of scope / not recommended

- **Login (`Auth`)**: carries credential-handling and ToS risk, with little value for public data.
- **majorDirectHolders, futuresChain, fundPerformance**: Python itself does not parse the first or the last. Revisit only on user demand.
- **v7 finance/download CSV**: deprecated by Yahoo, and v8 chart already covers it.

<!-- id: GAP-DOC.SOURCES -->
## Sources

- https://pypi.org/project/yfinance/ (latest 1.7.0, 2026-08-26)
- https://ranaroussi.github.io/yfinance/reference/index.html
- https://ranaroussi.github.io/yfinance/reference/api/yfinance.Ticker.html
- https://ranaroussi.github.io/yfinance/reference/yfinance.calendars.html
- https://ranaroussi.github.io/yfinance/reference/api/yfinance.Market.html
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/const.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/base.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/scrapers/history.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/scrapers/quote.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/scrapers/holders.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/scrapers/funds.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/calendars.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/domain/market.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/domain/domain.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/domain/sector.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/lookup.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/search.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/yfinance/screener/screener.py
- https://raw.githubusercontent.com/ranaroussi/yfinance/main/CHANGELOG.rst
- Local: `src/lib.rs`, `src/ticker/mod.rs`, `src/ticker/info.rs`, `src/core/client/constants.rs`, `src/core/quotes.rs`, `src/holders/api.rs`, `src/analysis/api.rs`, `src/fundamentals/api.rs`, `src/fundamentals/wire.rs`, `src/search/mod.rs`, `src/screener/predefined.rs`, `src/history/builder.rs`, `src/download/mod.rs`, `src/stream/mod.rs`, `src/core/client/retry.rs`, `src/core/net.rs`, `README.md`
- The Yahoo-only endpoints (trending, recommendationsbysymbol, insights) are listed from general knowledge of Yahoo's unofficial API. Their liveness was not verified for this analysis.
