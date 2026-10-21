---
id: YGAP-DOC
title: Yahoo Finance gap features (wave 1)
owner: unassigned
priority: high
version: 1
status: implemented
created: 2026-10-21
depends_on: [GAP-DOC, CORE-DOC]
---

<!-- id: YGAP-DOC -->
# Yahoo Finance gap features (wave 1)

Promotes eight proposals from `specs/2026-10-21-yahoo-feature-gap-analysis.md`
(REQ-GAP-001..008) into implementable requirements. Every feature follows the
house invariants in `CLAUDE.md`: a builder with `fetch()` / `fetch_with_diagnostics()`,
per-call `CallOptions`, projection through `ProjectionContext` (warn in best-effort,
fail in strict), overridable base URLs, all HTTP through `core::net`, and
`#[non_exhaustive]` public models. All features are additive (no breaking change to
existing public items).

Shared decisions (defaults chosen while Yahoo was rate-limiting the dev machine;
`[needs user confirmation]`):
- No HTML scraping and no new parsing dependencies; JSON endpoints only.
- New public models are crate-local unless a `paft` type already fits.
- Response shapes are taken from Python `yfinance` (main, 2026-10) parsers, since
  live fixtures could not be recorded. `[needs review — re-record fixtures live]`

<!-- id: REQ-YGAP-001 -->
## SEC filings MUST be available per ticker

`Ticker::sec_filings()` / `sec_filings_with_diagnostics()` and an `SecFilingsBuilder`
return the issuer's SEC filings from the quoteSummary `secFilings` module.
Each filing carries date, filing type (e.g. `10-K`), title, EDGAR URL and exhibits
(type → URL).

implementations:
  - src/filings/mod.rs:SecFilingsBuilder.fetch_with_diagnostics
  - src/ticker/gap_quote_summary.rs:Ticker.sec_filings

verifies:
  - tests/filings/synthetic.rs:sec_filings_requests_only_the_sec_filings_module
  - tests/filings/synthetic.rs:sec_filings_expose_date_type_title_edgar_url_and_all_exhibits
  - tests/filings/synthetic.rs:missing_sec_filings_module_returns_empty_with_warning_and_strict_error
  - tests/filings/synthetic.rs:sec_filing_with_unparseable_date_is_dropped_and_siblings_kept

<!-- id: REQ-YGAP-001-ACC -->
### Acceptance
<!-- id: REQ-YGAP-001.A1 -->
- The request asks for only the `secFilings` module.
<!-- id: REQ-YGAP-001.A2 -->
- Each filing exposes date, type, title, EDGAR URL and all exhibits.
<!-- id: REQ-YGAP-001.A3 -->
- A response without the module returns an empty list plus an unavailable-feature warning in best-effort mode, and an error in strict mode.
<!-- id: REQ-YGAP-001.A4 -->
- A filing with an unparseable date is dropped with a warning; its siblings are kept.

<!-- id: REQ-YGAP-002 -->
## Valuation measures and TTM statements MUST be available

From the fundamentals-timeseries endpoint:
- `Ticker::ttm_income_stmt()` and `Ticker::ttm_cashflow()` return trailing-twelve-month
  rows (the `trailing*` key prefix) in the same row types as the annual/quarterly statements.
- `Ticker::valuation_measures()` returns valuation history (market cap, enterprise value,
  trailing/forward P/E, PEG, P/S, P/B, EV/revenue, EV/EBITDA) per period, newest first.

Long key lists MUST be split into several requests whose results are merged, so a
single oversized `type=` query cannot fail the call.

implementations:
  - src/fundamentals/mod.rs:FundamentalsBuilder.ttm_income_statement
  - src/fundamentals/mod.rs:FundamentalsBuilder.ttm_cashflow
  - src/fundamentals/valuation.rs:ValuationBuilder.fetch_with_diagnostics
  - src/ticker/valuation.rs:Ticker.ttm_income_stmt
  - src/ticker/valuation.rs:Ticker.valuation_measures

verifies:
  - tests/fundamentals/ttm.rs:ttm_income_stmt_requests_only_trailing_keys_and_returns_single_row
  - tests/fundamentals/ttm.rs:ttm_cashflow_requests_only_trailing_keys_and_returns_single_row
  - tests/fundamentals/valuation.rs:valuation_measures_returns_one_row_per_as_of_date_newest_first
  - tests/fundamentals/valuation.rs:valuation_measures_every_ratio_is_optional
  - tests/fundamentals/chunking.rs:long_key_list_is_split_into_multiple_requests_and_merged
  - tests/fundamentals/chunking.rs:valuation_chunked_requests_merge_to_single_request_result
  - tests/fundamentals/chunking.rs:annual_and_quarterly_statement_requests_are_unchanged

