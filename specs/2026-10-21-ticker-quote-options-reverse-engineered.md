---
id: TICK-DOC
title: Ticker facade, batch quotes, and option chains (reverse-engineered)
owner: unassigned
priority: P1
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/ticker/mod.rs
  - src/ticker/quote.rs
  - src/ticker/info.rs
  - src/ticker/isin.rs
  - src/ticker/options.rs
  - src/ticker/model.rs
  - src/quote/mod.rs
  - src/core/quotes.rs
  - src/core/models.rs
  - src/core/client/urls.rs
  - src/core/client/retry.rs
  - tests/ticker.rs
  - tests/ticker/quote.rs
  - tests/ticker/fast_info.rs
  - tests/ticker/offline.rs
  - tests/ticker/data_quality.rs
  - tests/ticker/info_offline.rs
  - tests/ticker/isin_offline.rs
  - tests/ticker/key_statistics.rs
  - tests/ticker/options.rs
  - tests/ticker/options_expiry_from_url_fallback.rs
  - tests/ticker/actions.rs
  - tests/ticker/history_convenience.rs
  - tests/ticker/live.rs
  - tests/quotes.rs
  - tests/quotes/offline.rs
  - tests/quotes/names.rs
  - tests/quotes/new_fields.rs
  - tests/quotes/error_synthetic.rs
  - tests/quotes/retry_synthetic.rs
  - tests/quotes_status_mapping.rs
  - tests/common.rs
  - examples/01_basic_usage.rs
  - examples/10_convenience_methods.rs
---

<!-- id: TICK-DOC -->
# Ticker facade, batch quotes, and option chains (reverse-engineered)

Intent: `Ticker` is the high-level, per-symbol entry point of the crate, documented as similar to Python yfinance's `Ticker` object (src/ticker/mod.rs:38-42). It carries a cloned `YfClient`, a symbol, and per-call `CallOptions` (cache mode, retry override, data-quality policy), and exposes quote, fast-info, key-statistics, aggregate `info`, ISIN lookup, option expirations and chains, and thin delegations to the other builders (history, news, holders, analysis, ESG, fundamentals). `QuotesBuilder` / `quotes()` fetch Yahoo v7 quotes for many symbols at once. Every data-returning call has a `*_with_diagnostics` variant that returns `YfResponse<T>` (data plus projection warnings). Under `DataQuality::Strict`, any projection warning becomes an error. Business motivation beyond "yfinance parity" is not stated in code `[purpose unclear]`.

Verification caveat: the local toolchain is rustc 1.88 and the crate needs `rust-version = "1.91"`, so the tests were **not executed** while writing this spec. `- [x]` means "backed by an existing, non-`#[ignore]` test whose assertions match the criterion". `[needs review — tests not executed in this session]` applies to all `[x]` items.

<!-- id: REQ-TICK-001 -->
## REQ-TICK-001 Ticker construction and per-call option propagation

`Ticker::new(client: &YfClient, symbol: impl Into<String>) -> Ticker` clones the client and starts with `CallOptions::default()`. The setters consume and return `self`:
- `cache_mode(self, CacheMode) -> Self` (const)
- `retry_policy(self, Option<RetryConfig>) -> Self`
- `data_quality(self, DataQuality) -> Self` (const)
- `strict(self) -> Self`, which is `data_quality(DataQuality::Strict)`

`CacheMode::Default` means each endpoint applies its own policy. Quotes and options bypass the cache. Quote-summary data uses it when client caching is enabled (src/core/client/retry.rs:110-117). The private `*_builder()` helpers (history, news, holders, analysis, esg, fundamentals) forward cache mode, data quality, and the retry override to the builder they create. The symbol is not validated at construction. It is normalized lazily per call through `normalize_symbol`, which allows only ASCII alphanumerics and `. - _ = ^` (src/core/client/urls.rs:31-65).

implementations:
  - src/ticker/mod.rs:Ticker.new
  - src/ticker/mod.rs:Ticker.cache_mode
  - src/ticker/mod.rs:Ticker.retry_policy
  - src/ticker/mod.rs:Ticker.data_quality
  - src/ticker/mod.rs:Ticker.strict
  - src/ticker/mod.rs:Ticker.history_builder
  - src/ticker/mod.rs:Ticker.news_builder

verifies:
  - tests/ticker/data_quality.rs:ticker_data_quality_strict_rejects_direct_quote_projection_warnings
  - tests/ticker/data_quality.rs:ticker_strict_propagates_to_builder_backed_convenience_methods
  - tests/ticker/info_offline.rs:ticker_info_profile_respects_ticker_cache_bypass
  - tests/ticker/info_offline.rs:ticker_info_profile_respects_ticker_retry_policy
  - tests/ticker/history_convenience.rs:ticker_history_builder_respects_ticker_retry_policy
  - tests/ticker/actions.rs:ticker_actions_respect_ticker_cache_bypass

<!-- id: REQ-TICK-001.A1 -->
- [x] `Ticker::new(..).data_quality(DataQuality::Strict).quote()` and `.strict().quote_with_diagnostics()` both return `YfError::DataQuality` when the quote has an invalid optional field (`fullExchangeName: "!!!"`).
<!-- id: REQ-TICK-001.A2 -->
- [x] `.strict()` on a ticker carries over to builder-backed convenience methods: `Ticker::strict().news()` returns `YfError::DataQuality` for an article with no publication date.
<!-- id: REQ-TICK-001.A3 -->
- [x] `cache_mode(CacheMode::Bypass)` makes each `info()` call hit the quoteSummary endpoint (2 calls for 2 invocations), even with a client cache TTL set.
<!-- id: REQ-TICK-001.A4 -->
- [x] `retry_policy(Some(cfg))` replaces the client's retry policy. With `max_retries = 2` against a 503 response, the info quoteSummary endpoint is called 3 times.
<!-- id: REQ-TICK-001.A5 -->
- [ ] An invalid symbol (e.g. containing a space or `/`) causes every network call to fail with `YfError::InvalidParams` before any request is sent. `[needs review — no test backs this]`

<!-- id: REQ-TICK-002 -->
## REQ-TICK-002 Single-symbol quote

`Ticker::quote(&self) -> Result<Quote, YfError>` and `Ticker::quote_with_diagnostics(&self) -> Result<YfResponse<Quote>, YfError>` fetch Yahoo `/v7/finance/quote?symbols=<SYM>` and project the first result node into `paft::market::quote::Quote`. The fields are `instrument`, `name`, `currency`, `price`, `bid`, `ask`, `previous_close`, `day_volume`, `market_state`, and `as_of`. Diagnostics use the endpoint label `"quote"`. Each malformed optional field is omitted, and an `OmittedPresentField` warning is recorded with the symbol as key. A missing or invalid `quoteType` or currency is a hard error (`MissingData` / `InvalidData`) in every data-quality mode. The single quote path takes the first valid node and does not check that its symbol matches the request (src/core/quotes.rs:1591-1597) `[needs review — possibly intentional alias handling, e.g. BRK.B vs BRK-B]`.

implementations:
  - src/ticker/mod.rs:Ticker.quote
  - src/ticker/mod.rs:Ticker.quote_with_diagnostics
  - src/ticker/quote.rs:fetch_quote
  - src/ticker/quote.rs:fetch_quote_with_diagnostics
  - src/core/quotes.rs:fetch_v7_quote_values
  - src/core/quotes.rs:quote_nodes_from_values_with_context
  - src/core/quotes.rs:required_quote_node_from_nodes
  - src/core/quotes.rs:V7QuoteNode.to_quote_with_context

