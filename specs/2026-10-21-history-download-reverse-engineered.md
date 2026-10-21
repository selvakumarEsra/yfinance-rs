---
id: HIST-DOC
title: Historical price history (chart endpoint) and multi-symbol download
owner: unassigned
priority: high
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/history/mod.rs
  - src/history/wire.rs
  - src/history/builder.rs
  - src/history/builder/fetch.rs
  - src/history/builder/adjust.rs
  - src/history/builder/actions.rs
  - src/history/builder/assemble.rs
  - src/download/mod.rs
  - src/core/services.rs
  - src/core/client/urls.rs
  - src/core/models.rs
  - src/ticker/mod.rs
  - src/lib.rs
  - tests/history.rs
  - tests/history/action_currency_synthetic.rs
  - tests/history/adjust.rs
  - tests/history/adjust_from_splits_only.rs
  - tests/history/caching_synthetic.rs
  - tests/history/fractional_splits.rs
  - tests/history/http_status_error.rs
  - tests/history/intervals.rs
  - tests/history/live.rs
  - tests/history/malformed_ohlc.rs
  - tests/history/meta.rs
  - tests/history/nulls_synthetic.rs
  - tests/history/offline.rs
  - tests/history/params.rs
  - tests/history/ranges_new.rs
  - tests/history/retry_synthetic.rs
  - tests/history/smoke.rs
  - tests/history/volume_adjustment.rs
  - tests/download.rs
  - tests/download/back_adjust.rs
  - tests/download/live.rs
  - tests/download/malformed_rounding.rs
  - tests/download/no_repair.rs
  - tests/download/offline.rs
  - tests/common.rs
  - tests/fixtures/history_chart_AAPL.json
  - tests/fixtures/history_chart_MSFT.json
  - tests/fixtures/history_chart_IBM.json
  - tests/fixtures/history_chart_VFINX.json
  - tests/fixtures/history_chart_AXIA-P.json
  - tests/fixtures/history_chart_NVDA_SPLIT_VOLUME.json
  - tests/fixtures/history_chart_TSCO.L.json
  - examples/04_historical_actions.rs
  - examples/05_concurrent_requests.rs
  - examples/14_polars_dataframes.rs
  - CHANGELOG.md
---

<!-- id: HIST-DOC -->
# Historical price history and multi-symbol download (reverse-engineered)

Intent: `yfinance-rs` (v0.9.1) exposes Yahoo Finance's `/v8/finance/chart/{symbol}` endpoint as a typed,
diagnostics-aware historical-data API. `HistoryBuilder` fetches one symbol's OHLCV candles, corporate actions
(dividends, splits, capital gains), and exchange metadata, optionally price-adjusted. `DownloadBuilder` fans the same
request out across many symbols concurrently (documented as "similar to `yfinance.download` in Python") and adds
download-only transforms (back-adjustment, `priceHint` rounding, instrument fallback). Malformed provider data is
projected through a `ProjectionContext`: in best-effort mode bad rows/fields are dropped and reported as
`YfWarning`s; in strict mode (`.strict()` / `DataQuality::Strict`) the first such diagnostic becomes
`YfError::DataQuality`. Output types (`Candle`, `HistoryResponse`, `Action`, `OhlcPriceBasis`, `DownloadResponse`)
come from the external `paft` 0.9 crate.

Note on test status: every `[x]` below is backed by a test function that exists in the repository. The tests were
**not executed** while writing this spec because the local toolchain (rustc 1.88) is below the crate MSRV (1.91).
[needs review — confirm the cited tests pass on rustc >= 1.91]

<!-- id: REQ-HIST-001 -->
## HistoryBuilder construction, defaults, and fetch entry points

`HistoryBuilder` is a cloneable, by-value builder for one symbol.

- `HistoryBuilder::new(client: &YfClient, symbol: impl Into<String>) -> Self`
- Defaults: `range = Some(Range::M6)`, `period = None`, `interval = Interval::D1`, `auto_adjust = true`,
  `include_prepost = false`, `include_actions = true`, `CallOptions::default()`.
- Setters (all `#[must_use]`, consume `self`): `range(Range)`, `between(DateTime<Utc>, DateTime<Utc>)`,
  `interval(Interval)`, `auto_adjust(bool)`, `prepost(bool)`, `actions(bool)`, plus the shared call-option setters
  `cache_mode(CacheMode)`, `retry_policy(Option<RetryConfig>)`, `data_quality(DataQuality)`, `strict()`.
- `range()` clears any period; `between()` clears any range and stores whole-second Unix timestamps
  (sub-second precision is truncated).
- Fetch methods, all `async` and returning `Result<_, YfError>`:
  - `fetch() -> Vec<Candle>`
  - `fetch_with_diagnostics() -> YfResponse<Vec<Candle>>`
  - `fetch_full() -> HistoryResponse` (candles, actions, price_basis, meta)
  - `fetch_full_with_diagnostics() -> YfResponse<HistoryResponse>`
- `Ticker::history_builder()` returns a builder pre-seeded with the ticker's cache mode, data quality and retry policy.
- `impl HistoryService for YfClient { fetch_full_history(&self, symbol, HistoryRequest) }` maps a
  `HistoryRequest` (interval, auto_adjust, include_prepost, include_actions, period | range) onto a builder;
  `period` wins over `range`. It does **not** forward any `CallOptions` [needs review — see HIST-REVIEW F7].

implementations:
  - src/history/builder.rs:HistoryBuilder.new
  - src/history/builder.rs:HistoryBuilder.range
  - src/history/builder.rs:HistoryBuilder.between
  - src/history/builder.rs:HistoryBuilder.interval
  - src/history/builder.rs:HistoryBuilder.auto_adjust
  - src/history/builder.rs:HistoryBuilder.prepost
  - src/history/builder.rs:HistoryBuilder.actions
  - src/history/builder.rs:HistoryBuilder.fetch
  - src/history/builder.rs:HistoryBuilder.fetch_with_diagnostics
  - src/history/builder.rs:HistoryBuilder.fetch_full
  - src/history/builder.rs:HistoryBuilder.fetch_full_with_diagnostics
  - src/history/mod.rs:YfClient.fetch_full_history
  - src/ticker/mod.rs:Ticker.history_builder

verifies:
  - tests/history/smoke.rs:history_happy_path
  - tests/history/smoke.rs:history_no_data_is_ok
  - tests/history/caching_synthetic.rs:history_serves_from_cache_on_second_call

<!-- id: REQ-HIST-001.A1 -->
- [x] A builder created with only `new(client, "AAPL")` requests `range=6mo` and `interval=1d` with actions included.
<!-- id: REQ-HIST-001.A2 -->
- [x] `fetch()` against a recorded 6-month AAPL fixture returns more than 100 candles with positive OHLC values.
<!-- id: REQ-HIST-001.A3 -->
- [ ] Calling `between(..)` after `range(..)` (and vice versa) leaves only the last-set time selector in the request. [needs review — no test backs this]
<!-- id: REQ-HIST-001.A4 -->
- [ ] `HistoryService::fetch_full_history` with both `period` and `range` set uses `period`; an out-of-range `period1`/`period2` returns `YfError::InvalidParams("invalid period1"/"invalid period2")`. [needs review — no test backs this]