<!-- id: REQ-YGAP-002-ACC -->
### Acceptance
<!-- id: REQ-YGAP-002.A1 -->
- `ttm_income_stmt` requests only `trailing*` keys and returns the rows Yahoo provides (normally one).
<!-- id: REQ-YGAP-002.A2 -->
- `valuation_measures` returns one row per as-of date, newest first, with every ratio optional.
<!-- id: REQ-YGAP-002.A3 -->
- A key list longer than the chunk size is sent as several requests, and the merged result matches what a single request would return.
<!-- id: REQ-YGAP-002.A4 -->
- Existing annual and quarterly statement calls send unchanged requests (no regression in fixture keys).

<!-- id: REQ-YGAP-003 -->
## Fund and ETF data MUST be available per ticker

`Ticker::funds_data()` and a `FundsDataBuilder` return, for ETFs and mutual funds:
description, fund overview (category, family, legal type), operations (expense ratio,
turnover, net assets), asset-class mix, top holdings (symbol, name, weight), equity
and bond holding statistics, bond ratings and sector weightings.

implementations:
  - src/funds/mod.rs:FundsDataBuilder.fetch_with_diagnostics
  - src/ticker/gap_quote_summary.rs:Ticker.funds_data

verifies:
  - tests/funds/synthetic.rs:etf_funds_data_populates_holdings_sectors_and_asset_classes
  - tests/funds/synthetic.rs:funds_data_weights_and_percentages_are_fractions
  - tests/funds/synthetic.rs:unknown_sector_keys_are_kept_and_known_keys_map_to_stable_names
  - tests/funds/synthetic.rs:non_fund_symbol_returns_error_naming_symbol_and_quote_type

<!-- id: REQ-YGAP-003-ACC -->
### Acceptance
<!-- id: REQ-YGAP-003.A1 -->
- An ETF response populates top holdings, sector weightings and asset classes.
<!-- id: REQ-YGAP-003.A2 -->
- Every weight and percentage uses one documented unit (fraction, 0.0–1.0).
<!-- id: REQ-YGAP-003.A3 -->
- Sector keys map to stable names; unknown sector keys are kept, not dropped.
<!-- id: REQ-YGAP-003.A4 -->
- A symbol that is not a fund (no `topHoldings` module) returns an error naming the symbol and quote type.

<!-- id: REQ-YGAP-004 -->
## Earnings history and earnings dates MUST be available

- `Ticker::earnings_history()` returns past quarters' EPS actual, estimate, difference
  and surprise percentage from the `earningsHistory` module, currency-aware like `earnings()`.
- `Ticker::earnings_dates(limit)` and an `EarningsDatesBuilder` return past and future
  report dates with EPS estimate, reported EPS and surprise %, from the visualization API
  (`entityIdType=earnings`), newest first.

Decision `[needs user confirmation]`: no HTML-scraping fallback. If Yahoo returns no rows,
the call returns an empty list with a warning that the data source may be stale.

implementations:
  - src/analysis/mod.rs:AnalysisBuilder.earnings_history
  - src/calendars/earnings_dates.rs:EarningsDatesBuilder.fetch_with_diagnostics
  - src/ticker/gap_quote_summary.rs:Ticker.earnings_history
  - src/ticker/earnings_dates.rs:Ticker.earnings_dates

verifies:
  - tests/analysis/earnings_history.rs:earnings_history_returns_one_row_per_quarter_with_eps_money_in_resolved_currency
  - tests/calendars/earnings_dates.rs:earnings_dates_posts_earnings_entity_for_symbol_and_returns_newest_first
  - tests/calendars/earnings_dates.rs:earnings_dates_returns_at_most_limit_rows
  - tests/calendars/earnings_dates.rs:earnings_dates_empty_result_warns_provider_feature_unavailable
  - tests/calendars/earnings_dates.rs:earnings_dates_empty_result_is_error_in_strict_mode
  - tests/calendars/earnings_dates.rs:earnings_dates_future_rows_have_no_reported_eps_or_surprise