verifies:
  - tests/ticker/quote.rs:quote_v7_happy_path
  - tests/ticker/quote.rs:quote_with_diagnostics_errors_on_invalid_required_currency
  - tests/ticker/quote.rs:quote_with_diagnostics_reports_invalid_optional_provider_fields
  - tests/ticker/quote.rs:quote_with_diagnostics_omits_malformed_optional_price
  - tests/ticker/quote.rs:quote_v7_usd_crypto_price_keeps_provider_precision
  - tests/ticker/quote.rs:quote_v7_missing_quote_type_returns_error
  - tests/ticker/offline.rs:offline_quote_uses_recorded_fixture

<!-- id: REQ-TICK-002.A1 -->
- [x] For a body with `symbol=AAPL, quoteType=EQUITY, regularMarketPrice=190.25, regularMarketPreviousClose=189.50, currency=USD, fullExchangeName=NasdaqGS, marketState=REGULAR`, the result has symbol `AAPL`, exchange `NASDAQ`, market state `REGULAR`, price 190.25, and previous close 189.50.
<!-- id: REQ-TICK-002.A2 -->
- [x] `currency: "!!!"` makes `quote_with_diagnostics()` return `YfError::InvalidData` with a message containing `invalid quote currency: invalid currency code !!!`.
<!-- id: REQ-TICK-002.A3 -->
- [x] An invalid `fullExchangeName`, `marketState`, or `regularMarketTime` (i64::MAX) gives `None` for exchange, market_state, and as_of. Each produces `YfWarning::OmittedPresentField { endpoint: "quote", path, key: Some("AAPL"), reason: InvalidField{..} }`.
<!-- id: REQ-TICK-002.A4 -->
- [x] A non-numeric `regularMarketPrice` gives `price = None` while `previous_close` is still populated, with an `OmittedPresentField` warning on path `regularMarketPrice`.
<!-- id: REQ-TICK-002.A5 -->
- [x] A USD crypto price of `0.612345` keeps its full provider precision (`as_decimal() == "0.612345"`).
<!-- id: REQ-TICK-002.A6 -->
- [x] A node without `quoteType` makes `quote()` return `YfError::MissingData` with a message mentioning `quoteType`.
<!-- id: REQ-TICK-002.A7 -->
- [ ] An empty `result` array makes `quote()` return `YfError::MissingData("no valid quote result found for symbol <SYM>")`. `[needs review — no test backs this]`

<!-- id: REQ-TICK-003 -->
## REQ-TICK-003 Fast info (snapshot + moving averages)

`Ticker::fast_info(&self) -> Result<FastInfo, YfError>` and `fast_info_with_diagnostics` use the same single v7 quote request as REQ-TICK-002. They return `FastInfo { snapshot: paft::aggregates::Snapshot, moving_averages: MovingAverages { fifty_day: Option<Price>, two_hundred_day: Option<Price> } }`. The diagnostics endpoint label is `"fast_info"`. The snapshot includes open, day high/low, volume, previous close, last, currency, instrument/exchange, and market state. When `regularMarketPrice` is missing, `snapshot.last` stays `None`. The library does **not** derive `last` from previous close (see Code review findings F-05).

implementations:
  - src/ticker/mod.rs:Ticker.fast_info
  - src/ticker/mod.rs:Ticker.fast_info_with_diagnostics
  - src/ticker/quote.rs:fetch_fast_info
  - src/ticker/quote.rs:fetch_fast_info_with_diagnostics
  - src/core/quotes.rs:V7QuoteNode.to_fast_info_with_context
  - src/core/quotes.rs:V7QuoteNode.to_snapshot_with_context
  - src/core/quotes.rs:V7QuoteNode.to_moving_averages_with_context

verifies:
  - tests/ticker/fast_info.rs:fast_info_uses_previous_close_when_price_missing
  - tests/ticker/fast_info.rs:fast_info_omits_malformed_optional_moving_average
  - tests/ticker/fast_info.rs:fast_info_with_diagnostics_errors_without_required_currency
  - tests/ticker/fast_info.rs:fast_info_maps_snapshot_session_fields_from_v7_quote
  - tests/ticker/quote.rs:fast_info_derives_last_price

<!-- id: REQ-TICK-003.A1 -->
- [x] When `regularMarketPrice` is absent or null, `snapshot.last` is `None` and `snapshot.previous_close` equals `regularMarketPreviousClose`.
<!-- id: REQ-TICK-003.A2 -->
- [x] A non-numeric `fiftyDayAverage` gives `moving_averages.fifty_day = None`, keeps `two_hundred_day`, and records an `OmittedPresentField { endpoint: "fast_info", path: "fiftyDayAverage" }` warning. Under `.strict()` the same body returns `YfError::DataQuality`.
<!-- id: REQ-TICK-003.A3 -->
- [x] A missing `currency` makes `fast_info_with_diagnostics()` return `YfError::InvalidData` containing `invalid snapshot currency: currency unresolved`.
<!-- id: REQ-TICK-003.A4 -->
- [x] Against the recorded AAPL fixture, open, day high/low, volume, and the 50/200-day averages match the raw v7 fields within 0.01. With the `dataframe` feature, `snapshot.to_dataframe()` has height 1.

<!-- id: REQ-TICK-004 -->
## REQ-TICK-004 Key statistics (v7 quote merged with quoteSummary)

`Ticker::key_statistics(&self) -> Result<KeyStatistics, YfError>` and `key_statistics_with_diagnostics` run two requests concurrently (`tokio::join!`): the v7 quote, and quoteSummary with modules `summaryDetail,defaultKeyStatistics` (src/core/quotes.rs:34). Fields come from v7 first. `merge_key_statistics` fills only `None` fields from quoteSummary: market_cap, shares_outstanding, eps/pe TTM, forward dividend per share, trailing/forward dividend yield, ex-dividend date, 52-week high/low, 3-month average volume, and beta (src/core/quotes.rs:1288-1333). A quoteSummary failure is suppressed and recorded as a `suppressed_error` diagnostic on module `quote_summary_key_statistics` (it is an error under Strict). A v7 failure is fatal. An empty v7 result returns `MissingData`. If v7 returned nodes but none could be parsed, the result is `KeyStatistics::default()`, and in best-effort mode this succeeds (see F-03). Diagnostics endpoint label: `"key_statistics"`. Market cap is expressed in major currency units even when the quote currency is a minor unit such as GBp.

implementations:
  - src/ticker/mod.rs:Ticker.key_statistics
  - src/ticker/mod.rs:Ticker.key_statistics_with_diagnostics
  - src/ticker/quote.rs:fetch_key_statistics
  - src/ticker/quote.rs:fetch_key_statistics_with_diagnostics
  - src/core/quotes.rs:fetch_quote_summary_key_statistics
  - src/core/quotes.rs:merge_key_statistics
  - src/core/quotes.rs:V7QuoteNode.to_key_statistics_with_context

verifies:
  - tests/ticker/key_statistics.rs:key_statistics_market_cap_uses_major_units_for_minor_unit_quote_currency
  - tests/ticker/key_statistics.rs:key_statistics_with_diagnostics_reports_invalid_financial_currency
  - tests/ticker/key_statistics.rs:key_statistics_with_diagnostics_omits_malformed_v7_market_cap
  - tests/ticker/key_statistics.rs:key_statistics_market_cap_preserves_large_integer_precision
  - tests/ticker/key_statistics.rs:key_statistics_v7_dividend_yield_units_are_fixture_locked
  - tests/ticker/key_statistics.rs:aapl_dividend_yield_conventions_reconcile_across_recorded_paths
  - tests/ticker/key_statistics.rs:key_statistics_recorded_v7_currency_units_are_field_scoped
  - tests/ticker/key_statistics.rs:key_statistics_recorded_quote_summary_currency_units_are_field_scoped
  - tests/ticker/key_statistics.rs:key_statistics_merge_v7_quote_and_quote_summary_fixtures
  - tests/ticker/key_statistics.rs:key_statistics_backfills_recorded_quote_summary_when_v7_omits_fields