<!-- id: REQ-HIST-002 -->
## Chart request construction (URL and query parameters)

The builder issues a single `GET {base_chart}/{normalized_symbol}` with query parameters:

- Time selector: either `range=<Range::code()>` or `period1=<start>&period2=<end>` (Unix seconds). `Range::Max`
  is never sent as `range=max` (see REQ-HIST-004).
- `interval=<Interval::code()>`.
- `events=div|split|capitalGains` only when `actions(true)` (default).
- `includePrePost=true|false` always present.
- The symbol is validated/normalized by `normalize_symbol` (paft `Symbol` rules, rejects `.`/`..`, requires at
  least one ASCII alphanumeric, allows only `[A-Za-z0-9._-=^]`); invalid symbols fail with
  `YfError::InvalidParams` before any request.

implementations:
  - src/history/builder/fetch.rs:fetch_chart
  - src/history/builder/fetch.rs:ChartFetchRequest
  - src/core/client/urls.rs:normalize_symbol
  - src/core/models.rs:range_as_str
  - src/core/models.rs:interval_as_str

verifies:
  - tests/history/params.rs:history_has_expected_query_params
  - tests/history/intervals.rs:history_allows_intraday_interval
  - tests/history/ranges_new.rs:history_range_1d_ytd_10y
  - tests/history/smoke.rs:history_absolute_range_happy
  - tests/history/retry_synthetic.rs:history_retries_on_persistent_5xx

<!-- id: REQ-HIST-002.A1 -->
- [x] `range(Range::M6)` produces `range=6mo&interval=1d&events=div|split|capitalGains`.
<!-- id: REQ-HIST-002.A2 -->
- [x] `range(Range::D1|Ytd|Y10)` produce `range=1d|ytd|10y` respectively.
<!-- id: REQ-HIST-002.A3 -->
- [x] `range(Range::D5).interval(Interval::I5m)` produces `range=5d&interval=5m&includePrePost=false` and parses >100 recorded five-minute candles.
<!-- id: REQ-HIST-002.A4 -->
- [x] `between(start, end)` produces `period1=<start.timestamp()>&period2=<end.timestamp()>`.
<!-- id: REQ-HIST-002.A5 -->
- [ ] `actions(false)` omits the `events` parameter entirely and `prepost(true)` sends `includePrePost=true`. [needs review — no test backs this]

<!-- id: REQ-HIST-003 -->
## Time-window validation

- An explicit period takes precedence over any range at request-build time.
- `start >= end` returns `YfError::InvalidDates` before any network call.
- Neither range nor period set returns `YfError::InvalidParams("no range or period set")`
  [needs review — unreachable through the public builder, which always has one of them].

implementations:
  - src/history/builder/fetch.rs:ChartFetchRequest.time_query
  - src/history/builder/fetch.rs:checked_period

verifies:
  - tests/history/smoke.rs:history_between_invalid_dates
  - src/history/builder/fetch.rs:explicit_period_takes_precedence_over_range
  - src/history/builder/fetch.rs:non_max_range_stays_relative

<!-- id: REQ-HIST-003.A1 -->
- [x] `between(2024-01-10, 2024-01-09)` fails with `YfError::InvalidDates`.
<!-- id: REQ-HIST-003.A2 -->
- [x] A request carrying both `Range::Max` and period `(100, 200)` resolves to `Period { start: 100, end: 200 }`.
<!-- id: REQ-HIST-003.A3 -->
- [x] A non-Max range (e.g. `Range::M6`) is sent as a relative `range` parameter unchanged.
<!-- id: REQ-HIST-003.A4 -->
- [ ] `between(t, t)` (equal instants) fails with `YfError::InvalidDates`. [needs review — no test backs this]

<!-- id: REQ-HIST-004 -->
## `Range::Max` is translated into an explicit, interval-aware window

Introduced in 0.9.1 (commit c21eca0). Code comment: "Yahoo silently coarsens `range=max` responses. Match Python
yfinance by translating the semantic maximum into an explicit retention-aware window." CHANGELOG: prevents Yahoo
returning quarterly candles for a daily request.

- `end = ceil(now, bucket)` where `bucket = interval.seconds()` or 86 400 s when the interval has no fixed length;
  this makes equivalent calls share a cache key within one bucket.
- Lookback by interval code: `1m` → 8 days; `2m|5m|15m|30m|90m` → 60 days; `1h|60m` → 730 days; everything else
  (daily and coarser) → 99 × 365 days.
- `start = end - lookback + 5` (5 s processing slack).
- All arithmetic is checked; overflow returns `YfError::InvalidParams("maximum history window exceeds timestamp bounds")`.
- The request is sent as `period1/period2`; no `range` parameter.

implementations:
  - src/history/builder/fetch.rs:max_range_period
  - src/history/builder/fetch.rs:timestamp_ceiling
  - src/history/builder/fetch.rs:ChartFetchRequest.time_query

verifies:
  - src/history/builder/fetch.rs:max_range_uses_python_compatible_interval_windows
  - src/history/builder/fetch.rs:max_range_query_is_stable_within_interval_bucket
  - src/history/builder/fetch.rs:max_range_timestamp_arithmetic_is_checked
  - tests/history/params.rs:history_max_daily_uses_explicit_window_and_preserves_granularity

<!-- id: REQ-HIST-004.A1 -->
- [x] For each of `1m, 2m, 5m, 15m, 30m, 90m, 1h, 1d, 1mo` the window equals `(ceil(now) - lookback + 5, ceil(now))` with the lookbacks above.
<!-- id: REQ-HIST-004.A2 -->
- [x] Two daily Max requests one hour apart within the same day produce identical windows.
<!-- id: REQ-HIST-004.A3 -->
- [x] `now = i64::MIN` or `i64::MAX` yields `YfError::InvalidParams` instead of overflowing.
<!-- id: REQ-HIST-004.A4 -->
- [x] `HistoryBuilder::new(.., "VFINX").range(Range::Max).interval(Interval::D1)` sends no `range` param, a day-aligned `period2` within a day of now, span `99*365 days - 5 s`, and yields >5 000 daily candles from the recorded fixture.
<!-- id: REQ-HIST-004.A5 -->
- [ ] Weekly (`1wk`) and other intervals whose `seconds()` is defined but not daily bucket `end` to their own length (week buckets are aligned to the Unix epoch, i.e. Thursdays). [needs review — no test backs this; epoch alignment may be unintended]

<!-- id: REQ-HIST-005 -->
## Granularity validation and chart caching

- After decoding, if `chart.result[0].meta.dataGranularity` is present and non-blank it must equal the requested
  interval code (`1h` and `60m` are treated as equivalent). Otherwise the call fails with
  `YfError::InvalidData("Yahoo returned {actual} chart granularity for requested {expected} interval")` in **every**
  data-quality mode, and the cached body for that URL is invalidated.
