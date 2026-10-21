---
id: FUND-DOC
title: Fundamentals and analyst analysis (reverse-engineered)
owner: unassigned
priority: medium
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/fundamentals/mod.rs
  - src/fundamentals/api.rs
  - src/fundamentals/fetch.rs
  - src/fundamentals/model.rs
  - src/fundamentals/wire.rs
  - src/analysis/mod.rs
  - src/analysis/api.rs
  - src/analysis/fetch.rs
  - src/analysis/model.rs
  - src/analysis/wire.rs
  - src/core/quotesummary.rs
  - src/core/call_options.rs
  - src/core/client/urls.rs
  - src/core/currency_resolver/cache.rs
  - src/ticker/mod.rs
  - src/ticker/info.rs
  - tests/fundamentals.rs
  - tests/fundamentals/caching_synthetic.rs
  - tests/fundamentals/fcf_fallback.rs
  - tests/fundamentals/live.rs
  - tests/fundamentals/new_fields.rs
  - tests/fundamentals/offline.rs
  - tests/fundamentals/retry_synthetic.rs
  - tests/fundamentals/status_synthetic.rs
  - tests/analysis.rs
  - tests/analysis/earnings_trend.rs
  - tests/analysis/live.rs
  - tests/analysis/offline.rs
  - tests/analysis/price_target.rs
  - tests/analysis/price_target_live.rs
  - tests/analysis/retry_synthetic.rs
  - tests/analysis/sorted_upgrades.rs
  - tests/analysis/strict_synthetic.rs
  - tests/analysis/yahoo_error_passthrough.rs
  - tests/ticker/shares.rs
  - tests/fixtures/timeseries_*.json
  - tests/fixtures/fundamentals_api_*.json
  - tests/fixtures/analysis_api_*.json
  - examples/02_fundamentals_and_search.rs
  - examples/03_esg_and_analysis.rs
  - examples/07_quarterly_fundamentals.rs
  - examples/08_advanced_analysis.rs
---

<!-- id: FUND-DOC -->
# Fundamentals and analyst analysis (reverse-engineered)

Intent: `yfinance-rs` (v0.9.1) exposes two async builders, `FundamentalsBuilder` and `AnalysisBuilder`, plus matching convenience methods on `Ticker`. They fetch a symbol's financial statements (income statement, balance sheet, cash flow, annual or quarterly), earnings history, corporate calendar events and shares outstanding, along with analyst data: the recommendation trend and summary, upgrade/downgrade history, price targets, and the earnings trend (estimates, EPS trend, EPS revisions). Statements and share counts come from Yahoo's fundamentals-timeseries endpoint. Everything else comes from Yahoo `quoteSummary` modules. Every public fetch has a plain variant that returns data only and a `_with_diagnostics` variant that returns `YfResponse<T>`, which carries projection warnings. Under the default (best-effort) data-quality policy, projection problems become warnings. Under `strict()`, they become `YfError::DataQuality`. Public data types are re-exported from the `paft` crate (`paft::fundamentals::*`). [purpose unclear: why the crate needs strict vs best-effort beyond what the code shows. The code shows the mechanism but not the business reason.]

<!-- id: REQ-FUND-001 -->
## Fundamentals builder surface and call options

`FundamentalsBuilder::new(client: &YfClient, symbol: impl Into<String>) -> FundamentalsBuilder` clones the client, stores the symbol unnormalized, and starts with `CallOptions::default()`. The builder gets the shared call-option setters from `impl_call_option_setters!`: `cache_mode(CacheMode)`, `retry_policy(Option<RetryConfig>)`, `data_quality(DataQuality)` and `strict()`. Each fetch has a plain form that returns `Result<T, YfError>` and a `_with_diagnostics` form that returns `Result<YfResponse<T>, YfError>`. The plain form is `self.<x>_with_diagnostics(..).await?.into_data()`, which means it discards warnings.

Public methods:
- `income_statement(quarterly: bool, override_currency: Option<Currency>) -> Vec<IncomeStatementRow>`
- `balance_sheet(quarterly, override_currency) -> Vec<BalanceSheetRow>`
- `cashflow(quarterly, override_currency) -> Vec<CashflowRow>`
- `earnings(override_currency) -> Earnings`
- `calendar() -> Calendar`
- `shares(quarterly) -> Vec<ShareCount>`
- `shares_between(quarterly, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<ShareCount>`

`Ticker` wraps these methods:
- `income_stmt` / `quarterly_income_stmt`
- `balance_sheet` / `quarterly_balance_sheet`
- `cashflow` / `quarterly_cashflow`
- `earnings`
- `calendar`
- `shares` / `quarterly_shares`
- `shares_between` / `quarterly_shares_between`
- the `_with_diagnostics` twin of each

The types `IncomeStatementRow`, `BalanceSheetRow`, `CashflowRow`, `Calendar`, `Earnings`, `EarningsYear`, `EarningsQuarter`, `EarningsQuarterEps` and `ShareCount` are re-exports from `paft`.

implementations:
  - src/fundamentals/mod.rs:FundamentalsBuilder.new
  - src/fundamentals/mod.rs:FundamentalsBuilder.income_statement
  - src/fundamentals/mod.rs:FundamentalsBuilder.income_statement_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.balance_sheet
  - src/fundamentals/mod.rs:FundamentalsBuilder.balance_sheet_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.cashflow
  - src/fundamentals/mod.rs:FundamentalsBuilder.cashflow_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.earnings
  - src/fundamentals/mod.rs:FundamentalsBuilder.earnings_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.calendar
  - src/fundamentals/mod.rs:FundamentalsBuilder.calendar_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.shares
  - src/fundamentals/mod.rs:FundamentalsBuilder.shares_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.shares_between
  - src/fundamentals/mod.rs:FundamentalsBuilder.shares_between_with_diagnostics
  - src/fundamentals/mod.rs:FundamentalsBuilder.shares_window_with_diagnostics
  - src/ticker/mod.rs:Ticker.income_stmt
  - src/ticker/mod.rs:Ticker.quarterly_income_stmt
  - src/ticker/mod.rs:Ticker.balance_sheet
  - src/ticker/mod.rs:Ticker.quarterly_balance_sheet
  - src/ticker/mod.rs:Ticker.cashflow
  - src/ticker/mod.rs:Ticker.quarterly_cashflow
  - src/ticker/mod.rs:Ticker.earnings
  - src/ticker/mod.rs:Ticker.calendar
  - src/ticker/mod.rs:Ticker.shares
  - src/ticker/mod.rs:Ticker.quarterly_shares
  - src/ticker/mod.rs:Ticker.shares_between
  - src/ticker/mod.rs:Ticker.quarterly_shares_between

verifies:
  - tests/fundamentals/offline.rs:offline_income_quarterly_uses_recorded_fixture
  - tests/fundamentals/offline.rs:offline_balance_sheet_annual_uses_recorded_fixture
  - tests/fundamentals/offline.rs:offline_cashflow_annual_uses_recorded_fixture
  - tests/fundamentals/offline.rs:offline_earnings_uses_recorded_fixture
  - tests/fundamentals/offline.rs:offline_calendar_uses_recorded_fixture
  - tests/ticker/shares.rs:offline_shares_uses_recorded_fixture