<!-- id: REQ-YGAP-004-ACC -->
### Acceptance
<!-- id: REQ-YGAP-004.A1 -->
- `earnings_history` returns one row per reported quarter, with EPS amounts as money in the resolved currency.
<!-- id: REQ-YGAP-004.A2 -->
- `earnings_dates` returns rows newest first and at most `limit` rows (default 12, maximum 100).
<!-- id: REQ-YGAP-004.A3 -->
- An empty visualization result produces an empty list plus a provider-feature-unavailable warning, and an error in strict mode.
<!-- id: REQ-YGAP-004.A4 -->
- Future rows have no reported EPS or surprise; past rows carry both when Yahoo sends them.

<!-- id: REQ-YGAP-005 -->
## Rate limiting MUST honour Retry-After and MAY throttle client-side

- When Yahoo answers 429 (or 503) with a `Retry-After` header (seconds or HTTP date),
  the next retry waits at least that long, capped by the retry policy's maximum delay.
  This is on by default and can be turned off per `RetryConfig`.
- `YfClientBuilder::rate_limit(...)` optionally caps outbound requests per second with a
  burst allowance. The limit is shared by all clones of the client. Default: no limit.

implementations:
  - src/core/client/retry.rs:RetryConfig
  - src/core/client/mod.rs:YfClient.send_with_retry
  - src/core/client/rate_limit.rs:RateLimiter
  - src/core/client/mod.rs:YfClientBuilder.rate_limit

verifies:
  - tests/core/retry_after.rs:retry_after_seconds_on_429_delays_retry_at_least_that_long
  - tests/core/retry_after.rs:retry_after_longer_than_max_delay_is_capped_at_max_delay
  - tests/core/retry_after.rs:retry_after_ignored_when_respect_retry_after_is_false
  - tests/core/rate_limit.rs:rate_limit_spreads_requests_beyond_burst_across_cloned_clients
  - tests/core/rate_limit.rs:no_rate_limit_configured_adds_no_wait

<!-- id: REQ-YGAP-005-ACC -->
### Acceptance
<!-- id: REQ-YGAP-005.A1 -->
- A 429 with `Retry-After: 2` delays the retry by at least 2 s when the maximum delay allows it.
<!-- id: REQ-YGAP-005.A2 -->
- A `Retry-After` larger than the maximum delay is capped at the maximum delay.
<!-- id: REQ-YGAP-005.A3 -->
- With `respect_retry_after = false`, the existing backoff timing is unchanged.
<!-- id: REQ-YGAP-005.A4 -->
- With a limit of N requests per second and burst B, a burst of B+k requests from cloned clients spreads the extra k over roughly k/N seconds.
<!-- id: REQ-YGAP-005.A5 -->
- With no limit configured, request timing is unchanged (no added wait).

<!-- id: REQ-YGAP-006 -->
## Market status and summary MUST be available per region

`MarketBuilder::new(&client, region)` exposes:
- `status()`: open/closed state, open and close times, timezone and message, from `v6/finance/markettime`.
- `summary()`: index and benchmark quotes for the region, from `v6/finance/quote/marketSummary`.

Regions: US, GB, ASIA, EUROPE, RATES, COMMODITIES, CURRENCIES, CRYPTOCURRENCIES.
Yahoo's markettime endpoint is known to ignore the region and return US market time;
the crate detects this rather than mislabelling US data. `[needs review — confirm live]`

implementations:
  - src/market/mod.rs:MarketBuilder.status
  - src/market/mod.rs:MarketBuilder.summary

verifies:
  - tests/market/offline.rs:market_status_returns_utc_session_times_and_exchange_timezone
  - tests/market/offline.rs:market_summary_returns_one_entry_per_index_with_price_and_change
  - tests/market/offline.rs:market_summary_drops_malformed_entries_with_warnings
  - tests/market/offline.rs:market_requests_send_region_as_yahoo_market_code_for_all_regions
  - tests/market/offline.rs:market_status_for_non_us_region_rejects_us_market_time_with_warning_and_fails_strict

<!-- id: REQ-YGAP-006-ACC -->
### Acceptance
<!-- id: REQ-YGAP-006.A1 -->
- `status()` returns the open/close times as UTC timestamps plus the exchange timezone.
<!-- id: REQ-YGAP-006.A2 -->
- `summary()` returns one entry per index, with symbol, name, price, change and change percent; malformed entries are dropped with warnings.
<!-- id: REQ-YGAP-006.A3 -->
- The region is sent as Yahoo's `market` parameter code (`US`, `GB`, `ASIA`, `EUROPE`, `RATES`, `COMMODITIES`, `CURRENCIES`, `CRYPTOCURRENCIES`).
<!-- id: REQ-YGAP-006.A4 -->
- For a non-US region, when Yahoo answers with US market time, no status is returned as that region's; a provider-feature-unavailable warning is recorded instead (an error in strict mode).