<!-- id: REQ-TICK-004.A1 -->
- [x] For a `GBp`-quoted symbol (TSCO.L), market cap is reported in major units (GBP).
<!-- id: REQ-TICK-004.A2 -->
- [x] When v7 and quoteSummary both supply a field, the v7 value wins. When v7 omits it, the quoteSummary value fills the gap (MSFT recorded fixtures).
<!-- id: REQ-TICK-004.A3 -->
- [x] A malformed v7 `marketCap` is omitted with a diagnostic. A large-integer market cap keeps full integer precision.
<!-- id: REQ-TICK-004.A4 -->
- [x] Dividend-yield unit conventions (fraction vs percent points) match the recorded fixtures across the v7 and quoteSummary paths.
<!-- id: REQ-TICK-004.A5 -->
- [ ] If the quoteSummary request fails, `key_statistics()` still returns the v7-derived statistics in best-effort mode and fails under Strict. `[needs review — no test backs this]`

<!-- id: REQ-TICK-005 -->
## REQ-TICK-005 Aggregate `info`

`Ticker::info(&self) -> Result<Info, YfError>`, `info_with_diagnostics(&self) -> Result<YfResponse<Info>, YfError>`, and `info_strict(&self) -> Result<Info, YfError>` (the same call with options forced to Strict). `Info` is `#[non_exhaustive]` with these fields: `snapshot: Snapshot`, `moving_averages: MovingAverages`, `key_statistics: KeyStatistics`, `profile: Option<Profile>`, `calendar: Option<Calendar>`, `price_target: Option<PriceTarget>`, and `recommendation_summary: Option<RecommendationSummary>` (src/ticker/model.rs:17-33).

The symbol is normalized, and two requests run concurrently: one v7 quote (required) and one quoteSummary request with modules `summaryDetail,defaultKeyStatistics,assetProfile,quoteType,fundProfile,financialData,recommendationTrend,calendarEvents` (src/ticker/info.rs:10). Each optional part (key statistics, profile, analysis, calendar) is projected separately:
- A part that fails becomes `None`, and a `suppressed_error` diagnostic is recorded, which is fatal under Strict.
- A part whose diagnostics contain `ProviderFeatureUnavailable` for its module (`assetProfile`, `financialData`, `recommendationTrend`, `calendarEvents`, or a dotted sub-path of one) also becomes `None`.

Key statistics and moving averages from v7 are backfilled from quoteSummary. When `calendarEvents` is missing, `calendar` falls back to the v7 quote's dividend date. ESG is not fetched.

implementations:
  - src/ticker/mod.rs:Ticker.info
  - src/ticker/mod.rs:Ticker.info_with_diagnostics
  - src/ticker/mod.rs:Ticker.info_strict
  - src/ticker/info.rs:fetch_info
  - src/ticker/info.rs:fetch_info_with_diagnostics
  - src/ticker/info.rs:fetch_info_parts
  - src/ticker/info.rs:project_info_quote_summary_parts
  - src/ticker/info.rs:project_info_quote_summary_body
  - src/ticker/info.rs:log_optional_response_async
  - src/ticker/info.rs:diagnostics_has_unavailable_feature
  - src/ticker/model.rs:Info
  - src/core/quotes.rs:merge_moving_averages
  - src/core/quotes.rs:V7QuoteNode.calendar_fallback_with_context

verifies:
  - tests/ticker/info_offline.rs:offline_info_uses_recorded_fixtures
  - tests/ticker/info_offline.rs:ticker_info_missing_optional_quote_summary_modules_are_none
  - tests/ticker/info_offline.rs:ticker_info_backfills_moving_averages_from_summary_detail
  - tests/ticker/info_offline.rs:ticker_info_synthetic_etf_missing_legal_type_keeps_profile
  - tests/ticker/info_offline.rs:info_with_diagnostics_does_not_fetch_dead_esg_module
  - tests/ticker/info_offline.rs:ticker_info_profile_respects_ticker_retry_policy

<!-- id: REQ-TICK-005.A1 -->
- [x] With recorded MSFT fixtures, `info()` makes exactly 1 v7 quote call and 1 combined quoteSummary call, populates `profile`, and makes 0 ESG calls.
<!-- id: REQ-TICK-005.A2 -->
- [x] When `financialData`, `recommendationTrend`, and `calendarEvents` are absent, `price_target` and `recommendation_summary` are `None`. `ProviderFeatureUnavailable` warnings are emitted for each of the three modules, and `calendar.dividend_payment_date` is still filled from the v7 quote.
<!-- id: REQ-TICK-005.A3 -->
- [x] When v7 has no moving averages, they are backfilled from `summaryDetail` (fifty_day = 190.25 in the synthetic body).
<!-- id: REQ-TICK-005.A4 -->
- [x] For an ETF profile with no legal type, the `profile` is still `Some(Fund)` with kind `ETF`.
<!-- id: REQ-TICK-005.A5 -->
- [x] A quoteSummary 503 (after retries) does not fail `info()`. `profile` is `None`.
<!-- id: REQ-TICK-005.A6 -->
- [ ] `info_strict()` fails when any optional submodule is unavailable or fails to project. `[needs review — no test backs this]`
<!-- id: REQ-TICK-005.A7 -->
- [ ] A v7 quote failure fails `info()` even when quoteSummary succeeds. `[needs review — no test backs this]`

<!-- id: REQ-TICK-006 -->
## REQ-TICK-006 ISIN lookup

`Ticker::isin(&self) -> Result<Option<String>, YfError>`. The symbol is normalized, and symbols containing `^` (indices) return `Ok(None)` without a network call. Otherwise the call is `GET <base_insider_search>?max_results=5&query=<SYM>` (a Business Insider suggest endpoint, documented as mimicking Python yfinance). It uses only the ticker's retry override and ignores cache mode and data quality. A non-2xx status maps to a typed error. Two response shapes are parsed:
1. JSONP `mmSuggestDeliver(args...)`. A restricted JS-literal parser handles strings, `new Array(...)`, `[..]`, numbers, `true/false/null`, comments, and trailing commas. Any other expression or callback name makes the whole body unparseable. The row's symbol hint is the first pipe part of `Keywords` (or the second part of `IDs`). Matching uses `normalize_sym`: trim, lowercase, and map `-`, `:`, and whitespace to `.`. The ISIN is the first pipe part of `Keywords` that passes `paft::domain::Isin` check-digit validation.
2. A JSON array of `{symbol|Symbol, isin|ISIN, value|Value}` rows. A row matches if its symbol matches (rows without a symbol are accepted only when the array has exactly one row), or if the first pipe part of `value` matches.

If nothing matches, the result is `Ok(None)`. There is no `_with_diagnostics` variant.

implementations:
  - src/ticker/mod.rs:Ticker.isin
  - src/ticker/isin.rs:fetch_isin
  - src/ticker/isin.rs:fetch_isin_body
  - src/ticker/isin.rs:parse_business_insider_suggest
  - src/ticker/isin.rs:parse_json_suggest_rows
  - src/ticker/isin.rs:InsiderSuggestRow.isin_for_symbol
  - src/ticker/isin.rs:normalize_sym
  - src/ticker/isin.rs:validated_isin
  - src/ticker/isin.rs:JsDataParser.parse_mm_suggest_deliver_args