<!-- id: REQ-FUND-001.A1 -->
- [x] Given recorded Yahoo fixtures, the quarterly income statement, annual balance sheet, annual cash flow, earnings and calendar each return non-empty data. The quarterly income statement also returns empty diagnostics.
<!-- id: REQ-FUND-001.A2 -->
- [x] `Ticker::shares()` requests `type=annualOrdinarySharesNumber`, `Ticker::quarterly_shares()` requests `type=quarterlyOrdinarySharesNumber`, and both return positive share counts from the MSFT fixtures.
<!-- id: REQ-FUND-001.A3 -->
- [x] Calling `strict()` on the builder makes projection warnings fail the call with `YfError::DataQuality` (shown in REQ-FUND-005, -006, -007, -009, -010 and -011).
<!-- id: REQ-FUND-001.A4 -->
- [ ] The plain (non-diagnostics) variants return the same data as the `_with_diagnostics` variants, minus the warnings. [needs review — no test compares the two directly]

<!-- id: REQ-FUND-002 -->
## Financial statements are fetched from the fundamentals-timeseries endpoint and grouped into period rows

All three statements go through `fetch_timeseries_data`:
- Normalize the symbol with `normalize_symbol`, which trims, uppercases and validates it.
- Build the URL from `client.symbol_url(SymbolEndpoint::Timeseries, symbol)` with these query parameters: `symbol`, `type=<prefix><Key>` joined with commas (`prefix` is `annual` or `quarterly`), `period1` and `period2`.
- Use a statement window that ends at the next UTC midnight and starts `STATEMENT_LOOKBACK_DAYS = 365*5` days earlier. This keeps the URL, and so the cache key, stable for a whole UTC day.
- Fetch through `fetch_text_with_auth_retry` with `AuthMode::RequiredCrumb`, `CacheEndpoint::Fundamentals`, `retry_on_invalid_crumb_body: true`, and `validate_timeseries_body` as the cache validator. The endpoint label is `timeseries_<income_statement|balance_sheet|cash_flow>_<prefix>`.

Processing rules for each result item:
- Skip items that have only metadata (`meta` present, no `timestamp`, no values) without a warning.
- Drop items that lack `timestamp` or have no values, recording `DroppedItem{item:"timeseries_item"}`.
- Process every value key in an item. Grouped items with several keys are supported.
- Match the i-th value to the i-th timestamp by position.
- Create a row for a timestamp only when a value with a `reportedValue.raw` exists.
- Store rows in a `BTreeMap<i64, Row>` keyed by timestamp and return them newest first (`rows_map.into_values().rev()`).
- Build the period as `ReportingPeriod::date(i64_to_date(ts))`.

An empty `timeseries.result` array returns `Ok(vec![])`.

implementations:
  - src/fundamentals/api.rs:fetch_timeseries_data
  - src/fundamentals/api.rs:fetch_timeseries_body
  - src/fundamentals/api.rs:timeseries_url
  - src/fundamentals/api.rs:timeseries_window
  - src/fundamentals/api.rs:window_ending_at_next_utc_midnight
  - src/fundamentals/api.rs:next_utc_midnight_timestamp
  - src/fundamentals/api.rs:timestamp_days_before
  - src/fundamentals/api.rs:item_is_empty_metadata
  - src/fundamentals/api.rs:row_for_timestamp
  - src/fundamentals/api.rs:period_from_timestamp
  - src/fundamentals/api.rs:process_statement_money_values
  - src/fundamentals/api.rs:process_statement_u64_values
  - src/fundamentals/wire.rs:TimeseriesEnvelope
  - src/fundamentals/wire.rs:TimeseriesData

verifies:
  - src/fundamentals/api.rs:statement_window_is_stable_within_utc_day
  - src/fundamentals/api.rs:next_utc_midnight_advances_at_utc_boundary
  - tests/fundamentals/caching_synthetic.rs:default_timeseries_windows_hit_fundamentals_cache_across_seconds
  - tests/fundamentals/new_fields.rs:income_statement_accepts_timeseries_items_without_meta
  - tests/fundamentals/new_fields.rs:income_statement_skips_metadata_only_timeseries_items_losslessly
  - tests/fundamentals/new_fields.rs:income_statement_processes_all_fields_in_grouped_timeseries_item
  - tests/fundamentals/new_fields.rs:income_statement_preserves_large_integer_statement_values
  - tests/fundamentals/new_fields.rs:statement_values_without_reported_raw_do_not_create_rows

<!-- id: REQ-FUND-002.A1 -->
- [x] Any two times within the same UTC day produce the same statement window. The window ends at the next UTC midnight and spans exactly 5×365 days.
<!-- id: REQ-FUND-002.A2 -->
- [x] With `cache_ttl` set, two default-window calls to `balance_sheet` and `shares` hit the network once each.
<!-- id: REQ-FUND-002.A3 -->
- [x] A timeseries item without `meta` is still parsed. Metadata-only items are skipped and produce no diagnostics.
<!-- id: REQ-FUND-002.A4 -->
- [x] An item that groups several keys (for example `annualTotalRevenue` and `annualNetIncome`) fills every mapped field of the same row.
<!-- id: REQ-FUND-002.A5 -->
- [x] Large integer statement values are kept exactly. They are parsed as `Decimal`, not `f64`.
<!-- id: REQ-FUND-002.A6 -->
- [x] Values without `reportedValue.raw` create no rows, so the income statement, balance sheet and cash flow all come back empty.
<!-- id: REQ-FUND-002.A7 -->
- [ ] Rows come back newest first. [needs review — the ordering comes from `.rev()` at src/fundamentals/api.rs:241, but no test asserts the order]
<!-- id: REQ-FUND-002.A8 -->
- [ ] Values are matched to timestamps by array position. The `asOfDate` on each value is ignored. [needs review — no test backs this; see finding F1]

<!-- id: REQ-FUND-003 -->
## Income statement field mapping

Timeseries keys requested, all monetary: `TotalRevenue`, `GrossProfit`, `OperatingIncome`, `NetIncome`, `InterestExpense`, `TaxProvision`, `DepreciationAndAmortization`.

These map to the `IncomeStatementRow` fields `total_revenue`, `gross_profit`, `operating_income`, `net_income`, `interest_expense`, `income_tax_expense` (from `TaxProvision`) and `depreciation_and_amortization`, each an `Option<Money>`, plus `period: ReportingPeriod`.

implementations:
  - src/fundamentals/api.rs:income_statement
  - src/fundamentals/api.rs:empty_income_statement_row
  - src/fundamentals/api.rs:assign_income_statement_money

verifies:
  - tests/fundamentals/new_fields.rs:income_statement_new_fields_are_mapped_to_paft_names
  - tests/fundamentals/offline.rs:offline_income_quarterly_uses_recorded_fixture

<!-- id: REQ-FUND-003.A1 -->
- [x] `InterestExpense`, `TaxProvision` and `DepreciationAndAmortization` map to `interest_expense`, `income_tax_expense` and `depreciation_and_amortization`. A negative interest expense keeps its sign.
<!-- id: REQ-FUND-003.A2 -->
- [ ] Keys that the code does not map are ignored without a warning. [needs review — no test backs this. `assign_income_statement_money` has a `_ => {}` arm]

<!-- id: REQ-FUND-004 -->
## Balance sheet field mapping

Timeseries keys requested:
- `TotalAssets`
- `TotalLiabilitiesNetMinorityInterest`
- `StockholdersEquity`
- `CashAndCashEquivalents`
- `LongTermDebt`
- `OrdinarySharesNumber`
- `CurrentAssets`
- `CurrentLiabilities`
- `AccountsReceivable`
- `Inventory`
- `AccountsPayable`
- `NetPPE`
- `Goodwill`
- `OtherIntangibleAssets`