- Missing or blank `dataGranularity` is accepted.
- Bodies are fetched through the shared text cache (`CacheEndpoint::Chart`, fixture key = symbol, ext `json`);
  a body is only cached if it decodes (`validate_chart_body`). `CacheMode::Refresh` bypasses the cache read but
  updates the cache.

implementations:
  - src/history/builder/fetch.rs:validate_chart_granularity
  - src/history/builder/fetch.rs:validate_chart_body
  - src/history/builder/fetch.rs:fetch_chart

verifies:
  - src/history/builder/fetch.rs:chart_granularity_must_match_requested_interval
  - tests/history/meta.rs:mismatched_chart_granularity_is_rejected_in_every_quality_mode
  - tests/history/caching_synthetic.rs:history_serves_from_cache_on_second_call
  - tests/history/caching_synthetic.rs:history_cache_refresh_bypasses_cache_get_but_updates_cache

<!-- id: REQ-HIST-005.A1 -->
- [x] Requested `1d` with `dataGranularity: "3mo"` fails with `InvalidData` mentioning both `3mo` and `1d`, in both best-effort and strict mode.
<!-- id: REQ-HIST-005.A2 -->
- [x] With a cache TTL configured, the rejected body is not served from cache: two calls hit the server twice.
<!-- id: REQ-HIST-005.A3 -->
- [x] `dataGranularity` absent, blank, or `60m` for a requested `1h` is accepted.
<!-- id: REQ-HIST-005.A4 -->
- [x] With caching enabled, a second identical fetch is served from cache (one HTTP call); `CacheMode::Refresh` forces a call and the following default call is served from the refreshed cache.

<!-- id: REQ-HIST-006 -->
## Response envelope decoding and transport errors

- JSON parse failure → `YfError::json(..)`.
- `chart` missing → `MissingData("missing chart")`; `chart.error` present → `YfError::Api("chart error: {code} - {description}")`;
  `result` missing → `MissingData("missing result")`; empty result → `MissingData("empty result")`;
  no `indicators.quote[0]` → `MissingData("missing quote")`.
- Only `result[0]` is used. `adjclose[0].adjclose` is optional (defaults to empty).
- `timestamp` missing: OK (empty history) only if every quote array **and** adjclose are empty; otherwise
  `MissingData("missing timestamps")`.
- Non-2xx HTTP → `YfError::ServerError { status, url }` (404 surfaces as a "Not found at <url>" error); 5xx is
  retried per the client `RetryConfig` (initial + `max_retries` attempts).

implementations:
  - src/history/builder/fetch.rs:decode_chart
  - src/history/builder/fetch.rs:quote_block_is_empty
  - src/history/wire.rs:ChartEnvelope
  - src/history/wire.rs:ChartResult

verifies:
  - tests/history/meta.rs:missing_timestamp_with_empty_chart_series_returns_empty_history
  - tests/history/meta.rs:missing_timestamp_with_quote_data_remains_malformed
  - tests/history/http_status_error.rs:history_returns_status_error_on_non_2xx
  - tests/history/retry_synthetic.rs:history_retries_on_persistent_5xx

<!-- id: REQ-HIST-006.A1 -->
- [x] A result with no `timestamp` and empty `quote`/`adjclose` objects returns empty candles and actions, populated meta, and no diagnostics.
<!-- id: REQ-HIST-006.A2 -->
- [x] A result with no `timestamp` but quote data fails with `MissingData("missing timestamps")`.
<!-- id: REQ-HIST-006.A3 -->
- [x] HTTP 500 with retries disabled yields `ServerError { status: 500, url }` where `url` contains `/v8/finance/chart/FAIL`, after exactly one call.
<!-- id: REQ-HIST-006.A4 -->
- [x] Persistent 503 with `max_retries = 3` is attempted 4 times, then returns `ServerError { status: 503 }`.
<!-- id: REQ-HIST-006.A5 -->
- [ ] A body with `chart.error = {code, description}` yields `YfError::Api` containing both. [needs review — no test backs this]

<!-- id: REQ-HIST-007 -->
## Candle assembly and malformed-row handling

For each timestamp index `i`:

- Invalid timestamp (not representable as `DateTime<Utc>`) → row dropped, `DroppedItem{item:"candle", key: ts, InvalidField{field:"timestamp"}}`.
- Any of open/high/low/close null or missing → row dropped with `MissingRequiredFields{fields:[...]}` listing the missing ones.
- Any raw OHLC value non-finite or not representable as `Decimal` → row dropped with `InvalidField` **before**
  adjustment (an adjusted value that would fit does not rescue it).
- Adjusted values that fail currency/price conversion → dropped with `ConversionFailed{target:"candle prices"}`.
- `close_unadj` = raw close in the resolved currency; if unconvertible it is omitted with an
  `OmittedPresentField{"quote.close_unadj"}` warning but the candle is kept.
- `volume` is taken verbatim from Yahoo (never adjusted); null volume → `None`.
- Arrays shorter than `timestamp` are treated as null for missing indices.
- In strict mode the first dropped row returns `YfError::DataQuality`.
- Price outliers are **not** repaired (download test asserts a 100× spike is preserved).

implementations:
  - src/history/builder/assemble.rs:assemble_candles
  - src/history/builder/assemble.rs:raw_ohlc_values
  - src/history/builder/assemble.rs:valid_decimal_value
  - src/history/builder/assemble.rs:candle_prices
  - src/history/builder/assemble.rs:candle_capacity_upper_bound

verifies:
  - tests/history/malformed_ohlc.rs:history_drops_malformed_ohlc_rows
  - tests/history/malformed_ohlc.rs:history_reports_dropped_malformed_ohlc_rows
  - tests/history/malformed_ohlc.rs:strict_history_errors_on_dropped_malformed_ohlc_rows
  - tests/history/malformed_ohlc.rs:history_drops_unrepresentable_raw_ohlc_before_adjustment
  - tests/history/nulls_synthetic.rs:history_skips_points_with_null_ohlc
  - tests/download/no_repair.rs:download_preserves_suspicious_price_outliers
  - src/history/builder/assemble.rs:candle_capacity_upper_bound_uses_shortest_required_array

<!-- id: REQ-HIST-007.A1 -->
- [x] An all-null OHLC row is dropped; the valid neighbour is kept with its own volume.
<!-- id: REQ-HIST-007.A2 -->
- [x] The drop is reported as `DroppedItem{item:"candle", reason: MissingRequiredFields{["open","high","low","close"]}}` and `is_lossless()` is false.
<!-- id: REQ-HIST-007.A3 -->
- [x] Strict mode turns the dropped row into `YfError::DataQuality`.
<!-- id: REQ-HIST-007.A4 -->
- [x] A row with raw OHLC `1e30` is dropped even though its adjclose (100.0) would be representable.
<!-- id: REQ-HIST-007.A5 -->
- [x] A row with only `open` null is dropped.
<!-- id: REQ-HIST-007.A6 -->
- [x] A 100× single-bar price spike is passed through unchanged, including `close_unadj`.

<!-- id: REQ-HIST-008 -->
## Trading-currency resolution for candles