verifies:
  - tests/ticker/isin_offline.rs:offline_isin_happy_path
  - tests/ticker/isin_offline.rs:offline_isin_rejects_invalid_check_digit
  - tests/ticker/isin_offline.rs:offline_isin_business_insider_rows_require_symbol_match
  - tests/ticker/isin_offline.rs:offline_isin_business_insider_jsonp_uses_json_string_decoding
  - tests/ticker/isin_offline.rs:offline_isin_business_insider_jsonp_accepts_js_syntax_noise
  - tests/ticker/isin_offline.rs:offline_isin_business_insider_jsonp_rejects_executable_expressions
  - tests/ticker/isin_offline.rs:offline_isin_business_insider_jsonp_requires_expected_callback
  - tests/ticker/isin_offline.rs:offline_isin_json_array_requires_symbol_match
  - tests/ticker/isin_offline.rs:offline_isin_non_success_statuses_are_errors
  - tests/ticker/isin_offline.rs:offline_isin_does_not_match_base_symbol_for_suffix_qualified_query
  - tests/ticker/isin_offline.rs:offline_isin_business_insider_rows_preserve_exchange_suffix
  - tests/ticker/isin_offline.rs:offline_isin_accepts_exact_suffix_qualified_symbol

<!-- id: REQ-TICK-006.A1 -->
- [x] The recorded AAPL fixture yields `Some("US0378331005")`.
<!-- id: REQ-TICK-006.A2 -->
- [x] An ISIN with a bad check digit (`US0378331006`) yields `None`.
<!-- id: REQ-TICK-006.A3 -->
- [x] Rows for another symbol (MSFT when AAPL was queried) yield `None` in both the JSONP and JSON-array shapes.
<!-- id: REQ-TICK-006.A4 -->
- [x] JS string escapes are decoded (`AAP\u004c` → `AAPL`). Whitespace, single quotes, and trailing commas are accepted. A function-call argument (`makeRows()`) or an unexpected callback name yields `None`.
<!-- id: REQ-TICK-006.A5 -->
- [x] HTTP 404 maps to `NotFound`, 429 to `RateLimited`, 500 to `ServerError{500}`, and 418 to `Status{418}`.
<!-- id: REQ-TICK-006.A6 -->
- [x] Exchange suffixes are significant: `VOD.L` does not match a row for `VOD`, and does match a row for `VOD.L`.
<!-- id: REQ-TICK-006.A7 -->
- [ ] Symbols containing `^` (e.g. `^GSPC`) return `Ok(None)` with no HTTP request. `[needs review — no test backs this]`

<!-- id: REQ-TICK-007 -->
## REQ-TICK-007 Delegating convenience methods

The `Ticker` methods below are thin wrappers that build the matching builder with the ticker's options. Each `X()` returns `X_with_diagnostics().await?.into_data()`. Their projection behaviour belongs to the specs for those modules (out of scope here). Defaults set in this file:
- `history(range, interval, prepost)` forces `auto_adjust(true).actions(true)`. `range` / `interval` are applied only when `Some`. The docs claim defaults of 6mo / 1d, which come from `HistoryBuilder` `[needs review — default not set in this file]`.
- `actions(range)` defaults to `Range::Max` and sorts actions by date. Unknown `Action` variants sort last (`action_sort_key`).
- `get_history_metadata(range)` returns `HistoryMeta` from a full history fetch.
- holders (`major_holders`, `institutional_holders`, `mutual_fund_holders`, `insider_transactions`, `insider_roster_holders`, `net_share_purchase_activity`), analysis (`recommendations`, `recommendations_summary`, `upgrades_downgrades`, `analyst_price_target(Option<Currency>)`, `earnings_trend(Option<Currency>)`), `sustainability`, `profile`, fundamentals (`income_stmt`, `balance_sheet`, `cashflow` plus `quarterly_*`, `earnings`, `calendar`, `shares`, `shares_between`, `quarterly_shares`, `quarterly_shares_between`).

implementations:
  - src/ticker/mod.rs:Ticker.history
  - src/ticker/mod.rs:Ticker.history_with_diagnostics
  - src/ticker/mod.rs:Ticker.actions_with_diagnostics
  - src/ticker/mod.rs:Ticker.get_history_metadata_with_diagnostics
  - src/ticker/mod.rs:Ticker.profile_with_diagnostics
  - src/ticker/mod.rs:Ticker.sustainability
  - src/ticker/mod.rs:Ticker.income_stmt_with_diagnostics
  - src/ticker/mod.rs:Ticker.shares_between_with_diagnostics
  - src/ticker/mod.rs:action_sort_key

verifies:
  - tests/ticker/history_convenience.rs:ticker_history_convenience_builds_expected_query
  - tests/ticker/actions.rs:ticker_actions_include_dividends_and_splits
  - tests/ticker/actions.rs:ticker_actions_skip_invalid_amounts_and_keep_valid_siblings
  - tests/ticker/capital_gains.rs:offline_capital_gains_from_history
  - tests/ticker/shares.rs:offline_shares_between_rejects_inverted_window
  - tests/ticker/profile.rs:offline_profile_uses_recorded_fixture

<!-- id: REQ-TICK-007.A1 -->
- [x] `history(..)` sends a chart query that includes the dividend/split events flags and the ticker's range and interval.
<!-- id: REQ-TICK-007.A2 -->
- [x] `actions(..)` returns dividends, splits, and capital gains in date order and skips invalid amounts while keeping valid siblings.
<!-- id: REQ-TICK-007.A3 -->
- [x] `shares_between(start, end)` with `start > end` is rejected.

<!-- id: REQ-QUOTE-001 -->
## REQ-QUOTE-001 Batch quote API surface

Entry points:
- `pub async fn quotes<I, S>(client: &YfClient, symbols: I) -> Result<Vec<Quote>, YfError>`
- `quotes_with_diagnostics(..) -> Result<YfResponse<Vec<Quote>>, YfError>`
- `QuotesBuilder::new(&YfClient)` with `.symbols(iter)` (replaces the list), `.add_symbol(s)` (appends), the call-option setters from `impl_call_option_setters!` (`cache_mode`, `retry_policy`, `data_quality`, `strict`), `.fetch()`, and `.fetch_with_diagnostics()`

An empty symbol list returns `YfError::InvalidParams("symbols list cannot be empty")` before any I/O. Each symbol goes through `normalize_symbol`, and one invalid symbol fails the whole batch with `InvalidParams`. Diagnostics endpoint label: `"quotes"`. Duplicate symbols are not deduplicated `[needs review — unclear if intended]`.

implementations:
  - src/quote/mod.rs:quotes
  - src/quote/mod.rs:quotes_with_diagnostics
  - src/quote/mod.rs:QuotesBuilder.new
  - src/quote/mod.rs:QuotesBuilder.symbols
  - src/quote/mod.rs:QuotesBuilder.add_symbol
  - src/quote/mod.rs:QuotesBuilder.fetch
  - src/quote/mod.rs:QuotesBuilder.fetch_with_diagnostics

verifies:
  - tests/quotes/offline.rs:offline_multi_quotes_uses_recorded_fixture

<!-- id: REQ-QUOTE-001.A1 -->
- [ ] `QuotesBuilder::new(&c).fetch()` with no symbols returns `InvalidParams` and makes no HTTP call. `[needs review — no test backs this]`
<!-- id: REQ-QUOTE-001.A2 -->
- [ ] The free functions `quotes()` / `quotes_with_diagnostics()` and `add_symbol` behave the same as the equivalent builder chain. `[needs review — no test backs this]`
<!-- id: REQ-QUOTE-001.A3 -->
- [ ] A batch for `["AAPL","MSFT"]` returns quotes for both symbols. `[needs review — offline_multi_quotes_uses_recorded_fixture always skips because tests/fixtures/quote_v7_MULTI.json does not exist; see F-04]`