`OrdinarySharesNumber` is the only non-monetary key. It is parsed as `u64` into `shares_outstanding` and does not count as currency evidence. The other keys map to the `Money` fields `total_assets`, `total_liabilities`, `total_equity`, `cash`, `long_term_debt`, `current_assets`, `current_liabilities`, `accounts_receivable`, `inventory`, `accounts_payable`, `net_property_plant_equipment`, `goodwill` and `intangible_assets`.

implementations:
  - src/fundamentals/api.rs:balance_sheet
  - src/fundamentals/api.rs:empty_balance_sheet_row
  - src/fundamentals/api.rs:assign_balance_sheet_money
  - src/fundamentals/api.rs:assign_balance_sheet_shares
  - src/fundamentals/api.rs:process_balance_sheet_item

verifies:
  - tests/fundamentals/new_fields.rs:balance_sheet_new_fields_are_mapped_to_paft_names
  - tests/fundamentals/offline.rs:offline_balance_sheet_annual_uses_recorded_fixture

<!-- id: REQ-FUND-004.A1 -->
- [x] Current assets and liabilities, receivables, inventory, payables, NetPPE, goodwill and other intangibles map to the named `paft` fields as USD `Money`.
<!-- id: REQ-FUND-004.A2 -->
- [ ] `OrdinarySharesNumber` fills `shares_outstanding` as an integer and never goes through currency resolution. [needs review — no test asserts `shares_outstanding` on the balance sheet]

<!-- id: REQ-FUND-005 -->
## Cash flow mapping and free-cash-flow inference

Timeseries keys requested, all monetary: `OperatingCashFlow`, `CapitalExpenditure`, `FreeCashFlow`, `NetIncome`, `DepreciationAndAmortization`. These map to the `CashflowRow` fields `operating_cashflow`, `capital_expenditures`, `free_cash_flow`, `net_income` and `depreciation_and_amortization`.

After projection, if a row has no `free_cash_flow` but has both operating cash flow and capex, the code sets `free_cash_flow = operating_cashflow + capital_expenditures`. Capex is negative in Yahoo's data. The inference records `YfWarning::RepairedData{item:"cash_flow", repair:"inferred missing free cash flow from operating cash flow and capital expenditure"}`. If `Money::try_add` fails, for example on a currency mismatch, the row is left unchanged and no diagnostic is recorded.

implementations:
  - src/fundamentals/api.rs:cashflow
  - src/fundamentals/api.rs:empty_cashflow_row
  - src/fundamentals/api.rs:assign_cashflow_money

verifies:
  - tests/fundamentals/fcf_fallback.rs:cashflow_computes_fcf_when_missing
  - tests/fundamentals/new_fields.rs:cashflow_new_fields_are_mapped_to_paft_names
  - tests/fundamentals/offline.rs:offline_cashflow_annual_uses_recorded_fixture

<!-- id: REQ-FUND-005.A1 -->
- [x] With OCF = 100 USD, capex = −30 USD and no `FreeCashFlow`, the result is `free_cash_flow == 70 USD` plus a `RepairedData` warning for `cash_flow`.
<!-- id: REQ-FUND-005.A2 -->
- [x] In strict mode, the same payload fails with `YfError::DataQuality`. The FCF repair alone is enough to make the call fail (see finding F2).
<!-- id: REQ-FUND-005.A3 -->
- [x] `net_income` and `depreciation_and_amortization` come from the cash-flow timeseries keys.
<!-- id: REQ-FUND-005.A4 -->
- [ ] A currency mismatch between OCF and capex leaves FCF as `None` and records no warning. [needs review — no test backs this]

<!-- id: REQ-FUND-006 -->
## Statement reporting-currency resolution

The currency is resolved only when at least one monetary key has a `reportedValue.raw` (`timeseries_currency_evidence`). The first valid ISO `currencyCode` in the payload is the direct evidence. An invalid code is used as evidence only when no valid code exists.

Resolution calls `client.resolve_reporting_currency(symbol, override_currency, ReportingCurrencyEvidence::TimeseriesCurrencyCode(..), options)` through `project_currency_resolution` with `CurrencyPurpose::Reporting`. A valid direct code means no quote enrichment is needed.

Checks on each value, skipped entirely when `override_currency` is `Some`:
- An invalid `currencyCode` records `OmittedPresentField{reason: InvalidCurrency}` and omits that value.
- A code that conflicts with the expected code records `InvalidField{field:"currencyCode", details:"conflicting timeseries currencyCode values: …"}` and omits only the conflicting value.

If the currency resolved through enrichment is invalid, monetary values are omitted with an `InvalidCurrency` diagnostic.

implementations:
  - src/fundamentals/api.rs:timeseries_currency_evidence
  - src/fundamentals/api.rs:timeseries_value_currency_issue
  - src/fundamentals/api.rs:fetch_timeseries_data
  - src/fundamentals/api.rs:process_statement_money_values

verifies:
  - tests/fundamentals/fcf_fallback.rs:statements_use_first_valid_currency_code_and_compare_units
  - tests/fundamentals/fcf_fallback.rs:statements_use_timeseries_currency_code_before_enrichment
  - tests/fundamentals/fcf_fallback.rs:conflicting_timeseries_currency_codes_omit_only_conflicting_values
  - tests/fundamentals/fcf_fallback.rs:invalid_timeseries_currency_code_is_best_effort_diagnostic
  - tests/fundamentals/fcf_fallback.rs:statements_omit_values_when_enriched_currency_is_invalid
  - tests/fundamentals/caching_synthetic.rs:default_timeseries_windows_hit_fundamentals_cache_across_seconds

<!-- id: REQ-FUND-006.A1 -->
- [x] A valid `currencyCode` in the payload (for example EUR) is used directly, and the quote endpoint is called 0 times.
<!-- id: REQ-FUND-006.A2 -->
- [x] With values coded `!!!`, then `GBP`, then an equivalent GBP, the result has 3 rows. The `!!!` value is omitted with an `InvalidCurrency{code:"!!!"}` diagnostic keyed `TotalRevenue@<ts>`, and the GBP values are kept.
<!-- id: REQ-FUND-006.A3 -->
- [x] When codes conflict (USD vs another code), only the conflicting value is omitted, with an `InvalidField{field:"currencyCode"}` diagnostic. Strict mode fails with `DataQuality`.
<!-- id: REQ-FUND-006.A4 -->
- [x] When the only code is invalid and there is no other evidence, the value is omitted best-effort with a diagnostic, and the quote endpoint is not called.
<!-- id: REQ-FUND-006.A5 -->
- [x] When the currency resolved through quote enrichment is invalid, `total_revenue` is omitted and an `InvalidCurrency` diagnostic is recorded.
<!-- id: REQ-FUND-006.A6 -->
- [ ] `Some(override_currency)` skips per-value code checks and forces the given currency. [needs review — the only override use is in the caching test, which asserts nothing about currency]

<!-- id: REQ-FUND-007 -->
## Timeseries and quoteSummary error handling, retry and caching

Timeseries errors:
- A non-null `timeseries.error` or `finance.error` becomes `YfError::Api("yahoo error: <description|message|code|raw>")`.
- A missing `timeseries.result` becomes `YfError::MissingData("missing timeseries result")`.
- HTTP failures map through the shared net layer to `ServerError{status,url}` or `RateLimited{url}`, with `crumb=REDACTED` in the URL.
- Error responses are not cached. `validate_timeseries_body` rejects error bodies before they are cached.