- Trading currency is resolved only if at least one row has complete OHLC; resolution uses
  `YfClient::resolve_trading_currency` with `chart.meta.currency` as evidence (may fall back to the quote endpoint).
- If it cannot be resolved, **all** candles are dropped with a single `DroppedItem{item:"candle", key: None}`
  carrying the resolver's issue (`CurrencyUnresolved` or `InvalidCurrency{code}`); strict mode errors.
- If no row is complete but some OHLC values exist, a single `DroppedItem{"candle", MissingRequiredFields}` is emitted.
- Every `Candle.currency` equals the resolved currency; minor-unit currencies (e.g. `GBp`) are converted to the major
  unit (`GBP`) [needs review — conversion lives in `currency_resolver`, outside this scope].
- When chart meta has no currency and the symbol has no suffix, candles resolve to USD in tests
  [needs review — the default/fallback rule lives in `currency_resolver`; not derivable from this scope].

implementations:
  - src/history/builder.rs:history_trading_currency
  - src/history/builder.rs:has_complete_ohlc_row
  - src/history/builder.rs:has_any_ohlc_value
  - src/history/builder.rs:HistoryBuilder.fetch_full_yahoo_with_diagnostics

verifies:
  - tests/history/action_currency_synthetic.rs:candles_without_resolved_currency_are_dropped_with_diagnostics
  - tests/history/action_currency_synthetic.rs:candles_with_invalid_chart_currency_are_dropped_with_diagnostics
  - tests/history/nulls_synthetic.rs:history_skips_points_with_null_ohlc
  - tests/download/offline.rs:rounding_uses_recorded_minor_unit_price_hint_before_scaling

<!-- id: REQ-HIST-008.A1 -->
- [x] No chart currency and an empty quote-endpoint lookup → empty candles and first warning `DroppedItem{"candle", CurrencyUnresolved}`.
<!-- id: REQ-HIST-008.A2 -->
- [x] Chart currency `"!!!"` → empty candles with `DroppedItem{"candle", InvalidCurrency{code:"!!!"}}` in best-effort; `DataQuality` error in strict.
<!-- id: REQ-HIST-008.A3 -->
- [x] TSCO.L (`GBp`) candles are reported in `GBP`.

<!-- id: REQ-HIST-009 -->
## Price adjustment (auto_adjust) and price basis

When `auto_adjust(true)` (default) an `AdjustmentPlan` is selected per series:

- **ProviderAdjusted**: if every emitted (complete, representable) row has `adjclose` and non-zero close, each row's
  OHLC is multiplied by `adjclose[i] / close[i]`.
- **SplitAdjusted**: if no row has a usable adjclose, each row's OHLC is multiplied by
  `1 / product(split ratios with event ts > row ts)`.
- **Sparse adjclose** (some but not all rows): falls back to SplitAdjusted for **all** rows and emits
  `RepairedData{item:"candle_adjustment", repair:"ignored sparse chart.indicators.adjclose and used split-only adjustment for all candles"}`;
  strict mode errors.
- `auto_adjust(false)`: raw prices, no plan.
- Volume is never adjusted. `close_unadj` always holds the raw close.
- `HistoryResponse.price_basis`: ProviderAdjusted → `uniform(provider_latest_adjusted())`; SplitAdjusted with at least
  one non-unit cumulative factor → `uniform(split_adjusted_latest())`; otherwise `OhlcPriceBasis::raw()`.

[needs review — Yahoo chart OHLC is commonly understood to already be split-adjusted; the SplitAdjusted path may
double-apply splits on real data. Synthetic tests encode the current behaviour. See HIST-REVIEW F1.]

implementations:
  - src/history/builder/assemble.rs:adjustment_plan_for_series
  - src/history/builder/adjust.rs:AdjustmentPlan
  - src/history/builder/adjust.rs:AdjustmentPlan.factor_for_row
  - src/history/builder/adjust.rs:AdjustmentBasis
  - src/history/builder/adjust.rs:cumulative_split_after
  - src/history/builder/adjust.rs:provider_adjustment_factor
  - src/history/builder/adjust.rs:split_adjustment_factor
  - src/history/builder.rs:history_adjustment_plan
  - src/history/builder.rs:history_price_basis

verifies:
  - tests/history/adjust.rs:history_auto_adjust_and_actions
  - tests/history/adjust.rs:sparse_adjclose_uses_one_split_adjusted_basis_with_diagnostic
  - tests/history/adjust_from_splits_only.rs:history_auto_adjust_uses_splits_when_adjclose_missing
  - tests/history/volume_adjustment.rs:download_auto_adjust_leaves_yahoo_volume_unchanged_across_split
  - src/history/builder/assemble.rs:provider_adjustment_plan_carries_row_factors

<!-- id: REQ-HIST-009.A1 -->
- [x] Closes `[100,100,100]` with adjclose `[50,100,99]` yield adjusted closes `[50,100,99]`, open/high/low scaled by the same factor (e.g. 50/50.5/49.5), volume unchanged, basis `uniform(provider_latest_adjusted)`.
<!-- id: REQ-HIST-009.A2 -->
- [x] No adjclose plus a 2:1 split at the second bar yields closes `[50,100]` and basis `uniform(split_adjusted_latest)`.
<!-- id: REQ-HIST-009.A3 -->
- [x] Adjclose `[50,null,99]` yields split-only closes `[50,100,100]`, basis `split_adjusted_latest`, and the `candle_adjustment` repair warning; strict mode fails with that warning.
<!-- id: REQ-HIST-009.A4 -->
- [x] Across a real NVDA split fixture, Yahoo volumes (412 386 000 / 313 434 100) are passed through unchanged.
<!-- id: REQ-HIST-009.A5 -->
- [ ] `auto_adjust(false)` returns raw OHLC and basis `OhlcPriceBasis::raw()` even when adjclose and splits are present. [needs review — no test backs this]
<!-- id: REQ-HIST-009.A6 -->
- [ ] A split whose event timestamp equals a bar timestamp does not adjust that bar (strict `>` comparison). [needs review — no test backs this]

<!-- id: REQ-HIST-010 -->
## Corporate actions: dividends, capital gains, splits

Extracted from `result[0].events` (requested only when `actions(true)`):

- Output `Vec<Action>`: `Action::Dividend{date, amount}`, `Action::CapitalGain{date, gain}`,
  `Action::Split{date, numerator: NonZeroU32, denominator: NonZeroU32}`; sorted ascending by date (stable sort;
  ties keep dividend → capital gain → split insertion order).
- Event timestamp = the map key parsed as `i64`, falling back to the `date` field; `date` = UTC calendar date of that
  timestamp [needs review — UTC date may be off by one for non-UTC exchanges; see HIST-REVIEW F4].
- Amount currency precedence: event `currency` (non-blank) → trading currency → corporate-action currency resolver
  (only invoked if some dividend/gain lacks a currency and there is no trading currency). An invalid event currency
  drops that event with `InvalidCurrency`; no currency at all drops it with `CurrencyUnresolved`. A resolver error is
  an `OmittedPresentField{"chart.meta.currency", ProviderError}` warning in best-effort mode and a hard error in strict.