<!-- id: REQ-QUOTE-002 -->
## REQ-QUOTE-002 Chunking, concurrency, and ordering

`fetch_v7_quote_values` splits the normalized symbols into chunks. A chunk holds at most 100 symbols (`MAX_V7_QUOTE_SYMBOLS_PER_REQUEST`), and its URL is at most 1,800 bytes (`MAX_V7_QUOTE_URL_BYTES`). A single symbol whose URL alone exceeds the byte limit returns `InvalidParams`. Up to 12 chunks run concurrently (`MAX_V7_QUOTE_CONCURRENT_REQUESTS`, chosen from live probes per the comment at src/core/quotes.rs:37). Results are re-sorted by chunk index, so output order follows input chunk order and then provider order within each chunk. If any chunk fails, the whole call fails, and successful chunks are discarded (see F-01).

implementations:
  - src/core/quotes.rs:fetch_v7_quote_values
  - src/core/quotes.rs:chunk_v7_quote_symbols
  - src/core/quotes.rs:fetch_v7_quote_chunk
  - src/core/quotes.rs:v7_quote_url

verifies:
  - tests/quotes/offline.rs:batch_quotes_chunks_large_symbol_lists_by_count
  - src/core/quotes.rs:quote_symbol_chunks_respect_max_symbol_count
  - src/core/quotes.rs:quote_symbol_chunks_respect_encoded_url_byte_limit

<!-- id: REQ-QUOTE-002.A1 -->
- [x] 101 symbols are sent as two requests (100 + 1), and the output keeps input order (`quotes[0] == CHUNK000`, `quotes[100] == CHUNK100`).
<!-- id: REQ-QUOTE-002.A2 -->
- [x] Long symbols are split so that every chunk URL is at most 1,800 bytes.
<!-- id: REQ-QUOTE-002.A3 -->
- [ ] A single symbol that exceeds the URL byte limit by itself returns `InvalidParams`. `[needs review — no test backs this]`
<!-- id: REQ-QUOTE-002.A4 -->
- [ ] When one chunk fails, the batch error is that chunk's error. `[needs review — no test backs this; it is also unclear whether partial results should be returned]`

<!-- id: REQ-QUOTE-003 -->
## REQ-QUOTE-003 Per-item projection, dropping, and missing symbols

For batches, a malformed node (it fails to deserialize, has no or an invalid symbol, has an unprojectable instrument, or has an unresolved or invalid currency) is dropped as a `YfWarning::DroppedItem { endpoint: "quotes", item: "quote", key, reason }`. The remaining quotes are still returned. In contrast, the single-symbol Ticker path returns hard errors for the same cases. Every requested symbol that no returned node matches (case-insensitive, trimmed) produces `DroppedItem { reason: ProviderUnavailable { feature: "quote" } }`. A single-symbol request counts as resolved by any node (src/core/quotes.rs:1701-1705). Under Strict, any dropped item or omitted field returns `YfError::DataQuality`. As a side effect, currency hints and an `Instrument` are cached on the client for each provider symbol and for the requested alias (`store_v7_quote_side_effects`), and later calls such as option-chain currency resolution read them.

implementations:
  - src/quote/mod.rs:QuotesBuilder.fetch_with_diagnostics
  - src/core/quotes.rs:report_missing_requested_quote_values
  - src/core/quotes.rs:quote_nodes_from_values_with_context
  - src/core/quotes.rs:quote_node_from_value_with_context
  - src/core/quotes.rs:V7QuoteNode.to_quote_item_with_context
  - src/core/quotes.rs:store_v7_quote_side_effects

verifies:
  - tests/quotes/offline.rs:batch_quotes_with_diagnostics_reports_requested_symbols_missing_from_response
  - tests/quotes/offline.rs:malformed_quote_node_missing_symbol_is_dropped_from_batch
  - tests/quotes/offline.rs:malformed_quote_node_invalid_symbol_is_dropped_from_batch
  - tests/quotes/offline.rs:malformed_optional_quote_field_is_omitted_from_batch_item
  - tests/quotes/offline.rs:batch_quotes_with_diagnostics_drops_quote_without_required_currency

<!-- id: REQ-QUOTE-003.A1 -->
- [x] A request for `AAPL,ZZZZINVALID` where only AAPL comes back returns 1 quote plus `DroppedItem{key:"ZZZZINVALID", reason: ProviderUnavailable{feature:"quote"}}`. With `.strict()` it returns `YfError::DataQuality`.
<!-- id: REQ-QUOTE-003.A2 -->
- [x] A node with no `symbol` is dropped and the valid sibling (MSFT) is kept. Strict returns `DataQuality`.
<!-- id: REQ-QUOTE-003.A3 -->
- [x] A node with an invalid symbol (`BAD` request) is dropped, giving empty data. Strict returns `DataQuality`.
<!-- id: REQ-QUOTE-003.A4 -->
- [x] A malformed optional field (price) gives `price = None` on that item, keeps both items, and records a warning. Strict returns `DataQuality`.
<!-- id: REQ-QUOTE-003.A5 -->
- [x] A node with no currency is dropped with `DroppedItem{ reason: CurrencyUnresolved, key:"AAPL" }`.

<!-- id: REQ-QUOTE-004 -->
## REQ-QUOTE-004 Quote field mapping

The `Quote` field rules:
- `name` is `longName`, falling back to `shortName`.
- `bid` / `ask` are `BookLevel { price, size }` built from `bid`/`bidSize` and `ask`/`askSize` (`positive_book_level`; non-positive values are presumably rejected `[needs review — positive_book_level body not read]`).
- Counter fields (`regularMarketVolume`, `bidSize`, `askSize`) accept JSON numbers or numeric strings (`JsonU64`).
- `price` and `previous_close` are built with the resolved quote currency unit. Minor-unit currencies are handled in `currency_units()`.
- Exchange comes from several candidates (`fullExchangeName`, `exchange`, ...). Once a valid primary candidate is found, failures of lower-priority candidates are ignored. When every candidate is invalid, one aggregated warning is emitted.

implementations:
  - src/core/quotes.rs:V7QuoteNode.quote_from_instrument_with_context
  - src/core/quotes.rs:V7QuoteNode.to_quote_item_with_context

verifies:
  - tests/quotes/names.rs:quote_prefers_long_name_for_display_name
  - tests/quotes/new_fields.rs:quote_v7_bid_ask_are_mapped_to_book_levels
  - tests/quotes/offline.rs:quote_v7_counter_fields_accept_numeric_strings
  - tests/quotes/offline.rs:lower_priority_exchange_failures_after_valid_candidate_are_ignored
  - tests/quotes/offline.rs:invalid_exchange_candidates_emit_one_aggregated_warning

<!-- id: REQ-QUOTE-004.A1 -->
- [x] For BHP.MU, `name == "BHP Group Ltd"` (the longName is preferred).
<!-- id: REQ-QUOTE-004.A2 -->
- [x] Fixture `bid`/`bidSize`/`ask`/`askSize` map to `quote.bid` / `quote.ask` book levels with the same price and size.
<!-- id: REQ-QUOTE-004.A3 -->
- [x] Numeric-string counters (e.g. `"regularMarketVolume": "1234"`) are parsed, and the quote survives.
<!-- id: REQ-QUOTE-004.A4 -->
- [x] When every exchange candidate is invalid, `exchange = None` with exactly one exchange-candidate warning. Strict returns `DataQuality`. When a valid candidate exists, failures of lower-priority candidates produce no warning.

<!-- id: REQ-QUOTE-005 -->
## REQ-QUOTE-005 Transport, auth, error envelope, and caching for v7 quotes