quoteSummary fetches (`earnings`, `calendarEvents`) go through `quotesummary::fetch_body(.., caller="fundamentals", ..)`.

Both paths re-fetch the cookie and crumb and retry when the crumb is invalid.

implementations:
  - src/fundamentals/api.rs:reject_timeseries_error
  - src/fundamentals/api.rs:timeseries_error_message
  - src/fundamentals/api.rs:timeseries_results
  - src/fundamentals/api.rs:validate_timeseries_body
  - src/fundamentals/fetch.rs:fetch_modules_body
  - src/fundamentals/fetch.rs:parse_modules

verifies:
  - tests/fundamentals/status_synthetic.rs:quote_summary_http_error_maps_status_and_is_not_cached
  - tests/fundamentals/status_synthetic.rs:quote_summary_api_error_is_not_cached
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_http_error_maps_status_and_is_not_cached
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_api_error_is_not_cached
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_yahoo_error_returns_api_error
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_finance_error_returns_api_error
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_missing_result_is_missing_data
  - tests/fundamentals/retry_synthetic.rs:fundamentals_invalid_crumb_then_retry_succeeds

<!-- id: REQ-FUND-007.A1 -->
- [x] An HTTP 500 from quoteSummary returns `ServerError{status:500}`. The URL contains `/v10/finance/quoteSummary/` and `crumb=REDACTED`, never the real crumb. The next call is not served from cache.
<!-- id: REQ-FUND-007.A2 -->
- [x] An HTTP 429 from timeseries returns `RateLimited` with a redacted URL. The next successful call returns fresh data.
<!-- id: REQ-FUND-007.A3 -->
- [x] A body with a `timeseries.error` or `finance.error` returns `YfError::Api` containing Yahoo's message and is not cached.
<!-- id: REQ-FUND-007.A4 -->
- [x] A body without `timeseries.result` returns `YfError::MissingData`.
<!-- id: REQ-FUND-007.A5 -->
- [x] An invalid-crumb response triggers a cookie fetch, a crumb fetch and one successful retry.

<!-- id: REQ-FUND-008 -->
## Malformed timeseries values are reported without losing valid siblings

`parse_timeseries_values` parses each key's values array element by element:
- If the whole array is malformed, the key is dropped with `DroppedItem{item:"timeseries_item", key}`.
- If one element is malformed, it is recorded as `DroppedItem{item:"timeseries_value", key:"<key>[idx]"}` and the remaining elements are still parsed.
- If `reportedValue` is the wrong type, it becomes `OmittedPresentField{path:"timeseries.reportedValue", key:"<Field>@<ts>"}`.
- A timestamp that cannot be turned into a period drops the row with `DroppedItem{item:"timeseries_row"}`.

implementations:
  - src/fundamentals/api.rs:parse_timeseries_values
  - src/fundamentals/api.rs:parsed_timeseries_value
  - src/fundamentals/api.rs:reported_decimal_at
  - src/fundamentals/api.rs:reported_u64_at
  - src/fundamentals/api.rs:row_for_timestamp

verifies:
  - tests/fundamentals/status_synthetic.rs:malformed_timeseries_value_is_reported_without_dropping_valid_periods

<!-- id: REQ-FUND-008.A1 -->
- [x] When one period's `TotalRevenue` is malformed, only the valid period `2025-01-01` is returned, an `OmittedPresentField` keyed `TotalRevenue@1704067200` is recorded, and strict mode fails with `DataQuality`.
<!-- id: REQ-FUND-008.A2 -->
- [ ] A JSON `null` element in a values array counts as a malformed value. It produces a `DroppedItem` warning, which makes strict mode fail. [needs review — no fixture contains null elements, and it is unclear whether Yahoo sends them; see F1]

<!-- id: REQ-FUND-009 -->
## Earnings history (quoteSummary `earnings` module)

`earnings(override_currency)` fetches `modules=earnings` and builds `Earnings{yearly, quarterly, quarterly_eps}`:
- `yearly`: `Vec<EarningsYear{year, revenue, earnings}>` from `financialsChart.yearly`. `date` must be an i64 year that fits in i32 and passes `EarningsYear::new`.
- `quarterly`: `Vec<EarningsQuarter{period, revenue, earnings}>` from `financialsChart.quarterly`. `date` is a period string such as `"3Q2024"`.
- `quarterly_eps`: `Vec<EarningsQuarterEps{period, actual, estimate}>` from `earningsChart.quarterly`, as `Price` values.

`earnings.financialCurrency` is stored as a currency hint (`store_currency_hints`). The reporting currency is resolved only when monetary values are present (`ReportingCurrencyEvidence::FinancialCurrency`).

If the module is missing or invalid, the call returns `Earnings::default()` with `ProviderFeatureUnavailable{feature:"earnings"}`. Rows missing a required `date` are dropped with a diagnostic.

implementations:
  - src/fundamentals/api.rs:earnings
  - src/fundamentals/api.rs:earnings_has_monetary_values
  - src/fundamentals/api.rs:module_ref
  - src/fundamentals/api.rs:required_i64_from_wire
  - src/fundamentals/api.rs:required_period_from_wire
  - src/fundamentals/wire.rs:EarningsNode

verifies:
  - tests/fundamentals/offline.rs:offline_earnings_uses_recorded_fixture
  - tests/fundamentals/status_synthetic.rs:missing_earnings_module_is_provider_unavailable
  - tests/fundamentals/retry_synthetic.rs:fundamentals_invalid_crumb_then_retry_succeeds

<!-- id: REQ-FUND-009.A1 -->
- [x] When the `earnings` module is missing, all three vectors are empty and the first warning is `ProviderFeatureUnavailable`. Strict mode fails with `DataQuality`.
<!-- id: REQ-FUND-009.A2 -->
- [x] The AMZN fixture produces at least one non-empty earnings collection.
<!-- id: REQ-FUND-009.A3 -->
- [ ] A yearly `date` outside the i32 range, or rejected by `EarningsYear::new`, is dropped with an `InvalidField{field:"date"}` diagnostic. [needs review — no test backs this]
<!-- id: REQ-FUND-009.A4 -->
- [ ] EPS `actual` and `estimate` use the resolved reporting currency, not the trading currency. [needs review — no test backs this, and it is unclear whether Yahoo quotes EPS in the reporting currency]

<!-- id: REQ-FUND-010 -->
## Calendar events (quoteSummary `calendarEvents` module)

`calendar()` fetches `modules=calendarEvents` and builds `Calendar` with these fields:
- `earnings_dates: Vec<DateTime<Utc>>` from `earnings.earningsDate[]`. Each malformed entry is dropped with `DroppedItem{item:"calendar_earnings_date", key:<idx>}`.
- `ex_dividend_date: Option<NaiveDate>` from `exDividendDate`.
- `dividend_payment_date: Option<NaiveDate>` from `dividendDate`.

For the two dividend dates, a missing `raw`, an invalid date or the wrong type records `OmittedPresentField{path:"calendarEvents.<field>"}`. If the module is missing, the call returns an empty `Calendar` with `ProviderFeatureUnavailable{feature:"calendarEvents"}`.

The same projection is reused by `Ticker::info` through the crate-internal `calendar_from_quote_summary_raw`.