- Missing date/amount, invalid date, or unconvertible amount → `DroppedItem{item:"dividend"|"capital_gain"|"split"}`.
- Splits also feed adjustment (REQ-HIST-009).

implementations:
  - src/history/builder/actions.rs:extract_actions
  - src/history/builder/actions.rs:event_currency
  - src/history/builder/actions.rs:event_timestamp
  - src/history/builder/actions.rs:action_sort_key
  - src/history/builder.rs:action_default_currency
  - src/history/builder.rs:events_need_default_currency
  - src/history/builder.rs:is_missing_currency
  - src/history/wire.rs:Events

verifies:
  - tests/history/adjust.rs:history_auto_adjust_and_actions
  - tests/history/action_currency_synthetic.rs:action_currency_prefers_event_then_chart_currency
  - tests/history/action_currency_synthetic.rs:action_without_event_or_default_currency_is_omitted

<!-- id: REQ-HIST-010.A1 -->
- [x] A 2:1 split at ts 2000 and a 1.0 dividend at ts 3000 produce exactly two actions with the UTC dates of those timestamps.
<!-- id: REQ-HIST-010.A2 -->
- [x] A dividend with `currency:"EUR"` is EUR; a capital gain without currency inherits the chart currency USD; actions are ordered by date.
<!-- id: REQ-HIST-010.A3 -->
- [x] A dividend with no event currency, no chart currency, and a failing quote lookup (HTTP 500) is omitted and the call still succeeds.
<!-- id: REQ-HIST-010.A4 -->
- [ ] A dividend with missing `amount` is dropped with `DroppedItem{"dividend", MissingRequiredField{"amount"}}`. [needs review — no test backs this]

<!-- id: REQ-HIST-011 -->
## Split ratio normalization

- Wire `numerator`/`denominator` accept integers, decimals, numeric strings, or null (`de_opt_decimal_from_mixed`);
  unparseable values become `None` silently.
- If both components exist and normalize, they win; otherwise `splitRatio` is parsed (`a/b`, `a:b`, or a bare
  number meaning `a/1`, scientific notation accepted).
- Normalization scales both components to a common decimal scale, reduces by GCD, and requires each to fit a
  non-zero `u32`. Zero, negative, or overflowing ratios drop the split with
  `InvalidField{field:"splitRatio", details:"missing, zero, negative, non-finite, or too large"}`; the dropped split
  also does not affect adjustment.

implementations:
  - src/history/builder/actions.rs:normalize_split_event
  - src/history/builder/actions.rs:normalize_split_ratio
  - src/history/builder/actions.rs:parse_split_component
  - src/history/builder/actions.rs:normalize_split_pair
  - src/history/builder/actions.rs:split_component_parts
  - src/history/builder/actions.rs:SplitRatio
  - src/history/wire.rs:de_opt_decimal_from_mixed

verifies:
  - tests/history/fractional_splits.rs:history_tolerates_fractional_split_components
  - tests/history/fractional_splits.rs:history_preserves_split_precision_beyond_six_decimal_places
  - tests/history/fractional_splits.rs:history_skips_split_components_that_overflow_action_ratio

<!-- id: REQ-HIST-011.A1 -->
- [x] AXIA-P fixture split `1.262838` normalizes to `631419/500000`.
<!-- id: REQ-HIST-011.A2 -->
- [x] String components `"1.0000004"/"1"` normalize exactly to `2500001/2500000`.
<!-- id: REQ-HIST-011.A3 -->
- [x] Numerator `4294967296` (> u32::MAX) drops the split: no actions, candle closes stay 100.
<!-- id: REQ-HIST-011.A4 -->
- [ ] A split with only `splitRatio: "3:1"` (no numerator/denominator) yields `Split{3,1}`. [needs review — no test backs this]
<!-- id: REQ-HIST-011.A5 -->
- [ ] The dropped-split case emits the `splitRatio` `DroppedItem` warning and fails in strict mode. [needs review — overflow test does not assert diagnostics]

<!-- id: REQ-HIST-012 -->
## History metadata and instrument side effects

- `HistoryResponse.meta` is `Some(HistoryMeta{timezone: Option<Tz>, utc_offset_seconds: Option<i64>})` whenever
  `chart.meta` is present.
- Timezone: first non-blank of `exchangeTimezoneName`, then `timezone`, parsed as IANA `chrono_tz::Tz`. Each
  unparseable value emits `OmittedPresentField{path:"chart.meta.<field>", InvalidField}`; strict mode errors.
- `utc_offset_seconds` = `gmtoffset` verbatim.
- Side effects: an `Instrument` built from `instrumentType` (+ first parseable of `fullExchangeName`/`exchangeName`)
  is stored in the client side cache under the requested symbol and, if different, under `meta.symbol`;
  currency hints from meta are stored for the symbol. A malformed/unknown `instrumentType` is silently ignored
  (no diagnostic) and does not affect candles.
- `Ticker::get_history_metadata(Option<Range>)` exposes the meta (used in example 04).

implementations:
  - src/history/builder.rs:map_meta
  - src/history/builder.rs:parse_history_timezone
  - src/history/builder.rs:store_history_side_effects
  - src/history/builder.rs:history_instrument_from_meta
  - src/history/builder.rs:store_history_instrument
  - src/history/wire.rs:MetaNode

verifies:
  - tests/history/meta.rs:get_history_metadata_returns_timezone
  - tests/history/meta.rs:history_metadata_uses_exchange_timezone_name_without_warning
  - tests/history/meta.rs:malformed_history_timezone_is_reported_as_projection_loss
  - tests/history/meta.rs:malformed_history_instrument_type_does_not_abort_candles

<!-- id: REQ-HIST-012.A1 -->
- [x] Meta `{timezone:"EDT", exchangeTimezoneName:"America/New_York", gmtoffset:-14400}` maps to timezone `America/New_York`, offset `-14400`, with no diagnostics.
<!-- id: REQ-HIST-012.A2 -->
- [x] `timezone:"Not/A_Timezone"` yields `meta.timezone = None` plus an `OmittedPresentField{"chart.meta.timezone"}` warning; strict mode returns `DataQuality`.
<!-- id: REQ-HIST-012.A3 -->
- [x] `instrumentType:"!!!"` still returns the single candle (close 100.5) with no diagnostics.

<!-- id: REQ-DL-001 -->
## DownloadBuilder construction and configuration

- `DownloadBuilder::new(client: &YfClient) -> Self` (not `Clone`).
- Defaults: no symbols, `range = Some(Range::M6)`, `interval = Interval::D1`, `adjustment = DownloadAdjustment::Auto`,
  `include_prepost = false`, `include_actions = true`, `rounding = false`, `concurrency = DownloadConcurrency::DEFAULT (8)`.