<!-- id: REQ-YGAP-007 -->
## Symbol lookup by asset type MUST be available

`LookupBuilder::new(&client, query)` with `.kind(LookupKind)`, `.count(n)` and `.offset(n)`
returns matching instruments from `v1/finance/lookup`. Kinds: All, Equity, MutualFund,
ETF, Index, Future, Currency, Cryptocurrency. The existing `search()` API is not changed.

implementations:
  - src/lookup/mod.rs:LookupBuilder.fetch_with_diagnostics

verifies:
  - tests/lookup/offline.rs:lookup_sends_each_kind_as_yahoo_type_with_start_and_count
  - tests/lookup/offline.rs:lookup_results_expose_symbol_name_quote_type_exchange_and_last_price
  - tests/lookup/offline.rs:lookup_rejects_empty_query_before_any_request

<!-- id: REQ-YGAP-007-ACC -->
### Acceptance
<!-- id: REQ-YGAP-007.A1 -->
- Each kind is sent as Yahoo's `type` value (e.g. `etf`, `cryptocurrency`), with `start` and `count`.
<!-- id: REQ-YGAP-007.A2 -->
- Results expose symbol, name, quote type, exchange and the last price when Yahoo includes it.
<!-- id: REQ-YGAP-007.A3 -->
- An empty query is rejected with `InvalidParams` before any request is sent.

<!-- id: REQ-YGAP-008 -->
## Market-wide event calendars MUST be available

`CalendarsBuilder::new(&client)` with `.between(start, end)`, `.limit(n)` and `.offset(n)`
fetches four calendars from the visualization API:
- `earnings()`: optional minimum market cap and most-active-only filter
- `ipos()`
- `splits()`
- `economic_events()`

The default window is today plus 7 days. The page size is at most 100.

implementations:
  - src/calendars/mod.rs:CalendarsBuilder.earnings
  - src/calendars/mod.rs:CalendarsBuilder.ipos
  - src/calendars/mod.rs:CalendarsBuilder.splits
  - src/calendars/mod.rs:CalendarsBuilder.economic_events

verifies:
  - tests/calendars/offline.rs:earnings_calendar_posts_sp_earnings_entity_with_date_window_and_returns_typed_rows
  - tests/calendars/offline.rs:ipo_calendar_posts_ipo_info_entity_with_gtelt_date_filters
  - tests/calendars/offline.rs:calendars_limit_above_100_is_rejected_before_any_request
  - tests/calendars/offline.rs:calendars_offset_is_passed_through_for_paging
  - tests/calendars/offline.rs:calendars_request_retries_once_with_fresh_crumb_when_crumb_is_invalid
  - tests/calendars/offline.rs:calendar_rows_with_unparseable_dates_are_dropped_with_warnings

<!-- id: REQ-YGAP-008-ACC -->
### Acceptance
<!-- id: REQ-YGAP-008.A1 -->
- Each calendar POSTs the documented `entityIdType` with a date-range filter and returns typed rows.
<!-- id: REQ-YGAP-008.A2 -->
- A limit above 100 is rejected with `InvalidParams`; offset is passed through for paging.
<!-- id: REQ-YGAP-008.A3 -->
- The request goes through the crumb-auth path and is retried once when the crumb is invalid.
<!-- id: REQ-YGAP-008.A4 -->
- Rows with unparseable dates are dropped with warnings; their siblings are kept.

<!-- id: YGAP-NONGOALS -->
## Non-goals

- HTML scraping fallbacks (earnings dates, calendars).
- Extending `SearchResponse` with news/lists/research blocks (it is a `paft` type, so that is a semver-affecting change).
- Sector/industry pages, trending tickers, price repair (later waves of the gap analysis).

<!-- id: YGAP-OPEN -->
## Open questions

- Confirm the HTML-fallback decision for earnings dates (REQ-YGAP-004).
- Re-record all fixtures live once Yahoo stops rate-limiting the recording machine.
- Should `rate_limit` also apply to WebSocket connects? Default: no, HTTP only.