implementations:
  - src/fundamentals/api.rs:calendar
  - src/fundamentals/api.rs:calendar_from_quote_summary_raw
  - src/fundamentals/api.rs:map_calendar
  - src/fundamentals/api.rs:calendar_earnings_dates
  - src/fundamentals/api.rs:optional_calendar_date
  - src/fundamentals/mod.rs:calendar_from_quote_summary_raw

verifies:
  - tests/fundamentals/new_fields.rs:calendar_maps_dividend_dates_to_distinct_paft_fields
  - tests/fundamentals/offline.rs:offline_calendar_uses_recorded_fixture
  - tests/fundamentals/status_synthetic.rs:malformed_calendar_dates_are_reported_as_projection_loss
  - tests/fundamentals/status_synthetic.rs:calendar_wrong_type_optional_date_is_diagnostic_not_json
  - tests/fundamentals/status_synthetic.rs:strict_calendar_errors_on_malformed_date

<!-- id: REQ-FUND-010.A1 -->
- [x] `exDividendDate` and `dividendDate` map to the separate fields `ex_dividend_date` and `dividend_payment_date`.
<!-- id: REQ-FUND-010.A2 -->
- [x] A malformed earnings date is dropped while valid ones are kept (1 kept). A malformed ex-dividend date is omitted and the payment date is kept.
<!-- id: REQ-FUND-010.A3 -->
- [x] A wrong-type optional date produces a diagnostic, not a JSON error. Strict mode fails with `DataQuality`.
<!-- id: REQ-FUND-010.A4 -->
- [ ] `Ticker::info` produces the same calendar projection through `calendar_from_quote_summary_raw`. [needs review — no test in scope backs this]

<!-- id: REQ-FUND-011 -->
## Shares outstanding history

`shares(quarterly)` uses the default window. The end is the next UTC midnight, and the start is `SHARE_COUNT_LOOKBACK_DAYS = 548` days before the end. A code comment says this matches Python yfinance's `get_shares_full(start=None, end=None)`.

`shares_between(quarterly, start, end)` uses the exact given timestamps. Passing only `end` defaults the start to end − 548 days. `start >= end` returns `YfError::InvalidDates` before any request is made.

The request type is `annualOrdinarySharesNumber` or `quarterlyOrdinarySharesNumber`, with the same auth, cache and validator settings as statements. The result is built as follows:
- Take the first result item that contains the type key.
- Pair values with timestamps by position.
- Build `ShareCount{date: NaiveDate, shares: u64}` and keep Yahoo's order. There is no sort and no dedup.

A missing item, missing timestamps or a missing key returns `Ok(vec![])`. Malformed timestamps are recorded as `DroppedItem{item:"share_count"}`. A malformed value is recorded as `OmittedPresentField` keyed `<type>@<ts>`, and sibling values are kept.

implementations:
  - src/fundamentals/api.rs:shares
  - src/fundamentals/api.rs:shares_window
  - src/fundamentals/api.rs:reported_share_count_at
  - src/fundamentals/wire.rs:TimeseriesValue

verifies:
  - src/fundamentals/api.rs:shares_window_defaults_start_from_effective_end
  - src/fundamentals/api.rs:shares_window_respects_explicit_start
  - src/fundamentals/api.rs:shares_window_rejects_inverted_range
  - tests/ticker/shares.rs:offline_shares_accepts_timeseries_items_without_meta
  - tests/ticker/shares.rs:offline_shares_uses_recorded_fixture
  - tests/ticker/shares.rs:offline_shares_between_uses_explicit_window
  - tests/ticker/shares.rs:offline_shares_between_rejects_inverted_window
  - tests/fundamentals/status_synthetic.rs:malformed_share_timestamps_are_reported_as_projection_loss
  - tests/fundamentals/status_synthetic.rs:malformed_share_values_do_not_drop_valid_siblings

<!-- id: REQ-FUND-011.A1 -->
- [x] When only `end` is given, the start is end − 548 days. An explicit start is used verbatim.
<!-- id: REQ-FUND-011.A2 -->
- [x] `shares_between(2020-01-01, 2024-01-01)` sends exactly those epoch seconds as `period1` and `period2`.
<!-- id: REQ-FUND-011.A3 -->
- [x] An inverted window returns `YfError::InvalidDates`, both from the unit test and through `Ticker::shares_between`.
<!-- id: REQ-FUND-011.A4 -->
- [x] An item without `meta` gives `[ShareCount{date: 2024-01-01, shares: 100}]`.
<!-- id: REQ-FUND-011.A5 -->
- [x] Malformed timestamps give empty data plus a `DroppedItem{item:"share_count"}` warning. A malformed value keeps its valid sibling (`shares == 100`). Both cases fail in strict mode.
<!-- id: REQ-FUND-011.A6 -->
- [ ] Share counts keep Yahoo's response order, which is presumably ascending and so the reverse of statements' newest-first order. [needs review — no test asserts the order; see F4]

<!-- id: REQ-ANLY-001 -->
## Analysis builder surface, quoteSummary fetch and Yahoo error passthrough

`AnalysisBuilder::new(client, symbol)` has the same option setters as `FundamentalsBuilder`. Public methods, each with a `_with_diagnostics` twin:
- `recommendations() -> Vec<RecommendationRow>`
- `recommendations_summary() -> RecommendationSummary`
- `upgrades_downgrades() -> Vec<UpgradeDowngradeRow>`
- `analyst_price_target(override_currency: Option<Currency>) -> PriceTarget`
- `earnings_trend(override_currency) -> Vec<EarningsTrendRow>`

`Ticker` has wrapper methods with the same names. Each call fetches its modules through `quotesummary::fetch_body(.., caller="analysis", ..)`. A quoteSummary `error` comes back as `YfError::Api("yahoo error: …")`. A non-crumb error is not retried. An invalid crumb triggers a cookie and crumb refresh, then a retry.

implementations:
  - src/analysis/mod.rs:AnalysisBuilder.new
  - src/analysis/mod.rs:AnalysisBuilder.recommendations
  - src/analysis/mod.rs:AnalysisBuilder.recommendations_with_diagnostics
  - src/analysis/mod.rs:AnalysisBuilder.recommendations_summary
  - src/analysis/mod.rs:AnalysisBuilder.recommendations_summary_with_diagnostics
  - src/analysis/mod.rs:AnalysisBuilder.upgrades_downgrades
  - src/analysis/mod.rs:AnalysisBuilder.upgrades_downgrades_with_diagnostics
  - src/analysis/mod.rs:AnalysisBuilder.analyst_price_target
  - src/analysis/mod.rs:AnalysisBuilder.analyst_price_target_with_diagnostics
  - src/analysis/mod.rs:AnalysisBuilder.earnings_trend
  - src/analysis/mod.rs:AnalysisBuilder.earnings_trend_with_diagnostics
  - src/analysis/fetch.rs:fetch_modules_body
  - src/analysis/fetch.rs:parse_modules
  - src/ticker/mod.rs:Ticker.recommendations
  - src/ticker/mod.rs:Ticker.recommendations_summary
  - src/ticker/mod.rs:Ticker.upgrades_downgrades
  - src/ticker/mod.rs:Ticker.analyst_price_target
  - src/ticker/mod.rs:Ticker.earnings_trend

verifies:
  - tests/analysis/yahoo_error_passthrough.rs:analysis_other_yahoo_errors_are_surfaced_without_retry
  - tests/analysis/retry_synthetic.rs:analysis_invalid_crumb_then_retry_succeeds
  - tests/analysis/price_target.rs:price_target_invalid_crumb_then_retry_succeeds