- Setters: `symbols(IntoIterator<Item: Into<String>>)` (replaces), `add_symbol(impl Into<String>)` (appends),
  `range(Range)`, `between(DateTime<Utc>, DateTime<Utc>)` (mutually exclusive, last wins), `interval(Interval)`,
  `adjustment(DownloadAdjustment)`, shorthands `unadjusted()`, `auto_adjust()`, `back_adjust()`, `prepost(bool)`,
  `actions(bool)`, `rounding(bool)`, `concurrency(DownloadConcurrency)`, plus call-option setters
  (`cache_mode`, `retry_policy`, `data_quality`, `strict`) which are forwarded to every per-symbol `HistoryBuilder`.
- `pub enum DownloadAdjustment { None, #[default] Auto, Back }`.
- `pub struct DownloadConcurrency(usize)`; `DownloadConcurrency::new(0)` → `InvalidParams("download concurrency must be at least 1")`.
- `run() -> Result<DownloadResponse, YfError>` and `run_with_diagnostics() -> Result<YfResponse<DownloadResponse>, YfError>`.
- Output: `DownloadResponse { entries: Vec<DownloadEntry { instrument, history: HistoryResponse }> }`.

implementations:
  - src/download/mod.rs:DownloadBuilder.new
  - src/download/mod.rs:DownloadBuilder.symbols
  - src/download/mod.rs:DownloadBuilder.add_symbol
  - src/download/mod.rs:DownloadBuilder.range
  - src/download/mod.rs:DownloadBuilder.between
  - src/download/mod.rs:DownloadBuilder.interval
  - src/download/mod.rs:DownloadBuilder.adjustment
  - src/download/mod.rs:DownloadBuilder.unadjusted
  - src/download/mod.rs:DownloadBuilder.auto_adjust
  - src/download/mod.rs:DownloadBuilder.back_adjust
  - src/download/mod.rs:DownloadBuilder.prepost
  - src/download/mod.rs:DownloadBuilder.actions
  - src/download/mod.rs:DownloadBuilder.rounding
  - src/download/mod.rs:DownloadBuilder.concurrency
  - src/download/mod.rs:DownloadBuilder.build_history_for_symbol
  - src/download/mod.rs:DownloadConcurrency.new
  - src/download/mod.rs:DownloadAdjustment

verifies:
  - tests/download/offline.rs:download_multi_symbols_happy_path
  - tests/download/offline.rs:download_between_params_applied_to_all_symbols
  - tests/download/offline.rs:download_concurrency_rejects_zero

<!-- id: REQ-DL-001.A1 -->
- [x] Two symbols with `range(M6).interval(D1).auto_adjust().prepost(false).actions(true)` each issue `range=6mo&interval=1d&includePrePost=false&events=div|split|capitalGains` and both appear in `entries`.
<!-- id: REQ-DL-001.A2 -->
- [x] `between(2024-01-01, 2024-01-10)` sends identical `period1/period2` for every symbol and both return non-empty candles.
<!-- id: REQ-DL-001.A3 -->
- [x] `DownloadConcurrency::new(0)` returns `InvalidParams` mentioning "concurrency".
<!-- id: REQ-DL-001.A4 -->
- [ ] `add_symbol` appends to a list set by `symbols`, and `symbols` replaces any previous list. [needs review — no test backs this]

<!-- id: REQ-DL-002 -->
## Up-front validation before any network call

In `run_with_diagnostics`, in order: empty symbol list → `InvalidParams("no symbols specified")`; every symbol is
normalized (`normalize_symbols`) and the first invalid one fails the whole call with `InvalidParams("invalid symbol ...")`;
an explicit period with `start >= end` → `YfError::InvalidDates` (top-level, not per-symbol). Duplicate symbols are
**not** de-duplicated [needs review — unclear whether intentional].

implementations:
  - src/download/mod.rs:DownloadBuilder.run_with_diagnostics
  - src/download/mod.rs:DownloadBuilder.precompute_period_dt
  - src/core/client/urls.rs:normalize_symbols

verifies:
  - tests/download/offline.rs:download_rejects_invalid_symbol_before_request
  - tests/download/offline.rs:download_between_rejects_invalid_dates_as_top_level_error

<!-- id: REQ-DL-002.A1 -->
- [x] A 65-character symbol fails with `InvalidParams` containing "invalid symbol" without any request.
<!-- id: REQ-DL-002.A2 -->
- [x] `between(2024-01-10, 2024-01-01)` for two symbols fails with top-level `YfError::InvalidDates`.
<!-- id: REQ-DL-002.A3 -->
- [ ] An empty symbol list fails with `InvalidParams("no symbols specified")`. [needs review — no test backs this]

<!-- id: REQ-DL-003 -->
## Bounded concurrent fetching with input-order output

- Per-symbol `HistoryBuilder` futures run through `futures::stream::buffer_unordered(concurrency)`; at most
  `concurrency` chart requests are in flight.
- Results are re-sorted by original input index, so `entries` (and failure diagnostics) follow input order
  regardless of completion order.
- Each symbol's diagnostics are merged into the download context with the symbol as key prefix.
- Instruments come from each response (not the client side cache), so side-cache eviction cannot mix them up.

implementations:
  - src/download/mod.rs:DownloadBuilder.run_with_diagnostics
  - src/download/mod.rs:DownloadBuilder.process_joined_results

verifies:
  - tests/download/offline.rs:download_respects_configured_concurrency_limit
  - tests/download/offline.rs:strict_download_uses_per_response_instruments_after_side_cache_eviction

<!-- id: REQ-DL-003.A1 -->
- [x] With `concurrency(1)` and a 1 s delayed first symbol, the second symbol is not requested until the first completes; entries are `["AAPL","MSFT"]`.
<!-- id: REQ-DL-003.A2 -->
- [x] With `side_cache_max_entries(1)`, `concurrency(1)` and strict mode, both entries keep their typed instruments in input order.
<!-- id: REQ-DL-003.A3 -->
- [ ] Per-symbol warnings appear in download diagnostics with keys prefixed by the symbol. [needs review — no test backs this; exact prefix format lives in `with_key_prefix`]

<!-- id: REQ-DL-004 -->
## Partial failure semantics

- A per-symbol fetch error does not abort a best-effort download: the symbol is omitted from `entries` and a
  `DroppedItem{endpoint:"download", item:"download_entry", key: Some(symbol), reason: ProviderError{message:"history fetch failed: {err}"}}`
  is recorded. Successful symbols are returned.
- In strict mode the first such diagnostic (by input order) is returned as `YfError::DataQuality`, discarding successes.

implementations:
  - src/download/mod.rs:DownloadBuilder.run_with_diagnostics

verifies:
  - tests/download/offline.rs:best_effort_download_keeps_successes_when_symbol_fetch_fails

<!-- id: REQ-DL-004.A1 -->
- [x] `["AAPL","BROKEN"]` where BROKEN returns 404 yields one entry (AAPL) and a `download_entry` `ProviderError` warning keyed `BROKEN` whose message contains "history fetch failed: Not found at" and the chart URL.
<!-- id: REQ-DL-004.A2 -->
- [ ] The same scenario in strict mode returns `YfError::DataQuality`. [needs review — no test backs this]