Requests use `AuthMode::OptionalCrumb`: the first attempt is sent without a crumb. On 401/403 or an invalid-crumb body, the client fetches a cookie and crumb, refreshing a stale cached crumb if needed, and retries. HTTP status is mapped to `NotFound` (404) and `RateLimited` (429), each carrying the URL. A `quoteResponse.error` object becomes `YfError::Api("yahoo error: <description>")` even when `result` is null. A missing `quoteResponse` or `result` is `MissingData`. The cache endpoint is `CacheEndpoint::Quote`. With `CacheMode::Default` quotes bypass the response cache, and with `CacheMode::Use` they read and write it. The cache validator rejects bodies that contain Yahoo errors.

implementations:
  - src/core/quotes.rs:fetch_v7_quote_chunk
  - src/core/quotes.rs:validate_v7_quote_body
  - src/core/quotes.rs:reject_v7_quote_error

verifies:
  - tests/quotes_status_mapping.rs:quotes_v7_404_maps_to_not_found
  - tests/quotes_status_mapping.rs:quotes_v7_429_maps_to_rate_limited
  - tests/quotes/error_synthetic.rs:quote_v7_yahoo_error_with_null_result_returns_api_error
  - tests/quotes/retry_synthetic.rs:batch_quotes_401_then_retry_with_crumb_succeeds
  - tests/quotes/retry_synthetic.rs:batch_quotes_401_with_stale_cached_crumb_refreshes_before_retry
  - tests/quotes/offline.rs:default_quote_cache_mode_bypasses_response_cache
  - tests/quotes/offline.rs:explicit_quote_cache_mode_uses_response_cache

<!-- id: REQ-QUOTE-005.A1 -->
- [x] A 404 returns `YfError::NotFound` and a 429 returns `YfError::RateLimited`. Both carry a URL containing `/v7/finance/quote`.
<!-- id: REQ-QUOTE-005.A2 -->
- [x] `{"quoteResponse":{"result":null,"error":{"description":..}}}` returns `YfError::Api` with the Yahoo description.
<!-- id: REQ-QUOTE-005.A3 -->
- [x] A bare 401 triggers the cookie and crumb fetch and a retry with `crumb=<value>`. A stale preauth crumb is refreshed before the retry.
<!-- id: REQ-QUOTE-005.A4 -->
- [x] With a client cache TTL of 1 minute, two `fetch()` calls in `CacheMode::Default` make 2 HTTP calls, and in `CacheMode::Use` make 1.

<!-- id: REQ-OPT-001 -->
## REQ-OPT-001 Option expiration dates

`Ticker::options(&self) -> Result<Vec<i64>, YfError>` sends `GET <base_options_v7>/<SYM>` with no `date` parameter and returns `optionChain.result[last].expirationDates` as Unix-second timestamps, or an empty vec when absent. An `optionChain.error` object returns `YfError::Api("yahoo error: <description|message|json>")`. A missing `optionChain` or empty `result` is `MissingData("empty options result")`. There is no `_with_diagnostics` variant, and this path writes no currency hints or instrument to the client cache.

implementations:
  - src/ticker/mod.rs:Ticker.options
  - src/ticker/options.rs:expiration_dates
  - src/ticker/options.rs:first_option_result
  - src/ticker/options.rs:option_chain_error_message

verifies:
  - tests/ticker/options.rs:options_expirations_happy
  - tests/ticker/options.rs:options_expirations_surface_yahoo_error
  - tests/ticker/offline.rs:offline_options_uses_recorded_fixtures

<!-- id: REQ-OPT-001.A1 -->
- [x] The recorded AAPL fixture yields a non-empty list of expirations.
<!-- id: REQ-OPT-001.A2 -->
- [x] `optionChain.error.description = "No options found"` returns `YfError::Api` with a message containing `yahoo error:` and `No options found`.
<!-- id: REQ-OPT-001.A3 -->
- [ ] A result without `expirationDates` returns `Ok(vec![])`. `[needs review — no test backs this]`

<!-- id: REQ-OPT-002 -->
## REQ-OPT-002 Option chain fetch and envelope handling

`Ticker::option_chain(&self, date: Option<i64>) -> Result<OptionChain, YfError>` and `option_chain_with_diagnostics(..) -> Result<YfResponse<OptionChain>, YfError>` send `GET <base_options_v7>/<SYM>[?date=<ts>]`. With `date=None`, Yahoo returns its default, documented as the "nearest expiration" (src/ticker/mod.rs:464) `[needs review — this is Yahoo behaviour, not enforced in code]`. Diagnostics endpoint label: `"options"`. Auth is `OptionalCrumb` with retry on invalid crumb, and the cache endpoint is `CacheEndpoint::Options`. `OptionChain` is re-exported from `paft::market::options`, with `contracts: Vec<OptionContract>` (calls first, then puts) and `calls()` / `puts()` iterators. An empty response (no `options` node, or no calls and no puts) returns an empty chain without resolving the underlying or the currency.

implementations:
  - src/ticker/mod.rs:Ticker.option_chain
  - src/ticker/mod.rs:Ticker.option_chain_with_diagnostics
  - src/ticker/options.rs:option_chain
  - src/ticker/options.rs:option_chain_with_diagnostics
  - src/ticker/options.rs:fetch_options_raw
  - src/ticker/options.rs:validate_options_body
  - src/ticker/options.rs:option_date_has_contracts

verifies:
  - tests/ticker/options.rs:option_chain_for_specific_date
  - tests/ticker/options.rs:option_chain_with_diagnostics_surfaces_yahoo_error
  - tests/ticker/options.rs:options_retry_with_crumb_on_403
  - tests/ticker/offline.rs:offline_options_uses_recorded_fixtures

<!-- id: REQ-OPT-002.A1 -->
- [x] For the first recorded AAPL expiration, the chain has non-empty calls and puts with USD strikes, and every contract's `expiration_at` equals the requested date. No v7 quote request is made when the options quote carries a currency.
<!-- id: REQ-OPT-002.A2 -->
- [x] A dated request whose body contains `optionChain.error` ("No data found") returns `YfError::Api`.
<!-- id: REQ-OPT-002.A3 -->
- [x] A 403 caused by a stale crumb triggers a cookie and crumb refresh and a retry with the new crumb. A bare no-crumb request is never sent when a preauth crumb exists. An empty options node gives an empty chain.
<!-- id: REQ-OPT-002.A4 -->
- [ ] `option_chain(None)` sends no `date` parameter and returns the provider's default expiration. `[needs review — no test backs this]`

<!-- id: REQ-OPT-003 -->
## REQ-OPT-003 Contract expiration resolution

Each contract's expiration is resolved in this order:
1. the contract's own `expiration`
2. the date node's `expirationDate`
3. the `date=` query parameter of the request URL

A contract with none of these is dropped (`MissingRequiredField{field:"expiration"}`). A timestamp that cannot be represented is dropped (`InvalidField{field:"expiration"}`). `expiration_at` is set to the UTC instant, and the contract key uses its `date_naive()`.

implementations:
  - src/ticker/options.rs:option_chain_with_diagnostics
  - src/ticker/options.rs:project_option_contract

verifies:
  - tests/ticker/options_expiry_from_url_fallback.rs:option_chain_expiration_falls_back_to_url_date

<!-- id: REQ-OPT-003.A1 -->
- [x] When both `expirationDate` and contract `expiration` are omitted, every call and put has `expiration_at.timestamp() == <requested date>`.
<!-- id: REQ-OPT-003.A2 -->
- [ ] A contract with no resolvable expiration (undated request, no fields) is dropped with a `MissingRequiredField` warning. `[needs review — no test backs this]`

<!-- id: REQ-OPT-004 -->
## REQ-OPT-004 Option currency resolution