<!-- id: REQ-ANLY-001.A1 -->
- [x] `{"quoteSummary":{"result":null,"error":{"description":"Something broke"}}}` returns `YfError::Api`, whose message contains "yahoo error:" and "Something broke", and no retry happens.
<!-- id: REQ-ANLY-001.A2 -->
- [x] An invalid crumb, then a cookie refresh, then a crumb refresh leads to a successful recommendation summary (mean 2.5, key "buy") and price target (mean 123.45 USD).

<!-- id: REQ-ANLY-002 -->
## Analyst recommendation trend

`recommendations()` fetches `modules=recommendationTrend` and maps each `trend[]` entry to `RecommendationRow{period: ReportingPeriod, strong_buy, buy, hold, sell, strong_sell: Option<u32>}`. Counts are read as i64 and converted to u32 with `optional_u32_from_i64`. Invalid, negative or wrong-type counts become `None` with an `OmittedPresentField` diagnostic on paths like `recommendationTrend.trend[].strongBuy`. An entry without a valid `period` (for example `"0m"`) is dropped with `DroppedItem{item:"recommendation_trend"}`. A missing module or a missing `trend` returns an empty vector with `ProviderFeatureUnavailable`.

implementations:
  - src/analysis/api.rs:recommendation_trend
  - src/analysis/api.rs:recommendation_counts
  - src/analysis/api.rs:required_period_from_wire
  - src/analysis/api.rs:module_ref
  - src/analysis/wire.rs:RecommendationNode

verifies:
  - tests/analysis/offline.rs:offline_recommendations_trend_uses_recorded_fixture
  - tests/analysis/strict_synthetic.rs:missing_recommendation_trend_module_is_provider_unavailable
  - tests/analysis/strict_synthetic.rs:recommendation_trend_missing_period_reports_dropped_row
  - tests/analysis/strict_synthetic.rs:recommendation_counts_report_invalid_present_values
  - tests/analysis/strict_synthetic.rs:recommendation_counts_wrong_type_is_reported_as_projection_loss

<!-- id: REQ-ANLY-002.A1 -->
- [x] The AAPL fixture produces non-empty trend rows.
<!-- id: REQ-ANLY-002.A2 -->
- [x] A missing module gives empty data plus `ProviderFeatureUnavailable`. Strict mode fails with `DataQuality`.
<!-- id: REQ-ANLY-002.A3 -->
- [x] A missing period drops the row with `DroppedItem{item:"recommendation_trend"}`. Strict mode fails.
<!-- id: REQ-ANLY-002.A4 -->
- [x] An invalid or wrong-type `strongBuy` keeps the row with `strong_buy == None` and an `OmittedPresentField` warning. Strict mode fails.

<!-- id: REQ-ANLY-003 -->
## Recommendation summary

`recommendations_summary()` fetches `modules=recommendationTrend,financialData` and builds `RecommendationSummary` with these fields:
- `latest_period`, parsed from `trend[0].period` with `string_to_period`. Missing or invalid values become `None` plus an `OmittedPresentField` warning.
- The five counts from `trend[0]`.
- `mean: Option<Decimal>` from `financialData.recommendationMean.raw`, converted with `optional_decimal_f64`.
- `mean_rating_text: Option<String>` from `financialData.recommendationKey`.

The code treats `trend.first()` as the latest entry. A missing `recommendationTrend` records `ProviderFeatureUnavailable`. A missing `financialData` sets mean and text to `None` and records no unavailable-feature warning. The same mapper serves `Ticker::info` through `price_target_and_recommendation_summary_from_quote_summary_raw`.

implementations:
  - src/analysis/api.rs:recommendation_summary
  - src/analysis/api.rs:map_recommendation_summary
  - src/analysis/api.rs:price_target_and_recommendation_summary_from_quote_summary_raw
  - src/analysis/api.rs:map_price_target_and_recommendation_summary
  - src/analysis/mod.rs:price_target_and_recommendation_summary_from_quote_summary_raw
  - src/analysis/mod.rs:InfoAnalysisParts

verifies:
  - tests/analysis/offline.rs:offline_recommendations_summary_uses_recorded_fixture
  - tests/analysis/retry_synthetic.rs:analysis_invalid_crumb_then_retry_succeeds