<!-- id: REQ-DL-005 -->
## Download adjustment modes (None / Auto / Back)

- `None` (`unadjusted()`): per-symbol fetch with `auto_adjust(false)`; raw OHLC.
- `Auto` (default): per-symbol fetch with `auto_adjust(true)` (REQ-HIST-009).
- `Back` (`back_adjust()`): fetches adjusted history, then replaces each candle's `close` with `close_unadj` when it
  is present and finite; `price_basis` becomes `per_field(open, high, low, PriceBasis::raw())` keeping the fetched
  open/high/low bases. Only one HTTP request per symbol.

implementations:
  - src/download/mod.rs:DownloadAdjustment.fetches_adjusted_history
  - src/download/mod.rs:DownloadAdjustment.is_back_adjusted
  - src/download/mod.rs:DownloadBuilder.apply_back_adjust
  - src/download/mod.rs:DownloadBuilder.back_adjust_price_basis

verifies:
  - tests/download/back_adjust.rs:download_back_adjust_sets_close_to_raw
  - tests/download/offline.rs:download_back_adjust_offline

<!-- id: REQ-DL-005.A1 -->
- [x] Raw closes `[100,100]`, adjclose `[50,100]`: back-adjusted bar 0 has open 50 and close 100; bar 1 open/close 100; basis `per_field(adj, adj, adj, raw)`.
<!-- id: REQ-DL-005.A2 -->
- [x] On the AAPL fixture, back-adjusted O/H/L equal the auto-adjusted O/H/L bar-for-bar, candle counts match, and each mode makes exactly one request.
<!-- id: REQ-DL-005.A3 -->
- [ ] `unadjusted()` returns raw OHLC with basis `raw()`. [needs review — no test backs this]

<!-- id: REQ-DL-006 -->
## Price rounding with Yahoo `priceHint`

- Only when `rounding(true)` and `chart.meta.priceHint` is present (and fits `u32`): open/high/low/close are rounded
  to `min(priceHint, 28)` decimal places. Rounding happens in the provider's quote unit **before** minor-unit scaling
  (e.g. GBp rounded, then ÷100 to GBP) via `ResolvedCurrencyUnit::price_amount_rounded_at_provider_precision`;
  otherwise falls back to `Decimal::round_dp`.
- Without `priceHint`, prices are left unchanged. Rounding runs after back-adjustment. `close_unadj` is not rounded.
- Rounding mode (banker's vs half-up) is inherited from `rust_decimal` / paft [needs review — not pinned by tests].

implementations:
  - src/download/mod.rs:DownloadBuilder.apply_rounding_if_enabled
  - src/download/mod.rs:rounded_price

verifies:
  - tests/download/malformed_rounding.rs:download_drops_malformed_rows_and_rounds_valid_neighbors_to_price_hint
  - tests/download/malformed_rounding.rs:download_rounding_without_price_hint_leaves_prices_unchanged
  - tests/download/offline.rs:rounding_uses_recorded_price_hint
  - tests/download/offline.rs:rounding_uses_recorded_minor_unit_price_hint_before_scaling

<!-- id: REQ-DL-006.A1 -->
- [x] `priceHint: 3`, open `100.0014` → `100.001`, close `100.4996` → `100.500`; the null middle row is dropped (2 candles).
<!-- id: REQ-DL-006.A2 -->
- [x] No `priceHint`: `100.001/101.009/99.001/100.499` are returned unchanged.
<!-- id: REQ-DL-006.A3 -->
- [x] AAPL/MSFT fixtures (priceHint 2) produce OHLC with at most 2 decimals.
<!-- id: REQ-DL-006.A4 -->
- [x] TSCO.L (`GBp`) open equals `round(raw, priceHint) × 0.01`, and differs from the scale-first result.

<!-- id: REQ-DL-007 -->
## Download instrument resolution and untyped fallback

- Each entry's `instrument` is the one derived from that symbol's chart meta (REQ-HIST-012).
- If none could be derived, the entry is kept with `Instrument::from_symbol(symbol, AssetKind::Other("YAHOO_DOWNLOAD_UNTYPED"))`
  and a `RepairedData{endpoint:"download", item:"download_entry", key: symbol, repair:"used untyped Yahoo download instrument because chart.meta.instrumentType was missing"}`
  warning; in strict mode this is a `DataQuality` error that fails the whole download.

implementations:
  - src/download/mod.rs:resolve_download_instrument
  - src/download/mod.rs:untyped_download_asset_kind

verifies:
  - tests/download/offline.rs:best_effort_download_uses_untyped_instrument_without_instrument_metadata
  - tests/download/offline.rs:strict_download_errors_without_instrument_metadata

<!-- id: REQ-DL-007.A1 -->
- [x] Best-effort `["AAPL","NOKIND"]` returns 2 entries; NOKIND has kind code `YAHOO_DOWNLOAD_UNTYPED`, 1 candle, and the repair warning keyed `NOKIND`.
<!-- id: REQ-DL-007.A2 -->
- [x] Strict `["NOKIND"]` fails with `DataQuality(RepairedData{download_entry, key NOKIND})`.

<!-- id: HIST-NONGOALS -->
## Non-goals

<!-- id: HIST-NONGOALS-LIST -->
### Explicit signals in code

- Price-outlier repair (Python yfinance `repair=True`) is not performed; `download_preserves_suspicious_price_outliers` pins pass-through.
- Volume is never adjusted for splits (tests assert Yahoo volume is kept verbatim).
- Only `chart.result[0]` is consumed; multi-result chart responses are not supported.
- Back-adjustment and rounding are download-only; `HistoryBuilder` exposes neither.
- [fill in — any further non-goals are not derivable from code]

<!-- id: HIST-OPEN -->
## Open questions

1. Is the SplitAdjusted fallback correct against live Yahoo data, given that chart OHLC is generally already split-adjusted? (REQ-HIST-009)
2. Should action dates be computed in the exchange timezone (`meta.exchangeTimezoneName`) rather than UTC? (REQ-HIST-010)
3. Should `DownloadBuilder` de-duplicate symbols, and should `between()` validation happen at set time for both builders?
4. Should `HistoryService::fetch_full_history` accept/forward `CallOptions` (strict mode, cache, retry)?
5. Is the 99×365-day daily Max lookback (ignores leap days, ≈ 1927-11 today) intended to cover the oldest Yahoo series (e.g. `^GSPC` from 1927-12)?
6. Should a malformed (present but unparseable) `instrumentType` produce a diagnostic in `HistoryBuilder`, and should the download repair message distinguish "malformed" from "missing"?
7. Example 05 (`05_concurrent_requests.rs`) does not use history or download (it uses `Ticker::info`, search, fundamentals); was a different example intended for this scope?

<!-- id: HIST-REVIEW -->
## Code review findings (ranked by severity)

<!-- id: HIST-REVIEW-HIGH -->
### High

- **F1 — Possible double split adjustment in the split-only path.** `src/history/builder/adjust.rs:57-59`
  (`split_adjustment_factor`) and `src/history/builder/assemble.rs:138-152` divide pre-split bars by the cumulative
  split ratio whenever adjclose is absent **or sparse**. If Yahoo's raw chart OHLC is already split-adjusted (as is
  widely observed), this halves/thirds historical prices a second time, and the sparse-adjclose repair applies it to
  every row. Tests use synthetic unadjusted data only (`tests/history/adjust_from_splits_only.rs`,
  `tests/history/adjust.rs:sparse_adjclose_...`). [needs review — verify against a recorded live response containing a split and missing/sparse adjclose]