The trading currency comes from `project_currency_resolution`. Evidence is gathered in this order:
1. the options response `quote.currency`
2. the client's `resolve_trading_currency`, which may fetch a v7 quote or infer the currency from cached hints or the exchange suffix

When the options response includes `quote`, its currency, exchange, and quoteType hints are stored on the client. If the currency is invalid or unresolvable, every contract is dropped with the resolution issue (e.g. `InvalidCurrency{code}`) or with `CurrencyUnresolved`. Strike, last, bid, and ask are built with the resolved unit, including minor-unit scaling (GBp → GBP).

implementations:
  - src/ticker/options.rs:option_chain_with_diagnostics
  - src/ticker/options.rs:currency_from_result
  - src/ticker/options.rs:project_option_contract
  - src/ticker/options.rs:optional_option_price

verifies:
  - tests/ticker/options.rs:option_chain_currency_fallback_fetches_quote
  - tests/ticker/options.rs:option_chain_missing_currency_uses_resolver_inference_after_quote_failure
  - tests/ticker/options.rs:option_chain_with_diagnostics_drops_contract_when_direct_currency_is_invalid

<!-- id: REQ-OPT-004.A1 -->
- [x] When the options quote has no `currency`, at least one v7 quote request is made, and the contracts get USD strikes.
<!-- id: REQ-OPT-004.A2 -->
- [x] For TSCO.L with no currency and a v7 quote that fails with 500, the resolver infers the currency. Strikes are GBP, and a raw strike of 444.1 GBp becomes 4.441 GBP.
<!-- id: REQ-OPT-004.A3 -->
- [x] `quote.currency = "!!!"` in best-effort mode returns `Ok` with zero contracts and a `DroppedItem{item:"option_contract", key:"AAPL250117C00180000", reason: InvalidCurrency{code:"!!!"}}` warning.

<!-- id: REQ-OPT-005 -->
## REQ-OPT-005 Underlying instrument identity

The underlying `Instrument` is built from `underlyingSymbol` (or `quote.symbol`), the `quoteType` mapped to `AssetKind` via `parse_yahoo_quote_type`, and the first parseable exchange among `fullExchangeName`, `exchange`, `market`, and `marketCapFigureExchange`. On success it is cached on the client under both the requested symbol and the instrument symbol. Otherwise the client's cached instrument (e.g. from an earlier quote) is used. When there are contracts but no underlying type can be determined, the call fails hard with `YfError::OptionUnderlyingTypeUnavailable { symbol, quote_type: Option<String> }` (Display contains "contracts present, underlying type unavailable") in every data-quality mode. Each contract key may also carry a contract `Instrument` parsed from `contractSymbol` as `AssetKind::Option`. If that parse fails, the field is omitted with a warning.

implementations:
  - src/ticker/options.rs:underlying_instrument
  - src/ticker/options.rs:underlying_instrument_from_result
  - src/ticker/options.rs:quote_type_from_result
  - src/ticker/options.rs:OptQuoteNode.exchange
  - src/ticker/options.rs:optional_contract_instrument

verifies:
  - tests/ticker/options.rs:option_chain_uses_response_underlying_identity
  - tests/ticker/options.rs:option_chain_with_contracts_errors_when_underlying_quote_type_is_missing
  - tests/ticker/options.rs:option_chain_with_contracts_errors_when_underlying_quote_type_is_malformed

<!-- id: REQ-OPT-005.A1 -->
- [x] For SPY, the contract key's underlying has symbol `SPY`, `AssetKind::Fund`, and exchange `NYSE`.
<!-- id: REQ-OPT-005.A2 -->
- [x] With contracts present and no `quoteType`, the call returns `OptionUnderlyingTypeUnavailable{symbol:"NOQT", quote_type: None}`. A malformed `quoteType: "!!!"` returns the same error with `quote_type: Some("!!!")`.
<!-- id: REQ-OPT-005.A3 -->
- [ ] If an earlier quote call cached the instrument, a chain whose response lacks `quoteType` uses the cached instrument instead of failing. `[needs review — no test backs this]`

<!-- id: REQ-OPT-006 -->
## REQ-OPT-006 Per-contract projection and tolerance

Each call and put is projected independently into `OptionContract`:
- Fields: `key` (underlying, side, strike, expiry date, optional contract instrument), `currency`, `price` (lastPrice), `bid`, `ask`, `volume`, `open_interest` (u64; numeric strings accepted), `implied_volatility` (`NonNegativeDecimal`), `in_the_money`, `expiration_at`, and `last_trade_at`. `greeks` is always `None`.
- A contract is dropped (`DroppedItem{endpoint:"options", item:"option_contract"}`) when it fails to deserialize, has a missing or invalid `strike`, or its strike cannot be converted.
- A malformed optional field (lastPrice, bid, ask, impliedVolatility, lastTradeDate, ...) becomes `None` with an `OmittedPresentField` warning.
- The diagnostic key is `contractSymbol`, falling back to `"<side>:<strike>@<exp>"` or `"<side>[idx]@<exp>"`.
- Under Strict, any drop or omission is an error.

implementations:
  - src/ticker/options.rs:project_option_side
  - src/ticker/options.rs:project_option_contract
  - src/ticker/options.rs:optional_non_negative_decimal_f64
  - src/ticker/options.rs:optional_option_timestamp
  - src/ticker/options.rs:option_contract_diag_key
  - src/ticker/options.rs:option_contract_diag_key_from_value

verifies:
  - tests/ticker/options.rs:option_chain_skips_bad_strikes_and_keeps_valid_contracts
  - tests/ticker/options.rs:option_contract_counter_fields_accept_numeric_strings

<!-- id: REQ-OPT-006.A1 -->
- [x] With one bad-strike call, one valid call, and one valid put, the result has 1 call and 1 put, plus `DroppedItem{reason: InvalidField{field:"strike"}, key:"AAPL250117C00175000"}`.
<!-- id: REQ-OPT-006.A2 -->
- [x] On the surviving call, an invalid `lastPrice`, `ask`, or `impliedVolatility` becomes `None` with an `OmittedPresentField` warning keyed `AAPL250117C00180000`. A valid `bid` survives, and the strike is 180.0.
<!-- id: REQ-OPT-006.A3 -->
- [x] `volume: "15"` and `openInterest: "2048"` (strings) give `Some(15)` and `Some(2048)`.
<!-- id: REQ-OPT-006.A4 -->
- [ ] A negative `impliedVolatility` is omitted with a `ConversionFailed` warning. `[needs review — no test backs this]`

<!-- id: TICK-NONGOALS -->
## Non-goals

- Greeks are not computed or mapped. `greeks: None` is hard-coded (src/ticker/options.rs:382).
- `Info` does not include ESG data. The info path deliberately avoids the "dead ESG module" (tests/ticker/info_offline.rs:354, `info_with_diagnostics_does_not_fetch_dead_esg_module`).
- `Info` does not mirror Yahoo's raw response shape. It is grouped into paft domain models (src/ticker/model.rs:11-15).
- `[fill in — not derivable from code]` for anything beyond these explicit signals (e.g. streaming quotes, ISIN sources other than Business Insider).

<!-- id: TICK-OPENQ -->
## Open questions