<!-- id: REQ-ANLY-003.A1 -->
- [x] The MSFT fixture produces a positive total count and `mean_rating_text == Some("strong_buy")`.
<!-- id: REQ-ANLY-003.A2 -->
- [x] `recommendationMean.raw = 2.5` maps to `mean == decimal_from_f64(2.5)`.
<!-- id: REQ-ANLY-003.A3 -->
- [ ] If `trend[0]` is not the latest period ("0m"), the summary reports a stale period. [needs review — no test backs this, and the code relies on Yahoo's ordering; see F7]

<!-- id: REQ-ANLY-004 -->
## Upgrades and downgrades history

`upgrades_downgrades()` fetches `modules=upgradeDowngradeHistory` and maps each `history[]` entry to `UpgradeDowngradeRow` with these fields:
- `ts: DateTime<Utc>`, from `epochGradeDate`, which is required. A missing or invalid value drops the row with `DroppedItem{item:"upgrade_downgrade"}`.
- `firm: Option<String>`.
- `from_grade` and `to_grade`: `Option<RecommendationGrade>`, parsed with `string_to_recommendation_grade`.
- `action: Option<RecommendationAction>`, parsed from `action`, falling back to `gradeChange` when `action` is empty.

Invalid optional fields become `None` with `OmittedPresentField` warnings, and the row is kept. Rows are sorted ascending by `ts`, oldest first.

implementations:
  - src/analysis/api.rs:upgrades_downgrades
  - src/analysis/api.rs:optional_string_from_wire
  - src/analysis/api.rs:required_i64_from_wire
  - src/analysis/wire.rs:UpgradeNode

verifies:
  - tests/analysis/sorted_upgrades.rs:upgrades_downgrades_are_sorted_by_ts
  - tests/analysis/strict_synthetic.rs:upgrades_downgrades_keep_rows_when_optional_fields_are_invalid
  - tests/analysis/offline.rs:offline_upgrades_downgrades_uses_recorded_fixture

<!-- id: REQ-ANLY-004.A1 -->
- [x] Rows with out-of-order input come back sorted ascending by `ts` (firm "A", then "B").
<!-- id: REQ-ANLY-004.A2 -->
- [x] Invalid `fromGrade`, `action` or `firm` values keep both rows with those fields `None` and record `OmittedPresentField` warnings keyed by firm. Strict mode fails.
<!-- id: REQ-ANLY-004.A3 -->
- [ ] When `action` is empty, `action` is parsed from `gradeChange`. [needs review — no test isolates the fallback]

<!-- id: REQ-ANLY-005 -->
## Analyst price target

`analyst_price_target(override_currency)` fetches `modules=financialData` and builds `PriceTarget{mean, high, low: Option<Price>, number_of_analysts: Option<u32>}` from `targetMeanPrice`, `targetHighPrice`, `targetLowPrice` and `numberOfAnalystOpinions`.

`financialCurrency` is stored as a hint only. The price currency is resolved through `client.resolve_analyst_price_target_currency(symbol, override, options)` with `CurrencyPurpose::AnalystEstimate`, and only when some price is present. If the currency cannot be resolved, present prices are omitted with diagnostics, but the analyst count is kept. A fractional analyst count is rounded and records `CoercedPresentField`. A missing module returns `PriceTarget::default()` with `ProviderFeatureUnavailable`.

implementations:
  - src/analysis/api.rs:analyst_price_target
  - src/analysis/api.rs:map_analyst_price_target
  - src/analysis/wire.rs:FinancialDataNode

verifies:
  - tests/analysis/price_target.rs:offline_price_target_happy
  - tests/analysis/strict_synthetic.rs:missing_price_target_module_is_provider_unavailable
  - tests/analysis/strict_synthetic.rs:price_target_reports_present_prices_when_currency_cannot_be_resolved
  - tests/analysis/strict_synthetic.rs:price_target_uses_quote_currency_enrichment_in_strict_mode
  - tests/analysis/strict_synthetic.rs:price_target_accepts_override_currency_in_strict_mode
  - tests/analysis/strict_synthetic.rs:analyst_count_fractional_rounding_is_diagnostic

<!-- id: REQ-ANLY-005.A1 -->
- [x] The happy path gives mean, high and low of 200, 250 and 150 USD and `number_of_analysts == 31`. The currency comes from quote enrichment.
<!-- id: REQ-ANLY-005.A2 -->
- [x] A missing `financialData` gives `mean == None`, `number_of_analysts == None` and `ProviderFeatureUnavailable`. Strict mode fails.
<!-- id: REQ-ANLY-005.A3 -->
- [x] When the currency cannot be resolved, `mean == None`, `number_of_analysts == Some(7)` and an `OmittedPresentField{path:"financialData.targetMeanPrice"}` warning is recorded.
<!-- id: REQ-ANLY-005.A4 -->
- [x] In strict mode, quote enrichment or an explicit override currency succeeds with empty diagnostics.
<!-- id: REQ-ANLY-005.A5 -->
- [x] `numberOfAnalystOpinions.raw = 12.7` becomes `Some(13)` with a `CoercedPresentField` warning. Strict mode fails.

<!-- id: REQ-ANLY-006 -->
## Earnings trend: estimates, EPS trend and EPS revisions

`earnings_trend(override_currency)` fetches `modules=earningsTrend`. For each `trend[]` item, it first validates `period` (for example `0q`, `+1q`, `0y`, `+1y`). An invalid period drops the item with `DroppedItem{item:"earnings_trend"}` before any currency enrichment happens.

Each valid item becomes an `EarningsTrendRow` with these fields:
- `period`.
- `growth: Option<Decimal>`.
- `earnings_estimate: EarningsEstimate{avg, low, high, year_ago_eps: Option<Price>, num_analysts, growth}`.
- `revenue_estimate: RevenueEstimate{avg, low, high, year_ago_revenue: Option<Money>, num_analysts, growth}`. The wire values are i64.
- `eps_trend: EpsTrend{current: Option<Price>, historical: Vec<TrendPoint>}`, with points for "7d", "30d", "60d" and "90d".
- `eps_revisions: EpsRevisions{historical: Vec<RevisionPoint>}`, with points for "7d" and "30d" taken from `upLast7days`/`downLast7days` and `upLast30days`/`downLast30days`. The aliases `downLast7Days` and `downLast30Days` are also accepted.

Currency is resolved separately for each field group (earnings, revenue, EPS trend) and for each distinct direct currency code (`earningsCurrency`, `revenueCurrency`, `epsTrendCurrency`), through `resolve_analyst_estimate_currency`. A revision point needs both its up and down counts. If only one is present, an `OmittedPresentField` warning names the missing field.

implementations:
  - src/analysis/api.rs:earnings_trend
  - src/analysis/api.rs:AnalystCurrencyResolver.resolve
  - src/analysis/api.rs:resolve_analyst_currency_groups
  - src/analysis/api.rs:currency_group_key
  - src/analysis/api.rs:RawEarningsTrendItem.from_node
  - src/analysis/api.rs:RawEarningsEstimate.from_wire
  - src/analysis/api.rs:RawRevenueEstimate.from_wire
  - src/analysis/api.rs:RawEpsTrend.from_wire
  - src/analysis/api.rs:RawEpsRevisions.from_wire
  - src/analysis/api.rs:project_earnings_trend_row
  - src/analysis/api.rs:project_earnings_estimate
  - src/analysis/api.rs:project_revenue_estimate
  - src/analysis/api.rs:project_eps_trend
  - src/analysis/api.rs:push_eps_trend_point
  - src/analysis/api.rs:project_eps_revisions
  - src/analysis/api.rs:push_revision_point
  - src/analysis/wire.rs:EarningsTrendItemNode
  - src/analysis/wire.rs:EpsRevisionsNode

verifies:
  - tests/analysis/earnings_trend.rs:offline_earnings_trend_uses_recorded_fixture
  - tests/analysis/earnings_trend.rs:earnings_trend_uses_quote_enrichment_without_currency_diagnostic
  - tests/analysis/earnings_trend.rs:earnings_trend_wrong_type_optional_price_is_diagnostic_not_json
  - tests/analysis/earnings_trend.rs:earnings_trend_validates_period_before_currency_enrichment
  - tests/analysis/earnings_trend.rs:earnings_trend_reports_half_present_eps_revision_fields
  - tests/analysis/earnings_trend.rs:earnings_trend_accepts_yahoo_eps_revision_down_days_casing
  - tests/analysis/earnings_trend.rs:earnings_trend_revenue_currency_does_not_poison_reporting_currency_cache
  - tests/analysis/strict_synthetic.rs:missing_earnings_trend_module_is_provider_unavailable
  - tests/analysis/strict_synthetic.rs:earnings_trend_omits_values_when_enriched_currency_is_invalid

<!-- id: REQ-ANLY-006.A1 -->
- [x] The AAPL fixture gives 4 rows with empty diagnostics. The current-year row has an earnings average, a revenue average, a current EPS and non-empty revisions.
<!-- id: REQ-ANLY-006.A2 -->
- [x] Quote enrichment resolves the currency without diagnostics. A second strict call reuses the cached quote: the trend endpoint is called twice and the quote endpoint once.
<!-- id: REQ-ANLY-006.A3 -->
- [x] A wrong-type `earningsEstimate.avg` is omitted with a diagnostic while `low` is kept. Strict mode fails.
<!-- id: REQ-ANLY-006.A4 -->
- [x] An invalid period drops the row before enrichment, so the quote endpoint is called 0 times. Exactly one `DroppedItem{item:"earnings_trend"}` is recorded, and strict mode returns that warning as the `DataQuality` error.
<!-- id: REQ-ANLY-006.A5 -->
- [x] Half-present revision fields leave `historical` empty and record warnings keyed `0q`. Strict mode fails on `upLast7days`.
<!-- id: REQ-ANLY-006.A6 -->
- [x] The `downLast7Days` and `downLast30Days` casing is accepted: 7d has up=2 and down=1, 30d has up=4 and down=3.
<!-- id: REQ-ANLY-006.A7 -->
- [x] For TSCO.L, a USD `revenueCurrency` gives USD revenue estimates, and the reporting-currency cache still resolves GBP afterwards.
<!-- id: REQ-ANLY-006.A8 -->
- [x] When the enriched currency is invalid, `revenue_estimate.avg` is omitted with an `OmittedPresentField` warning.
<!-- id: REQ-ANLY-006.A9 -->
- [x] A missing `earningsTrend` module gives empty data plus `ProviderFeatureUnavailable`. Strict mode fails.
<!-- id: REQ-ANLY-006.A10 -->
- [ ] A `TrendPoint` or `RevisionPoint` construction failure drops the point with no diagnostic. [needs review — no test backs this; see F6]

<!-- id: FUND-NONGOALS -->
## Non-goals

- Statement line items beyond the hard-coded key lists in `income_statement`, `balance_sheet` and `cashflow` are not exposed. Keys the code does not map are ignored (`_ => {}` arms).
- The quoteSummary `incomeStatementHistory` and `incomeStatementHistoryQuarterly` modules are no longer used for statements. Their wire types are marked `#[allow(dead_code)]`, and statements come only from fundamentals-timeseries.
- Statement history is limited to a fixed 5-year window, and the public API has no parameter to change it.
- [fill in — not derivable from code] Whether TTM statements, multi-symbol batch fundamentals, or ESG/holders belong in this spec (they live in other modules).

<!-- id: FUND-OPEN -->
## Open questions

- Is the ascending (oldest-first) order of upgrades/downgrades intentional? Python yfinance shows newest first. [needs review — compatibility intent unclear]
- Should the cash-flow FCF repair fail in strict mode? Right now, strict callers cannot get cash flow at all for any issuer whose payload lacks `FreeCashFlow`.
- Should share counts be sorted and deduplicated so their order matches statements (newest first)?
- Does Yahoo ever return `null` elements, or arrays shorter than `timestamp`, in timeseries values? The positional pairing and strict-mode behaviour depend on the answer.
- Should a missing `financialData` in `recommendations_summary` record `ProviderFeatureUnavailable` for consistency with `analyst_price_target`?
- Is the 548-day share window kept only for Python parity, or is there a product reason? [purpose unclear]

<!-- id: FUND-REVIEW -->
## Code review findings

Ranked by severity.

<!-- id: FUND-REVIEW-HIGH -->
### High

- **F1. Timeseries values are matched to timestamps by array position, and `asOfDate` is ignored.** See `src/fundamentals/api.rs:674-676` and `:736-738` (statements) and `:1506-1508` (shares). If Yahoo sends a values array that is shorter, reordered, or padded with `null`, values are silently assigned to the wrong period. A `null` element also fails `TimeseriesValueDecimal` deserialization at `src/fundamentals/api.rs:508-521`, which emits `DroppedItem` and makes strict mode fail. No fixture contains nulls, so this is untested. [needs review — confirm Yahoo's wire behaviour]

<!-- id: FUND-REVIEW-MED -->
### Medium

- **F2. Strict mode turns the successful FCF inference into a hard error.** The repair is recorded at `src/fundamentals/api.rs:1000-1004`, and `tests/fundamentals/fcf_fallback.rs:91-99` locks the behaviour in. Strict callers get `DataQuality` instead of the cash-flow data for any issuer missing `FreeCashFlow`. This looks like a design smell.
- **F3. Currency hints are stored under an unnormalized symbol.** `client.store_currency_hints(symbol, ..)` and the resolver calls use the builder's raw `symbol` (`src/fundamentals/api.rs:1057`, `:1068`; `src/analysis/api.rs:582`, `:621`, `:752`). The timeseries path uses `normalize_symbol`, which uppercases (`src/fundamentals/api.rs:164`). With `FundamentalsBuilder::new(&c, "aapl")`, hints are cached under `"aapl"` and later read under `"AAPL"` (`src/core/currency_resolver/cache.rs:6,18`), which causes cache misses and extra enrichment requests. [needs review — `Ticker` may normalize first; the builders do not]
- **F4. Share-count output is unordered and not deduplicated.** Statements are returned newest first (`src/fundamentals/api.rs:241`), while `shares()` keeps Yahoo's order and only takes the first result item that contains the key (`src/fundamentals/api.rs:1482-1484`, `:1505-1529`). Callers indexing `[0]` (for example `examples/08_advanced_analysis.rs:37`) may expect the latest value but get the oldest.
- **F5. Revenue estimates are deserialized as `RawNum<i64>`** (`src/analysis/wire.rs:158-165`). A fractional or exponent-form float from Yahoo becomes `Invalid`, so the value is omitted and strict mode fails, whereas the statement path uses `RawDecimal`. [needs review — check what Yahoo actually sends]

<!-- id: FUND-REVIEW-LOW -->
### Low

- **F6. Point construction failures are dropped without a diagnostic.** `TrendPoint::try_new_str` and `RevisionPoint::try_new_str` failures are discarded silently (`src/analysis/api.rs:1314`, `:1399`), which breaks the crate's otherwise lossless-diagnostic pattern. The FCF `try_add` failure at `src/fundamentals/api.rs:998` is also silent.
- **F7. The recommendation summary assumes `trend[0]` is the latest month** (`src/analysis/api.rs:304`). It never checks that `period == "0m"` and never picks the max period.
- **F8. Unavailable-feature reporting is inconsistent.** `map_recommendation_summary` records no `unavailable_feature("financialData")` when the module is missing (`src/analysis/api.rs:349-369`), but `map_analyst_price_target` does (`src/analysis/api.rs:571-573`).
- **F9. Dead code.** `IncomeHistoryNode`, `IncomeRowNode` and the `income_statement_history*` fields of `V10Result` are kept alive with `#[allow(dead_code)]` (`src/fundamentals/wire.rs:14-58`). The fixtures `tests/fixtures/fundamentals_api_incomeStatementHistory*_*.json` may be orphaned.
- **F10. Duplicated helpers.** `module_ref`, `required_i64_from_wire` and `required_period_from_wire` are copied between `src/fundamentals/api.rs:38-105` and `src/analysis/api.rs:71-164`. `shares()` re-implements the fetch config from `fetch_timeseries_body` (`src/fundamentals/api.rs:1453-1477` vs `:248-276`).
- **F11. Weak or missing tests.**
  - The offline fixture tests assert only that results are non-empty (`tests/fundamentals/offline.rs:50,88,126,159,195`; `tests/analysis/offline.rs:37`). `offline_upgrades_downgrades_uses_recorded_fixture` asserts nothing beyond the mock being hit (`tests/analysis/offline.rs:103-105`).
  - No tests cover: quarterly balance sheet and cash flow, statement row ordering, the statement `override_currency` path, `shares_outstanding` on the balance sheet, the `gradeChange` fallback, earnings `date` edge cases, or the `Ticker::info` reuse paths.

<!-- id: FUND-RE-NOTES -->
## Reverse-engineering notes

- **High confidence:** the public API surface, Yahoo endpoints and modules, timeseries keys, field mappings, diagnostics and strict-mode behaviour. All were read directly from source, and most have targeted synthetic tests.
- **Medium confidence:** the ordering semantics (statements newest first, shares in Yahoo order, upgrades ascending) and currency-resolver internals. The resolver lives in `src/core/currency_resolver`, which is outside this scope and was only skimmed.
- **Low confidence:** Yahoo wire edge cases (null elements, value/timestamp misalignment, float revenue estimates) and business intent.
- **Tests were not executed** for this spec. The local toolchain is rustc 1.88.0, and the crate requires `rust-version = "1.91"` (and recent commits target Rust 1.97), so `cargo test` refused to build. `[x]` marks mean "backed by a named test in the repo", not "verified passing locally". [needs review — re-run `cargo test --test fundamentals --test analysis --test ticker` on a supported toolchain]
- Live tests (`tests/fundamentals/live.rs`, `tests/analysis/live.rs`, `tests/analysis/price_target_live.rs`) are `#[ignore]` and are not cited as verification.
- Unit tests inside `src/fundamentals/api.rs` (`mod tests`) are cited in `verifies:` with their source path.