- **F2 — Back-adjust reports a raw Close basis even when Close was not replaced.** `src/download/mod.rs:156-176`:
  `apply_back_adjust` silently skips candles whose `close_unadj` is `None` or non-finite (leaving the adjusted close),
  but `back_adjust_price_basis` unconditionally labels close as `PriceBasis::raw()`. `close_unadj` can be `None`
  (`src/history/builder/assemble.rs:85-95`), so consumers can receive mislabeled data with no diagnostic.

<!-- id: HIST-REVIEW-MED -->
### Medium

- **F3 — Granularity mismatch is fatal in all modes with no fallback.** `src/history/builder/fetch.rs:146-149,153-172`:
  any mismatch (including legitimate Yahoo aliases other than `1h`/`60m`, e.g. a possible `1wk` vs `5d` or
  `3mo` coarsening near listing dates) aborts the whole call, and in `DownloadBuilder` turns into a dropped entry.
  Only one alias pair is recognised. [needs review — enumerate Yahoo's actual `dataGranularity` values]
- **F4 — Corporate-action dates use the UTC calendar date.** `src/history/builder/actions.rs:62,123,184`
  (`i64_to_date(ts)`): for exchanges ahead of UTC whose event timestamps fall before UTC midnight (e.g. ASX at
  10:00 AEDT = 23:00 UTC previous day) the reported ex-date can be one day early. `meta.exchangeTimezoneName` is
  available but unused. [needs review — confirm Yahoo event timestamp convention for non-US listings]
- **F5 — `close == 0` or a single missing adjclose silently downgrades the whole series.**
  `src/history/builder/adjust.rs:50-55` + `src/history/builder/assemble.rs:129-152`: one row with a zero close makes the
  series "sparse", discarding all provider (dividend) adjustments for every other row.
- **F6 — Duplicate download symbols are fetched and returned twice.** `src/download/mod.rs:379,385-398`:
  `normalize_symbols` does not de-duplicate; duplicate entries and requests result (mitigated only if caching is on).

<!-- id: HIST-REVIEW-LOW -->
### Low / design smells

- **F7 — `HistoryService` drops call options.** `src/history/mod.rs:20-53` builds a `HistoryBuilder` with default
  `CallOptions`; trait callers cannot request strict mode, cache mode, or retry policy, and diagnostics are discarded.
- **F8 — Download instrument repair message is misleading.** `src/download/mod.rs:457-461` always says
  "instrumentType was missing", but `history_instrument_from_meta` (`src/history/builder.rs:541-564`) also returns
  `None` for a present-but-unparseable type or an `Instrument` construction error; and `HistoryBuilder` emits no
  diagnostic at all for that loss (`tests/history/meta.rs:malformed_history_instrument_type_does_not_abort_candles` asserts empty diagnostics).
- **F9 — Duplicated logic.** `action_sort_key` is duplicated (`src/history/builder.rs:324-331` and
  `src/history/builder/actions.rs:226-233`) and actions are sorted twice (`actions.rs:220`, `builder.rs:279`); period
  validation is duplicated (`src/download/mod.rs:111-129` vs `src/history/builder/fetch.rs:52-58`, plus
  `src/history/mod.rs:37-48`); the symbol is normalized twice (`builder.rs:193`, `fetch.rs:98`).
- **F10 — Rounding leaves `close_unadj` unrounded.** `src/download/mod.rs:192-197`: after rounding, `ohlc.close`
  (raw under back-adjust) and `close_unadj` can differ only by rounding, which is surprising for equality checks.
- **F11 — Max-range lookback ignores leap days.** `src/history/builder/fetch.rs:10` uses `99 * 365` days (~24 days
  short of 99 calendar years). Weekly Max buckets align to the Unix epoch (Thursday) via `timestamp_ceiling` (`fetch.rs:64,81-90`).
- **F12 — Wire decoding silently nulls unparseable split components.** `src/history/wire.rs:129-137` swallows parse
  errors (`.ok()`), so a malformed numerator falls back to `splitRatio` without any diagnostic.
- **F13 — `between()` truncates sub-second precision and is unvalidated until fetch/run.**
  `src/history/builder.rs:91-100`, `src/download/mod.rs:288-296`.

<!-- id: HIST-REVIEW-TESTS -->
### Missing tests

- `actions(false)` omitting `events`; `prepost(true)` sending `includePrePost=true`.
- `auto_adjust(false)` / `DownloadAdjustment::None` raw path and `OhlcPriceBasis::raw()`.
- `chart.error` → `YfError::Api`; empty `result`; missing `quote`.
- Diagnostics for dropped splits/dividends/capital gains (overflow test asserts no diagnostics).
- Strict-mode download with a failing symbol; empty symbol list; duplicate symbols.
- Back-adjust when `close_unadj` is missing (F2).
- Split on the same timestamp as a bar; `splitRatio`-only events.
- `HistoryService::fetch_full_history` has no behavioural test (only a type-name check in `tests/core/public_validation.rs`).
- Live tests (`tests/history/live.rs`, `tests/download/live.rs`) are `#[ignore]`d record/smoke tests only.

<!-- id: HIST-NOTES -->
## Reverse-engineering notes

- **High confidence:** request construction, Max-range windowing, granularity validation, malformed-row dropping,
  adjustment plan selection, split normalization, download concurrency/order/failure/instrument behaviour — all read
  directly from code and pinned by offline httpmock tests or unit tests.
- **Medium confidence:** currency fallback behaviour (delegated to `src/core/currency_resolver`, outside this scope);
  diagnostic key prefixing in downloads (`YfDiagnostics::with_key_prefix`, outside scope); exact `paft` semantics of
  `PriceBasis`, `Interval::seconds()` and `Interval::code()` for every variant (paft sources were not available locally).
- **Low confidence / intent unknown:** whether the split-only fallback reflects Yahoo's real data model (F1); why
  the 99-year/730-day/60-day/8-day windows were chosen beyond "match Python yfinance" [purpose unclear beyond parity].
- Tests were not executed: local rustc 1.88 is below the crate MSRV 1.91 (paft 0.9 requires 1.90). `[x]` marks mean
  "a matching test exists and its assertions were read", not "observed passing".
- Examples: 04 demonstrates `Ticker::actions`, a weekly back-adjusted + rounded `DownloadBuilder` over a 30-day
  `between` window, and `get_history_metadata`; 14 converts `Ticker::history` output to Polars (feature `dataframe`);
  05 does not exercise history/download.
- Unit tests cited from `src/history/builder/fetch.rs` and `src/history/builder/assemble.rs` live in in-file
  `#[cfg(test)] mod tests`.