1. Should the single-symbol quote, fast_info, and info paths reject a response node whose symbol differs from the requested one? Today the first node is accepted, and the single-request "resolved" check always passes (src/core/quotes.rs:1591-1597, 1701-1705).
2. Should batch quotes return partial results (plus diagnostics) when one chunk fails, instead of failing the whole batch?
3. Is it intended that `key_statistics()` returns `KeyStatistics::default()` when every v7 node is malformed, while `quote()` / `fast_info()` / `info()` error in that case?
4. Should `fast_info` derive `last` from `previous_close`, as the test name `fast_info_derives_last_price` and its "fallback to previous close" comment suggest, or should the test be renamed?
5. Is `OptionUnderlyingTypeUnavailable` meant to be a hard error even in best-effort mode, unlike every other per-contract problem, which only drops that contract?
6. Should `Ticker::isin` honour cache mode and data quality, and get a `_with_diagnostics` variant like every other method?
7. Should batch quotes deduplicate repeated symbols?
8. What is the intended contract for `option_chain(Some(date))` when `date` is not a listed expiration?

<!-- id: TICK-REVIEW -->
## Code review findings

Ranked by severity (High → Low).

<!-- id: TICK-REVIEW-HIGH -->
### High

- **F-01 Batch quote failures discard all results and do not short-circuit.** src/core/quotes.rs:1428-1437 collects every chunk future through `buffer_unordered(12)` into a `Vec` and only then applies `?`. A single failed chunk (e.g. a 429) discards all successful chunks. All remaining chunks are still sent, up to 12 at a time, after the first failure, so a rate-limited batch keeps sending requests. Consider `try_collect` / `try_buffer_unordered` for fail-fast, or return partial results with a `DroppedItem` per failed chunk.

<!-- id: TICK-REVIEW-MED -->
### Medium

- **F-02 Single-symbol quote accepts any returned symbol.** `required_quote_node_from_nodes` (src/core/quotes.rs:1591-1597) takes the first node without checking its symbol. `mark_resolved_requested_symbol` (1701-1705) marks a 1-symbol request as resolved even when the symbols differ, and `store_v7_quote_side_effects` (1626-1629) stores that node's currency hints under the requested symbol. A provider substitution or redirect is therefore silently accepted as the requested ticker, and the client's currency-hint cache can be polluted for later option and fundamentals calls.
- **F-03 Inconsistent empty-result semantics in key_statistics.** src/ticker/quote.rs:106-115: an empty v7 result returns `MissingData`, but when every node is malformed the code falls through to `KeyStatistics::default()` and succeeds in best-effort mode. Every other single-symbol path uses `required_quote_node_from_nodes` and errors.
- **F-04 Vacuous test.** tests/quotes/offline.rs:33 `offline_multi_quotes_uses_recorded_fixture` checks for `tests/fixtures/quote_v7_MULTI.json` and silently returns when it is missing. That file does not exist (the recorded fixture is `quote_v7_AAPL-MSFT.json`, matching the `symbols.join("-")` fixture key at src/core/quotes.rs:1489), so the test always passes without asserting anything. The helper `mock_quote_v7_multi` (tests/common.rs:187) uses the same stale `MULTI` key.
- **F-05 Misleading test name and comment versus behaviour.** tests/ticker/quote.rs:260 `fast_info_derives_last_price`, with the comment "fallback to previous close" (line 303), actually asserts `snapshot.last.is_none()` (line 299). No derivation exists. examples/01_basic_usage.rs:61 does the fallback by hand. This invites a wrong contract assumption.

<!-- id: TICK-REVIEW-LOW -->
### Low

- **F-06 `isin` ignores the ticker's options.** src/ticker/mod.rs:442-449 passes only `retry_override()`, so cache mode and data quality are ignored and there is no diagnostics variant. The symbol is normalized three times (src/ticker/mod.rs:443, src/ticker/isin.rs:40, src/ticker/isin.rs:62). The `^` index heuristic is the only "no ISIN" short-circuit.
- **F-07 "first" helpers take the last element.** `first_option_result` pops the last `result` (src/ticker/options.rs:647). `option_chain_with_diagnostics` pops the last date node (src/ticker/options.rs:87). If Yahoo returns more than one, the earlier entries are silently ignored, and the names suggest otherwise.
- **F-08 Redundant options requests and missing parity.** `expiration_dates` (src/ticker/options.rs:32-44) fetches the same URL as `option_chain(None)` but has no diagnostics variant, does not cache the underlying instrument or currency hints, and discards the chain it already downloaded. Options bypass the cache by default, so `options()` followed by `option_chain(None)` makes two identical requests.
- **F-09 Hard error for the underlying type in best-effort mode.** src/ticker/options.rs:178-181 fails the whole chain, unlike the drop-per-contract policy everywhere else. The behaviour is locked in by tests. It may be intentional `[needs review]`.
- **F-10 Quadratic chunk building.** src/core/quotes.rs:1456-1474 clones `current` and rebuilds and serializes the URL for every symbol, which is O(k²) per chunk with k ≤ 100. It is cheap in practice but could track the running URL length instead.
- **F-11 No dedup in `QuotesBuilder`.** src/quote/mod.rs:61-75: repeated symbols are sent and returned twice.
- **F-12 Limited JS number grammar.** `JsDataParser::parse_number` (src/ticker/isin.rs:304-328) handles no leading `-`, `+`, or `.`. A negative numeric argument in the `mmSuggestDeliver(...)` callback makes the whole body unparseable, so the ISIN lookup returns `None`.
- **F-13 `TryFrom<V7QuoteNode> for Quote` drops diagnostics.** src/core/quotes.rs:1720-1727 always uses `DataQuality::BestEffort` and discards warnings, which bypasses the caller's data-quality policy.
- **F-14 Missing tests.** Not covered:
  - empty-symbol `InvalidParams` (src/quote/mod.rs:100-104)
  - the free functions `quotes()` / `quotes_with_diagnostics()` and `add_symbol`
  - `info_strict`
  - `option_chain(None)`
  - `options()` with no `expirationDates`
  - the `isin` `^` short-circuit
  - `key_statistics` with a quoteSummary failure, in both best-effort and strict modes
  - a single oversized symbol hitting the URL byte limit
  - a chunk failure in a multi-chunk batch
  - use of the cached-instrument fallback for the option underlying

<!-- id: TICK-NOTES -->
## Reverse-engineering notes

- **Tests not executed.** The local `rustc 1.88.0` is below the crate's `rust-version = "1.91"`, and cargo refused to build. Every `[x]` is based on reading the test source, not on a green run. `[needs review — tests not executed in this session]`
- **High confidence:** REQ-TICK-002, REQ-TICK-003, REQ-TICK-006, REQ-QUOTE-003, REQ-QUOTE-005, REQ-OPT-002, REQ-OPT-004, REQ-OPT-005, REQ-OPT-006. Behaviour is read directly from code and locked by explicit offline tests.
- **Medium confidence:** REQ-TICK-004 and REQ-TICK-005 (merge and backfill rules come from code, but the strict-mode and failure branches are untested), REQ-QUOTE-002 (the chunk constants are certain, failure semantics untested), and REQ-OPT-001 and REQ-OPT-003 (thin test coverage of edge cases).
- **Low confidence / summarized:** REQ-TICK-007 only lists delegation. Its real behaviour belongs to the history, holders, analysis, fundamentals, ESG, news, and profile specs. REQ-QUOTE-004 relies on helper bodies in src/core/quotes.rs (`positive_book_level`, `currency_units`, `exchange_with_context`) that were not read line by line.
- Live tests (`#[ignore = "exercise live Yahoo Finance API"]` in tests/ticker/live.rs, tests/ticker/info_live.rs, tests/ticker/isin_live.rs, tests/quotes/live.rs, and parts of tests/ticker/quote.rs and tests/ticker/profile.rs) are fixture recorders or smoke tests and were not used as acceptance evidence.
- The `verifies:` entries for REQ-QUOTE-002 point at unit tests inside src/core/quotes.rs (`#[cfg(test)] mod tests`) rather than tests/ `[needs review — confirm SpecShip accepts src/ paths in verifies]`.
