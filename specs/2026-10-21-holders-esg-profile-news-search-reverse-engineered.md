---
id: INFO-DOC
title: Holders, ESG, Profile, News and Search (reverse-engineered)
owner: unassigned
priority: medium
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/holders/mod.rs
  - src/holders/api.rs
  - src/holders/wire.rs
  - src/holders/model.rs
  - src/esg/mod.rs
  - src/esg/api.rs
  - src/esg/wire.rs
  - src/esg/model.rs
  - src/profile/mod.rs
  - src/profile/api.rs
  - src/profile/model.rs
  - src/profile/debug.rs
  - src/news/mod.rs
  - src/news/api.rs
  - src/news/wire.rs
  - src/news/model.rs
  - src/search/mod.rs
  - src/ticker/mod.rs
  - src/core/conversions.rs
  - src/core/diagnostics/context.rs
  - src/core/diagnostics/monetary.rs
  - src/core/quotesummary.rs
  - tests/holders.rs
  - tests/holders/offline.rs
  - tests/holders/strict_synthetic.rs
  - tests/holders/live.rs
  - tests/esg.rs
  - tests/esg/status_synthetic.rs
  - tests/esg/offline.rs
  - tests/esg/offline_involvement.rs
  - tests/profile.rs
  - tests/profile/api_smoke.rs
  - tests/profile/live.rs
  - tests/ticker/profile.rs
  - tests/ticker/data_quality.rs
  - tests/news.rs
  - tests/news/offline.rs
  - tests/news/live.rs
  - tests/search.rs
  - tests/search/offline.rs
  - tests/search/names.rs
  - tests/search/live.rs
  - tests/fixtures/esg_api_esgScores_MSFT.json
  - examples/02_fundamentals_and_search.rs
  - examples/03_esg_and_analysis.rs
  - examples/09_holders_and_insiders.rs
---

<!-- id: INFO-DOC -->
# Holders, ESG, Profile, News and Search (reverse-engineered)

Intent: the `yfinance-rs` crate (v0.9.1) exposes typed, async accessors for five Yahoo Finance data areas: ownership data (major holders breakdown, institutional and mutual-fund holders, insider transactions, insider roster, net insider share activity), ESG scores and involvement flags, company/ETF/mutual-fund profiles, per-symbol news, and symbol search. Each accessor fetches a Yahoo endpoint and maps the wire JSON onto `paft` domain types. Every call has a plain form that returns data and a `*_with_diagnostics` form that returns `YfResponse<T>`, which carries projection warnings (dropped rows, omitted fields, unavailable provider features). Under `strict()` data-quality mode, any of those warnings becomes `YfError::DataQuality`. This spec describes the behaviour the code shows today. It does not record any product intent beyond that.

<!-- id: REQ-HOLD-001 -->
## Holders builder surface and request shape

`HoldersBuilder::new(client: &YfClient, symbol: impl Into<String>) -> HoldersBuilder` carries `CallOptions` (cache mode, data quality, retry policy), set through the shared `impl_call_option_setters!` macro (`.strict()`, `.cache_mode(..)`, `.data_quality(..)`, `.retry_policy(..)`). It has six capabilities, and each comes as a plain method and a `_with_diagnostics` method:

- `major_holders() -> Result<Vec<MajorHolder>, YfError>`
- `institutional_holders() -> Result<Vec<InstitutionalHolder>, YfError>`
- `mutual_fund_holders() -> Result<Vec<InstitutionalHolder>, YfError>`
- `insider_transactions() -> Result<Vec<InsiderTransaction>, YfError>`
- `insider_roster_holders() -> Result<Vec<InsiderRosterHolder>, YfError>`
- `net_share_purchase_activity() -> Result<Option<NetSharePurchaseActivity>, YfError>`

Each method makes its own `GET /v10/finance/quoteSummary/{symbol}?modules=<one module>&crumb=..` request through `quotesummary::fetch_module_result`, with caller tag `"holders"`. The modules are `majorHoldersBreakdown`, `institutionOwnership`, `fundOwnership`, `insiderTransactions`, `insiderHolders` and `netSharePurchaseActivity`. `Ticker` has a matching convenience method for each one, and those methods pass on the ticker's cache mode, data quality and retry override. The model types are re-exported from `paft::fundamentals::holders`.

implementations:
  - src/holders/mod.rs:HoldersBuilder.new
  - src/holders/mod.rs:HoldersBuilder.major_holders
  - src/holders/mod.rs:HoldersBuilder.institutional_holders
  - src/holders/mod.rs:HoldersBuilder.mutual_fund_holders
  - src/holders/mod.rs:HoldersBuilder.insider_transactions
  - src/holders/mod.rs:HoldersBuilder.insider_roster_holders
  - src/holders/mod.rs:HoldersBuilder.net_share_purchase_activity
  - src/holders/api.rs:fetch_holders_modules
  - src/ticker/mod.rs:Ticker.holders_builder
  - src/ticker/mod.rs:Ticker.major_holders
  - src/ticker/mod.rs:Ticker.institutional_holders
  - src/ticker/mod.rs:Ticker.mutual_fund_holders
  - src/ticker/mod.rs:Ticker.insider_transactions
  - src/ticker/mod.rs:Ticker.insider_roster_holders
  - src/ticker/mod.rs:Ticker.net_share_purchase_activity

verifies:
  - tests/holders/offline.rs:offline_all_holders_from_fixture
  - tests/holders/live.rs:live_holders_smoke_and_or_record

<!-- id: REQ-HOLD-001.A1 -->
- [x] Each of the six `Ticker` holder methods sends exactly one quoteSummary request whose `modules` parameter is the single matching module name and which includes the `crumb` query parameter.
<!-- id: REQ-HOLD-001.A2 -->
- [ ] The `Ticker` convenience methods pass the ticker's `CallOptions` (cache mode, data quality, retry) on to `HoldersBuilder`. [needs review — no test backs this for holders specifically; strict propagation is only tested for news]

<!-- id: REQ-HOLD-002 -->
## Major holders breakdown

`major_holders` reads `majorHoldersBreakdown` and emits up to three `MajorHolder { category: String, value: Ratio }` rows, always in this order:

1. `insidersPercentHeld` → category `"% of Shares Held by All Insiders"`
2. `institutionsPercentHeld` → `"% of Shares Held by Institutions"`
3. `institutionsFloatPercentHeld` → `"% of Float Held by Institutions"`

A field that is absent produces no row. A raw value that cannot be converted to `Ratio` (for example `1e30`) is left out and recorded as an `OmittedPresentField` warning with reason `ConversionFailed { target: "major holder percent" }`. Other module fields, such as `institutionsCount`, are ignored.

implementations:
  - src/holders/api.rs:major_holders

verifies:
  - tests/holders/offline.rs:offline_all_holders_from_fixture
  - tests/holders/strict_synthetic.rs:major_holder_decimal_conversion_failure_is_reported

<!-- id: REQ-HOLD-002.A1 -->
- [x] With the recorded AAPL fixture, the result contains categories containing "Held by All Insider" and "Held by Institutions".
<!-- id: REQ-HOLD-002.A2 -->
- [x] An unconvertible `insidersPercentHeld` is omitted with an `OmittedPresentField { path: "majorHoldersBreakdown.insidersPercentHeld", reason: ConversionFailed{target:"major holder percent"} }` warning, and the valid institutions row is still returned (1 row).
<!-- id: REQ-HOLD-002.A3 -->
- [ ] A missing `majorHoldersBreakdown` module returns an empty Vec plus a `ProviderFeatureUnavailable` warning (an error under strict). [needs review — no test backs this for the major-holders module; the analogous behaviour is tested for institutionOwnership]

<!-- id: REQ-HOLD-003 -->
## Institutional and mutual-fund holder lists

`institutional_holders` and `mutual_fund_holders` share `map_ownership_list`. They read `institutionOwnership.ownershipList` and `fundOwnership.ownershipList` respectively. Each list entry becomes an `InstitutionalHolder { holder, shares: Option<u64>, date_reported, pct_held: Option<Ratio>, value: Option<Money> }`.

- Required: `organization` (non-empty string) and `reportDate` (a wire date). A row missing either one is dropped with a `DroppedItem` warning (`item: "institutional_holder"`).
- Optional: `position` → `shares`, `pctHeld` → `pct_held` (converted to a ratio), `value` → `value` (money, see REQ-HOLD-004). A malformed optional raw value is omitted, the row is kept, and a warning is recorded.
- An entry that fails to deserialize is dropped with `InvalidField { field: "holder" }`. The diagnostic key is the trimmed `organization` if there is one, otherwise `ownershipList[idx]`.
- If the module or its `ownershipList` is missing, the method returns an empty Vec and reports `ProviderFeatureUnavailable` for `institutionOwnership` / `fundOwnership`, or for `<module>.ownershipList` when only the list is missing.

implementations:
  - src/holders/api.rs:institutional_holders
  - src/holders/api.rs:mutual_fund_holders
  - src/holders/api.rs:map_ownership_list
  - src/holders/api.rs:ownership_list_or_unavailable
  - src/holders/api.rs:holder_diag_key

verifies:
  - tests/holders/offline.rs:offline_all_holders_from_fixture
  - tests/holders/strict_synthetic.rs:missing_institution_ownership_module_is_provider_unavailable
  - tests/holders/strict_synthetic.rs:missing_ownership_list_is_provider_unavailable
  - tests/holders/strict_synthetic.rs:malformed_optional_holder_raw_field_is_omitted_without_dropping_row

<!-- id: REQ-HOLD-003.A1 -->
- [x] AAPL fixture: the institutional holder list is non-empty and the first row has `shares > 0`. The mutual-fund list is non-empty and the first row has a positive `value`.
<!-- id: REQ-HOLD-003.A2 -->
- [x] An empty `result[{}]` returns empty data plus `ProviderFeatureUnavailable { feature: "institutionOwnership" }`. The same call with `.strict()` returns `YfError::DataQuality`.
<!-- id: REQ-HOLD-003.A3 -->
- [x] A missing `ownershipList` is reported as provider-unavailable in lenient mode and is a `DataQuality` error in strict mode.
<!-- id: REQ-HOLD-003.A4 -->
- [x] A row with `position.raw = "not-a-number"` is kept with `shares = None`, the valid sibling row keeps `shares = Some(10)`, and strict mode rejects the response.
<!-- id: REQ-HOLD-003.A5 -->
- [ ] A row missing `organization` or `reportDate` is dropped with a `DroppedItem` warning. [needs review — no test backs this]

<!-- id: REQ-HOLD-004 -->
## Holder and insider monetary values use the trading currency

When at least one row has a non-null `value.raw`, and only then, the code resolves the symbol's trading currency once per call through `YfClient::resolve_trading_currency(symbol, None, TradingCurrencyEvidence::None, ..)`. In practice that means a `/v7/finance/quote` lookup. The amount is expressed in the currency's **major unit**: GBp maps to GBP, ZAc to ZAR, ILA to ILS. The trading currency is used even when it differs from `financialCurrency`. If the currency cannot be resolved, `value` is `None` and the row is kept, with an `OmittedPresentField { path: "ownershipList[].value", reason: CurrencyUnresolved }` warning. In strict mode, an invalid enriched currency code (for example `"!!!"`) gives `DataQuality`. This applies to both ownership lists and to insider transactions.

implementations:
  - src/holders/api.rs:optional_holder_value_currency
  - src/holders/api.rs:resolve_trading_currency
  - src/holders/api.rs:raw_field_present

verifies:
  - tests/holders/offline.rs:recorded_holder_values_use_trading_major_currency
  - tests/holders/offline.rs:recorded_insider_transaction_values_use_trading_major_currency
  - tests/holders/strict_synthetic.rs:optional_holder_value_is_omitted_when_currency_cannot_be_resolved
  - tests/holders/strict_synthetic.rs:holder_diagnostics_report_present_value_with_unresolved_currency
  - tests/holders/strict_synthetic.rs:strict_holder_value_rejects_invalid_enriched_currency_as_data_quality
  - tests/holders/strict_synthetic.rs:insider_transaction_value_uses_trading_currency_when_financial_currency_differs

<!-- id: REQ-HOLD-004.A1 -->
- [x] SAP (quote currency USD, financial currency EUR) holder values are in USD. TSCO.L (GBp) holder values are in GBP. The amount equals the raw value.
<!-- id: REQ-HOLD-004.A2 -->
- [x] AAPL insider transaction values are in USD and the amount equals the raw value.
<!-- id: REQ-HOLD-004.A3 -->
- [x] If the currency is unresolvable, `value` is `None`, `shares` is kept, and a `CurrencyUnresolved` warning is recorded.
<!-- id: REQ-HOLD-004.A4 -->
- [x] A strict call with an invalid enriched currency code returns `YfError::DataQuality`.
<!-- id: REQ-HOLD-004.A5 -->
- [ ] No currency lookup is made when no row has a raw value. [needs review — no test asserts the quote endpoint is not called]
<!-- id: REQ-HOLD-004.A6 -->
- [ ] Amounts in a minor-unit currency (GBp) are passed through without division by 100. [needs review — the test asserts amount == raw value with currency GBP; it is unclear whether Yahoo reports holder values in GBP or GBp]

<!-- id: REQ-HOLD-005 -->
## Insider transactions

`insider_transactions` reads `insiderTransactions.transactions[]` into `InsiderTransaction { insider, position: InsiderPosition, transaction_type: TransactionType, shares: Option<u64>, value: Option<Money>, transaction_date, url: Option<String> }`.

- Required: `filerName` (non-empty), `filerRelation` (parsed through `string_to_insider_position`), `transactionText` (parsed through `string_to_transaction_type`), and `startDate`. A row that fails any of these is dropped with a warning (`item: "insider_transaction"`).
- **Blank-exercise inference.** If `transactionText` is missing or blank, `shares.raw > 0`, and `value.raw` is missing, the row is classified as `TransactionType::Exercise` and gets no warning. [purpose unclear — the code appears to treat these Yahoo rows as option exercises; the business basis is not documented]
- `filerUrl` maps to `url`, and an empty string becomes `None`.
- A missing module or `transactions` list gives an empty Vec with `ProviderFeatureUnavailable` (`insiderTransactions` / `insiderTransactions.transactions`).

implementations:
  - src/holders/api.rs:insider_transactions
  - src/holders/api.rs:infer_blank_insider_transaction_type
  - src/core/conversions.rs:string_to_insider_position
  - src/core/conversions.rs:string_to_transaction_type

verifies:
  - tests/holders/offline.rs:recorded_blank_insider_transactions_are_preserved_without_drop_diagnostics
  - tests/holders/strict_synthetic.rs:blank_no_cash_insider_transaction_is_inferred_as_exercise
  - tests/holders/strict_synthetic.rs:insider_transaction_value_uses_trading_currency_when_financial_currency_differs

<!-- id: REQ-HOLD-005.A1 -->
- [x] AAPL fixture: every recorded row is returned (`data.len()` == fixture row count), with no `DroppedItem { reason: MissingRequiredField{field:"transaction"} }` warnings. The number of blank-text rows inferred as `Exercise` with `value == None` is at least the number of blank rows.
<!-- id: REQ-HOLD-005.A2 -->
- [x] A synthetic blank-text row with `shares = 131576` and no value maps to `Exercise`, `shares = Some(131576)`, `value = None` and `url = None`, with empty diagnostics.
<!-- id: REQ-HOLD-005.A3 -->
- [ ] A row with an unparseable `filerRelation` or a non-blank but unrecognised `transactionText` is dropped with a warning. [needs review — no test backs this]

<!-- id: REQ-HOLD-006 -->
## Insider roster

`insider_roster_holders` reads `insiderHolders.holders[]` into `InsiderRosterHolder { name, position: InsiderPosition, most_recent_transaction: TransactionType, latest_transaction_date, shares_owned_directly: Option<u64>, position_direct_date }`. The fields `name`, `relation`, `transactionDescription`, `latestTransDate` and `positionDirectDate` are all **required**. A row missing any of them is dropped (`item: "insider_roster_holder"`). Positions are never filled with a default value. `positionDirect` is optional. A missing module or list gives an empty Vec with `ProviderFeatureUnavailable`.

implementations:
  - src/holders/api.rs:insider_roster_holders

verifies:
  - tests/holders/offline.rs:offline_all_holders_from_fixture
  - tests/holders/strict_synthetic.rs:insider_roster_missing_position_is_not_defaulted_to_officer

<!-- id: REQ-HOLD-006.A1 -->
- [x] The AAPL fixture roster is non-empty and contains a name containing "cook" (case-insensitive).
<!-- id: REQ-HOLD-006.A2 -->
- [x] A roster row with no `relation` is dropped rather than given a default position (the result is empty, and lenient mode does not error).
<!-- id: REQ-HOLD-006.A3 -->
- [ ] A row missing `transactionDescription` or either date is dropped. [needs review — no test backs this; it may drop legitimate roster entries, see the review findings]

<!-- id: REQ-HOLD-007 -->
## Net share purchase activity

`net_share_purchase_activity` returns `Option<NetSharePurchaseActivity>`. `period` is required and is parsed through `string_to_period` (for example `"6m"`). If it is missing or invalid, the method returns `None` with `DroppedItem { item: "net_share_purchase_activity", reason: MissingRequiredField{field:"period"} }`, which becomes an error under strict. These fields are optional raw values: `buyInfoShares`/`buyInfoCount` → `buy_shares`/`buy_count`, `sellInfoShares`/`sellInfoCount`, `netInfoShares`/`netInfoCount` (i64), `totalInsiderShares`, and `netPercentInsiderShares`. The last one is converted to `Decimal` through `optional_decimal_f64`, not to `Ratio`. A missing module gives `None` plus `ProviderFeatureUnavailable("netSharePurchaseActivity")`.

implementations:
  - src/holders/api.rs:net_share_purchase_activity
  - src/core/conversions.rs:string_to_period

verifies:
  - tests/holders/offline.rs:offline_all_holders_from_fixture
  - tests/holders/strict_synthetic.rs:net_share_purchase_activity_missing_period_is_reported
  - tests/holders/strict_synthetic.rs:strict_net_share_purchase_activity_errors_on_missing_period
  - tests/holders/strict_synthetic.rs:net_share_purchase_activity_percent_conversion_failure_is_reported

<!-- id: REQ-HOLD-007.A1 -->
- [x] AAPL fixture: returns `Some`, with a non-empty period string and `total_insider_shares > 0`.
<!-- id: REQ-HOLD-007.A2 -->
- [x] A missing period gives `data == None` with a `DroppedItem` warning in lenient mode and `YfError::DataQuality` in strict mode.
<!-- id: REQ-HOLD-007.A3 -->
- [x] `netPercentInsiderShares = 1e30` is omitted with `ConversionFailed { target: "net percent insider shares" }`, and the activity is kept (`buy_shares == Some(10)`).

<!-- id: REQ-ESG-001 -->
## ESG scores

`EsgBuilder::new(client, symbol)` (with call-option setters) exposes `fetch() -> Result<EsgSummary, YfError>` and `fetch_with_diagnostics()`. `Ticker::sustainability()` and `Ticker::sustainability_with_diagnostics()` delegate to it. The request is `GET /v10/finance/quoteSummary/{symbol}?modules=esgScores` with caller tag `"esg"`. Only `environmentScore`, `socialScore` and `governanceScore` are mapped, into `EsgScores { environmental, social, governance }: Option<Decimal>`. `totalEsg`, `percentile` and `highestControversy` are deserialized but discarded (`#[allow(dead_code)]`). When the module is present, `scores` is always `Some(..)`, even if all three values are `None`. A malformed raw score is omitted with an `OmittedPresentField { path: "esgScores.<field>", key: Some(symbol), reason: InvalidField }` warning.

implementations:
  - src/esg/mod.rs:EsgBuilder.new
  - src/esg/mod.rs:EsgBuilder.fetch
  - src/esg/mod.rs:EsgBuilder.fetch_with_diagnostics
  - src/esg/api.rs:fetch_esg_scores
  - src/esg/api.rs:map_esg_scores
  - src/ticker/mod.rs:Ticker.sustainability
  - src/ticker/mod.rs:Ticker.sustainability_with_diagnostics

verifies:
  - tests/esg/status_synthetic.rs:malformed_esg_raw_score_is_omitted_without_json_error

<!-- id: REQ-ESG-001.A1 -->
- [x] `environmentScore.raw = "not-a-number"` leaves `environmental = None`, while `social` and `governance` stay `Some`. An `OmittedPresentField` warning keyed by the symbol is recorded, and strict mode errors with `DataQuality`.
<!-- id: REQ-ESG-001.A2 -->
- [ ] Valid recorded ESG fixture data maps to E/S/G scores. [needs review — the offline fixture test is disabled in tests/esg.rs, and the recorded fixture `esg_api_esgScores_MSFT.json` is a "Not Found" response, so there is no positive-path coverage]

<!-- id: REQ-ESG-002 -->
## ESG involvement flags

The boolean product-involvement flags in `esgScores` become `EsgInvolvement { category, score: None }` entries. An entry is emitted only when the flag is `true`, and entries come in this fixed order: `adult, alcoholic, animal_testing, catholic, controversial_weapons, small_arms, fur_leather, gambling, gmo, military_contract, nuclear, palm_oil, pesticides, thermal_coal, tobacco`. The wire key `coal` is renamed `thermal_coal`.

implementations:
  - src/esg/api.rs:map_esg_scores

<!-- id: REQ-ESG-002.A1 -->
- [ ] Only flags set to `true` produce involvement entries, using the snake_case category names listed above. [needs review — no test backs this; `tests/esg/offline_involvement.rs` exists but is commented out in tests/esg.rs]
<!-- id: REQ-ESG-002.A2 -->
- [ ] A flag of non-boolean type causes the whole ESG response to fail to deserialize, rather than being omitted. [needs review — inferred from `Option<bool>` wire fields that are not wrapped in `WireValue`; untested]

<!-- id: REQ-ESG-003 -->
## ESG error and unavailability handling

If Yahoo returns HTTP 404 or a 200 body with `quoteSummary.error` ("No fundamentals data found"), the result is an error whose message contains "Not found" or the Yahoo description. A `result[{}]` with no `esgScores` returns `EsgSummary { scores: None, involvement: [] }` plus `ProviderFeatureUnavailable { feature: "esgScores" }`, which is a `DataQuality` error under strict.

implementations:
  - src/esg/api.rs:map_esg_scores
  - src/core/quotesummary.rs:fetch_module_result

verifies:
  - tests/esg/status_synthetic.rs:esg_http_not_found_returns_error
  - tests/esg/status_synthetic.rs:esg_not_found_body_returns_error
  - tests/esg/status_synthetic.rs:missing_esg_module_is_reported_as_unavailable

<!-- id: REQ-ESG-003.A1 -->
- [x] HTTP 404 returns an error whose `to_string()` contains "Not found".
<!-- id: REQ-ESG-003.A2 -->
- [x] A 200 response with a quoteSummary error body returns an error containing "No fundamentals data found".
<!-- id: REQ-ESG-003.A3 -->
- [x] A missing `esgScores` module gives `scores == None` with a `ProviderFeatureUnavailable` warning in lenient mode and `DataQuality` in strict mode.

<!-- id: REQ-PROF-001 -->
## Profile loading and kind dispatch

Public entry points:

- `profile::load_profile(client, symbol) -> Result<Profile, YfError>`
- `profile::load_profile_with_diagnostics(client, symbol) -> Result<YfResponse<Profile>, YfError>`. Both free functions force `CacheMode::Use`.
- `Ticker::profile()` and `Ticker::profile_with_diagnostics()`, which use the ticker's options.

The symbol is normalised with `normalize_symbol`, then the code makes one `GET /v10/finance/quoteSummary/{symbol}?modules=assetProfile,quoteType,fundProfile` call with caller tag `"profile"`. `quoteType.quoteType` picks the variant: `EQUITY` → `Profile::Company`, and `ETF` or `MUTUALFUND` → `Profile::Fund`. Any other value (for example `INDEX` or `CRYPTOCURRENCY`) gives `YfError::InvalidData("profile unavailable for Yahoo quoteType <X>")`. A missing `quoteType` module or field gives `YfError::MissingData`. `Profile.name` is `longName`, falling back to `shortName`, then to the symbol. `Ticker::info` also builds the profile from a shared quoteSummary body through `load_profile_from_quote_summary_raw`, which skips symbol normalisation.

implementations:
  - src/profile/mod.rs:load_profile
  - src/profile/mod.rs:load_profile_with_diagnostics
  - src/profile/mod.rs:load_profile_with_options
  - src/profile/mod.rs:load_profile_with_options_and_diagnostics
  - src/profile/mod.rs:load_profile_from_quote_summary_raw
  - src/profile/mod.rs:YahooProfileKind.from_quote_type
  - src/profile/api.rs:load_from_quote_summary_api_with_diagnostics
  - src/profile/api.rs:load_from_quote_summary_result_with_diagnostics
  - src/profile/api.rs:profile_base
  - src/ticker/mod.rs:Ticker.profile
  - src/ticker/mod.rs:Ticker.profile_with_diagnostics

verifies:
  - tests/profile/api_smoke.rs:profile_api_company_happy
  - tests/profile/api_smoke.rs:profile_api_unsupported_quote_type_is_provider_data_error
  - tests/ticker/profile.rs:offline_profile_uses_recorded_fixture
  - tests/ticker/profile.rs:live_profile_unsupported_quote_types_are_provider_data_errors

<!-- id: REQ-PROF-001.A1 -->
- [x] The request uses `modules=assetProfile,quoteType,fundProfile` and includes the crumb.
<!-- id: REQ-PROF-001.A2 -->
- [x] `^GSPC` (INDEX) and `BTC-USD` (CRYPTOCURRENCY) fail with `YfError::InvalidData` whose message contains "profile unavailable" and the quote type.
<!-- id: REQ-PROF-001.A3 -->
- [ ] The name falls back from `longName` to `shortName` to the symbol. [needs review — no test exercises the fallbacks]
<!-- id: REQ-PROF-001.A4 -->
- [ ] A missing `quoteType` module returns `YfError::MissingData("quoteType missing")`. [needs review — no test backs this]

<!-- id: REQ-PROF-002 -->
## Company profile

For `EQUITY`, the `assetProfile` module is required; if it is missing the call fails with `YfError::MissingData("assetProfile missing")`. The result is `Profile::Company(Company { name, sector, industry, website, summary (longBusinessSummary), address: Some(Address{street1, street2, city, state, country, zip}), isin: Option<Isin> })`. A malformed or wrong-type optional string field is omitted with an `OmittedPresentField` warning, and the profile still succeeds in lenient mode. An invalid ISIN is omitted with `InvalidField { field: "isin" }`. Side effect: `client.store_currency_hints(symbol, CurrencyHints::from_profile(country, exchange, "EQUITY"))` seeds currency resolution, and other features use those hints later.

implementations:
  - src/profile/api.rs:map_company_profile
  - src/profile/api.rs:map_address
  - src/profile/api.rs:optional_isin
  - src/profile/api.rs:module_ref

verifies:
  - tests/profile/api_smoke.rs:profile_api_company_happy
  - tests/profile/api_smoke.rs:profile_api_wrong_type_optional_field_does_not_fail_profile
  - tests/ticker/profile.rs:offline_profile_uses_recorded_fixture
  - tests/ticker/profile.rs:strict_profile_rejects_malformed_optional_field

<!-- id: REQ-PROF-002.A1 -->
- [x] AAPL fixture: name "Apple Inc.", sector "Technology", industry "Consumer Electronics", website "https://www.apple.com".
<!-- id: REQ-PROF-002.A2 -->
- [x] `assetProfile.sector = 42` gives `sector == None` with an `OmittedPresentField { endpoint:"profile", path:"assetProfile.sector", key: Some(symbol), reason: InvalidField{field:"sector", details ~ "expected string"} }` warning, and the other fields are kept.
<!-- id: REQ-PROF-002.A3 -->
- [x] The same malformed field through `Ticker::new(..).strict().profile()` returns a `DataQuality` error.
<!-- id: REQ-PROF-002.A4 -->
- [ ] Currency hints are stored from the profile's country and exchange. [needs review — no test in scope asserts this side effect]

<!-- id: REQ-PROF-003 -->
## Fund (ETF / mutual fund) profile

For `ETF` or `MUTUALFUND`, the `fundProfile` module is required (`MissingData("fundProfile missing")` if absent). The result is `Profile::Fund(Fund { name, family, kind: FundKind, isin })`. `kind` comes from `fundProfile.legalType` through `string_to_fund_kind`, which maps "Exchange Traded Fund", "Mutual Fund", "Index Fund", "Closed-End Fund", "Money Market Fund", "Hedge Fund", "Real Estate Investment Trust" and "Unit Investment Trust", and parses anything else as a token. If `legalType` is absent, the quote type decides: `ETF` → `FundKind::Etf`, `MUTUALFUND` → `FundKind::MutualFund`. Currency hints are stored with no country.

implementations:
  - src/profile/api.rs:map_fund_profile
  - src/profile/mod.rs:resolve_fund_kind
  - src/profile/mod.rs:FundQuoteKind.fund_kind
  - src/core/conversions.rs:string_to_fund_kind

verifies:
  - tests/profile/api_smoke.rs:profile_api_fund_happy
  - tests/profile/api_smoke.rs:profile_api_synthetic_etf_missing_legal_type_uses_quote_type
  - tests/profile/api_smoke.rs:profile_api_mutual_fund_happy

<!-- id: REQ-PROF-003.A1 -->
- [x] QQQ fixture: name "Invesco QQQ Trust", family "Invesco", kind "ETF".
<!-- id: REQ-PROF-003.A2 -->
- [x] A synthetic ETF with no `legalType` resolves to kind "ETF".
<!-- id: REQ-PROF-003.A3 -->
- [x] Mutual-fund fixture: name "Vanguard Total Stock Mkt Idx Adm", family "Vanguard", kind "MUTUAL_FUND".
<!-- id: REQ-PROF-003.A4 -->
- [ ] A blank or unparseable `legalType` makes the whole profile call fail (`?` in `resolve_fund_kind`) instead of falling back with a warning. [needs review — no test backs this; it may be unintended]

<!-- id: REQ-NEWS-001 -->
## News request, tabs and caching

`NewsBuilder::new(client, symbol)` defaults to `count = 10` and `tab = NewsTab::News`. Builder methods: `.count(u32)`, `.tab(NewsTab)`, plus the call-option setters. `NewsTab` is `#[non_exhaustive]` and maps to a `queryRef`: `News` → `latestNews`, `All` → `newsAll`, `PressReleases` → `pressRelease`. `fetch()` and `fetch_with_diagnostics()` normalise the symbol, then send `POST {base_news}/xhr/ncp?queryRef=<tab>&serviceKey=ncp_fin` with the JSON body `{"serviceConfig":{"snippetCount":<count>,"s":[<symbol>]}}`. Responses are cached under `CacheEndpoint::News`, and the cache key includes the body. With the default cache mode the response cache is bypassed. With an explicit `CacheMode::Use`, identical requests are served from cache and different bodies are not. `Ticker::news()`, `news_with_diagnostics()` and `news_builder()` pass on the ticker's options.

implementations:
  - src/news/mod.rs:NewsBuilder.new
  - src/news/mod.rs:NewsBuilder.count
  - src/news/mod.rs:NewsBuilder.tab
  - src/news/mod.rs:NewsBuilder.fetch
  - src/news/mod.rs:NewsBuilder.fetch_with_diagnostics
  - src/news/mod.rs:tab_as_str
  - src/news/api.rs:fetch_news
  - src/ticker/mod.rs:Ticker.news_builder
  - src/ticker/mod.rs:Ticker.news
  - src/ticker/mod.rs:Ticker.news_with_diagnostics

verifies:
  - tests/news/offline.rs:offline_news_uses_recorded_fixture
  - tests/news/offline.rs:offline_news_builder_configures_request
  - tests/news/offline.rs:default_news_cache_mode_bypasses_response_cache
  - tests/news/offline.rs:explicit_news_cache_mode_uses_body_aware_response_cache
  - tests/ticker/data_quality.rs:ticker_strict_propagates_to_builder_backed_convenience_methods

<!-- id: REQ-NEWS-001.A1 -->
- [x] The default request is a POST to `/xhr/ncp` with `queryRef=latestNews`, `serviceKey=ncp_fin` and body `snippetCount: 10, s: [sym]`.
<!-- id: REQ-NEWS-001.A2 -->
- [x] `.count(5).tab(NewsTab::PressReleases)` sends `queryRef=pressRelease` with `snippetCount: 5`.
<!-- id: REQ-NEWS-001.A3 -->
- [x] With a client cache TTL configured, two default `ticker.news()` calls hit the server twice.
<!-- id: REQ-NEWS-001.A4 -->
- [x] With `CacheMode::Use`, the sequence count 10 → count 5 → count 10 makes exactly one server call per distinct body.
<!-- id: REQ-NEWS-001.A5 -->
- [x] `Ticker::new(..).strict().news()` returns `DataQuality` for a stream item with no `pubDate`.
<!-- id: REQ-NEWS-001.A6 -->
- [ ] `NewsTab::All` maps to `queryRef=newsAll`. [needs review — no test backs this]

<!-- id: REQ-NEWS-002 -->
## News article projection

Items in `data.tickerStream.stream[]` become `NewsArticle { uuid, title, publisher: Option<String>, link: Option<String>, published_at }`.

- Required: `id` (a string, trimmed, non-empty), `content`, `content.title`, and `content.pubDate` (RFC 3339, converted to a UTC datetime). An item that fails any of these is dropped with a `DroppedItem { endpoint: "news", item: "news_article" }` warning. The key is the trimmed id, or `stream[idx]` when there is none.
- Optional: `content.provider.displayName` → `publisher`, and `content.canonicalUrl.url` → `link`. A malformed value is omitted with an `OmittedPresentField` warning keyed by the article id.
- An item with a non-null `ad` field is dropped with reason `ProviderUnavailable { feature: "article" }`.
- If `data`, `tickerStream` or `stream` is missing, the result is an empty Vec with **no** diagnostic.

implementations:
  - src/news/api.rs:fetch_news
  - src/news/api.rs:news_stream_item_diag_key

verifies:
  - tests/news/offline.rs:offline_news_uses_recorded_fixture
  - tests/news/offline.rs:malformed_news_stream_items_are_dropped_with_diagnostics

<!-- id: REQ-NEWS-002.A1 -->
- [x] AAPL fixture: the result is non-empty, and the first article has a non-empty uuid and title and a positive timestamp.
<!-- id: REQ-NEWS-002.A2 -->
- [x] A numeric id is dropped with `InvalidField{field:"id"}` keyed `stream[0]`. A missing id is dropped with `MissingRequiredField{field:"id"}` keyed `stream[1]`.
<!-- id: REQ-NEWS-002.A3 -->
- [x] A valid item with a numeric `displayName` or `url` is kept, with `publisher`/`link == None` and `OmittedPresentField` warnings on `provider.displayName` and `canonicalUrl.url` keyed by the id. The same payload in strict mode returns `DataQuality`.
<!-- id: REQ-NEWS-002.A4 -->
- [ ] Ad items are dropped. In strict mode, a single ad fails the whole fetch. [needs review — no test backs this; the strict-mode consequence looks unintended]
<!-- id: REQ-NEWS-002.A5 -->
- [ ] A missing `tickerStream` returns `Ok(vec![])` silently. [needs review — no test; inconsistent with how holders and ESG report unavailable features]

<!-- id: REQ-SRCH-001 -->
## Search request, parameters and authentication

The free function `search(client, query) -> Result<SearchResponse, YfError>` is a shortcut for `SearchBuilder::new(client, query).fetch()`. `SearchBuilder` defaults to `quotes_count = Some(10)` and to base URL `https://query2.finance.yahoo.com/v1/finance/search`, which is hard-coded per builder and can be overridden with `.search_base(Url)`, documented as for testing. Builder methods are `.quotes_count(u32)`, `.lang(..)` and `.region(..)`, with getters `lang_ref()` and `region_ref()`, plus the call-option setters. The query is trimmed. An empty or whitespace-only query gives `YfError::InvalidParams("search query cannot be empty")` before any request is sent. The request is `GET ?q=<query>&quotesCount=<n>[&lang=..][&region=..]`. `newsCount` and `listsCount` are not sent. Authentication is `AuthMode::OptionalCrumb`: cached cookie and crumb credentials are tried first, a 403 triggers a crumb refresh and a retry, and an invalid-crumb body is retried. Responses are cached under `CacheEndpoint::Search`, and the cache validator rejects any body that has no `quotes`.

implementations:
  - src/search/mod.rs:search
  - src/search/mod.rs:SearchBuilder.new
  - src/search/mod.rs:SearchBuilder.search_base
  - src/search/mod.rs:SearchBuilder.quotes_count
  - src/search/mod.rs:SearchBuilder.lang
  - src/search/mod.rs:SearchBuilder.region
  - src/search/mod.rs:SearchBuilder.lang_ref
  - src/search/mod.rs:SearchBuilder.region_ref
  - src/search/mod.rs:SearchBuilder.fetch
  - src/search/mod.rs:SearchBuilder.fetch_with_diagnostics
  - src/search/mod.rs:SearchBuilder.validate_query
  - src/search/mod.rs:SearchBuilder.append_query_params
  - src/search/mod.rs:validate_search_body

verifies:
  - tests/search/offline.rs:offline_search_uses_recorded_fixture
  - tests/search/offline.rs:search_rejects_empty_query_before_request
  - tests/search/offline.rs:search_403_with_stale_cached_crumb_refreshes_before_retry
  - tests/search/live.rs:live_search_smoke_and_or_record

<!-- id: REQ-SRCH-001.A1 -->
- [x] The request sends `q` and `quotesCount=10` and does not send `newsCount` or `listsCount`. The "apple" fixture result includes `AAPL`.
<!-- id: REQ-SRCH-001.A2 -->
- [x] `""` and `" \t\n "` both return `InvalidParams` mentioning "query", and no HTTP call is made.
<!-- id: REQ-SRCH-001.A3 -->
- [x] With a stale cached crumb, the stale-crumb request returns 403, the cookie and crumb are refreshed, and the retry with the new crumb succeeds. A bare request with no crumb is never sent.
<!-- id: REQ-SRCH-001.A4 -->
- [ ] `lang` and `region` are appended only when set. [needs review — no test backs this]
<!-- id: REQ-SRCH-001.A5 -->
- [ ] A response with no `quotes` gives `YfError::MissingData("search quotes missing")`. [needs review — no test backs this]

<!-- id: REQ-SRCH-002 -->
## Search result projection

Each element of `quotes[]` becomes `SearchResult { instrument: Instrument, name: Option<String> }`.

- `symbol` is required, trimmed and non-empty. `quoteType` is required and parsed with `parse_yahoo_quote_type`. A missing or invalid value of either drops the result with `DroppedItem { endpoint: "search", item: "search_result" }`. The key is the symbol, or `quotes[idx]` when there is none.
- `exchange` is optional, trimmed, and parsed with `parse_yahoo_exchange`. Yahoo codes such as `NMS` normalise to `NASDAQ` with no warning. An unknown code is omitted with `OmittedPresentField { path: "quotes[].exchange" }` and the result is kept without an exchange.
- `name` is `longname`, falling back to `shortname`. A malformed `longname` is omitted with a warning whose `field` is the path string `"quotes[].longname"`.
- `Instrument` is built with `from_symbol_and_exchange` or `from_symbol`. If that fails, the result is dropped with `InvalidField{field:"symbol"}`.
- `exchDisp` and `typeDisp` are parsed and ignored.

implementations:
  - src/search/mod.rs:parse_search_body
  - src/search/mod.rs:search_quote_diag_key

verifies:
  - tests/search/names.rs:search_prefers_long_name_for_display_name
  - tests/search/offline.rs:invalid_search_exchange_is_reported_as_projection_loss
  - tests/search/offline.rs:yahoo_search_exchange_codes_normalize_without_diagnostics
  - tests/search/offline.rs:malformed_optional_search_name_is_omitted_without_losing_result
  - tests/search/offline.rs:malformed_search_quote_is_dropped_without_losing_valid_siblings

<!-- id: REQ-SRCH-002.A1 -->
- [x] If both names are present, `name` is the `longname` ("The a2 Milk Company Limited").
<!-- id: REQ-SRCH-002.A2 -->
- [x] Exchange `"!!!"` keeps the result (1 result) with an `OmittedPresentField{path:"quotes[].exchange"}` warning. Strict mode returns `DataQuality`.
<!-- id: REQ-SRCH-002.A3 -->
- [x] Exchange `NMS` resolves to `NASDAQ` with empty diagnostics, and strict mode succeeds.
<!-- id: REQ-SRCH-002.A4 -->
- [x] `longname: 42` falls back to `shortname` "Apple Inc." with a warning keyed "AAPL". Strict mode returns `DataQuality`.
<!-- id: REQ-SRCH-002.A5 -->
- [x] `symbol: 123` drops that quote with `InvalidField{field:"symbol"}` keyed `quotes[0]` while the valid sibling AAPL is kept. Strict mode returns `DataQuality`.
<!-- id: REQ-SRCH-002.A6 -->
- [ ] A quote with no `quoteType` is dropped. [needs review — no test backs this]

<!-- id: REQ-HOLD-008 -->
## Cross-cutting: lenient vs strict projection diagnostics

All five areas use `ProjectionContext::new(<endpoint>, options.data_quality())`. In lenient (default) mode, warnings (`DroppedItem`, `OmittedPresentField`, `ProviderFeatureUnavailable`) are collected into `YfResponse.diagnostics`, and the plain methods throw them away through `into_data()`. In `Strict` mode the first warning returns `YfError::DataQuality(Box<YfWarning>)` straight away (context.rs:33). Endpoint tags are `"holders"`, `"esg"`, `"profile"`, `"news"` and `"search"`.

implementations:
  - src/core/diagnostics/context.rs:ProjectionContext.dropped_item
  - src/core/diagnostics/context.rs:ProjectionContext.unavailable_feature
  - src/core/diagnostics/context.rs:ProjectionContext.provider_feature_unavailable
  - src/core/diagnostics/context.rs:ProjectionContext.finish

verifies:
  - tests/holders/strict_synthetic.rs:missing_institution_ownership_module_is_provider_unavailable
  - tests/esg/status_synthetic.rs:missing_esg_module_is_reported_as_unavailable
  - tests/ticker/profile.rs:strict_profile_rejects_malformed_optional_field
  - tests/news/offline.rs:malformed_news_stream_items_are_dropped_with_diagnostics
  - tests/search/offline.rs:malformed_search_quote_is_dropped_without_losing_valid_siblings

<!-- id: REQ-HOLD-008.A1 -->
- [x] In each of the five areas, a payload that produces a warning in lenient mode produces `YfError::DataQuality` under `.strict()`.

<!-- id: INFO-NONGOALS -->
## Non-goals

- ESG `totalEsg`, `percentile` and `highestControversy` are deliberately not projected. They are marked `#[allow(dead_code)]`, and a comment says `paft::EsgScores` "now has only environmental/social/governance" (src/esg/api.rs:55).
- Search does not request or return news or lists. The test asserts that `newsCount` and `listsCount` are absent (tests/search/offline.rs:22-26).
- Profiles for index, crypto and other non-equity, non-fund quote types are not supported (src/profile/mod.rs:33).
- Live ESG coverage is out of scope while the Yahoo endpoint is unavailable (comment at tests/esg.rs:6-7).
- Anything beyond these: [fill in — not derivable from code]

<!-- id: INFO-OPEN-QUESTIONS -->
## Open questions

1. Is the ESG feature still supported? The only recorded fixture is a "Not Found" body and the positive-path tests are disabled. Should the feature be deprecated or documented as best-effort?
2. Should strict mode treat news **ads** as fatal? Today one sponsored item fails `strict().news()`.
3. Should a missing `tickerStream` or `stream` in news emit `ProviderFeatureUnavailable`, as holders and ESG do?
4. Is dropping roster rows that lack `transactionDescription`, `latestTransDate` or `positionDirectDate` acceptable data loss, or should those fields be optional?
5. What is the business basis for treating a blank-text, positive-shares, no-value insider transaction as `Exercise`?
6. Should the five holder modules be fetched in one combined quoteSummary call? A combined fixture `holders_api_institutionOwnership-fundOwnership-majorHoldersBreakdown-insiderTransactions-insiderHolders-netSharePurchaseActivity_AAPL.json` exists but no test uses it.
7. Should the search base URL be configurable on `YfClientBuilder`, like `base_news` and `base_quote_api`, instead of a per-builder "testing" override?
8. Are the hard-coded English `MajorHolder.category` labels part of the public contract?
9. Do Yahoo holder values for GBp-quoted symbols come in pounds or pence?

<!-- id: INFO-REVIEW -->
## Code review findings

Ranked by severity, highest first.

1. **[High] Strict-mode news fails on any ad item.** At `src/news/api.rs:91-98`, ads go through `ctx.dropped_item(...)`, which in `Strict` mode returns `Err(DataQuality)` (`src/core/diagnostics/context.rs:33`). An ad is routine provider content, not a data-quality problem, so `Ticker::strict().news()` can fail intermittently whenever Yahoo includes a sponsored item. No test covers this.
2. **[High] ESG has no working positive path and no live coverage.** `tests/esg.rs:6-15` comments out all fixture and live tests, and `tests/fixtures/esg_api_esgScores_MSFT.json` is a Not Found body. The involvement mapping (`src/esg/api.rs:80-107`) and score mapping on real data are untested. The public `Ticker::sustainability()` API is still advertised, and example 03 tolerates failure.
3. **[Medium] A non-`WireValue` ESG field can fail the whole response.** `src/esg/wire.rs:27-47` declares `percentile`, `highest_controversy` and the 15 involvement flags as plain `Option<f64>` / `Option<bool>`. If Yahoo sends a `{raw: ..}` object or a string there, deserialisation of `V10Result` fails with a JSON error instead of a lenient omission. The other ESG fields use `WireValue`, so this is inconsistent with the crate's own tolerance pattern. `percentile` and `highest_controversy` are not even used.
4. **[Medium] A blank or invalid fund `legalType` aborts the whole profile.** At `src/profile/mod.rs:73` (called from `src/profile/api.rs:239`), `resolve_fund_kind` propagates the error from `string_to_fund_kind`. For `legalType: ""`, `parse_required_token` (`src/core/conversions.rs:366-367`) returns `MissingData("fund kind missing")`. The fund profile then fails even in lenient mode, when it could fall back to the quote-type kind with a warning. Untested.
5. **[Medium] Missing news structure is silently empty.** `src/news/api.rs:68-72` uses `unwrap_or_default()` when `data`, `tickerStream` or `stream` is absent. No `ProviderFeatureUnavailable` is reported, so a Yahoo schema change looks like "no news" even under strict. This is inconsistent with holders (`src/holders/api.rs:350-357`) and ESG (`src/esg/api.rs:28-33`).
6. **[Medium] The insider roster drops rows aggressively.** At `src/holders/api.rs:532-562`, `transactionDescription`, `latestTransDate` and `positionDirectDate` are all required. An insider who has holdings but no recorded direct-position date, or no recognised transaction description, is removed entirely. Only the missing-`relation` case is tested.
7. **[Low] One HTTP request per holder method.** `src/holders/api.rs:45-52`: callers who want every holder view (example 09, `offline_all_holders_from_fixture`) make six quoteSummary calls plus one quote call, instead of one multi-module call. The combined fixture is orphaned.
8. **[Low] The search base URL ignores client configuration.** At `src/search/mod.rs:189` and `:339`, the URL is hard-coded per builder with `Url::parse(..).unwrap()`. Proxies and custom bases set on `YfClientBuilder` do not apply to it, and only `search_base` (documented "for testing") can override it. Also, `quotes_count: Option<u32>` (`:173`) can never be `None`.
9. **[Low] Diagnostic `field` naming is inconsistent.** For search optional fields, `optional_cloned` (`src/core/diagnostics/projection.rs:109`) passes the path as the field (`field: "quotes[].longname"`), while other modules use bare field names (`"sector"`, `"environmentScore"`). The test at `tests/search/offline.rs:289-299` locks in this behaviour.
10. **[Low] The debug dump swallows write errors and lives in the wrong module.** `src/profile/debug.rs:8` discards the `write_all` error and still returns `Ok`. It is called for every quoteSummary fetch (`src/core/quotesummary.rs:83`), not only profiles, but it sits under `profile/`.
11. **[Low] Percent units are inconsistent.** `netPercentInsiderShares` is projected as `Decimal` (`src/holders/api.rs:660`), while the other holder percents are `Ratio` (`:74`, `:226`). Consumers cannot tell whether 0.05 means 5% or 0.05%. [needs review — depends on Yahoo semantics]
12. **[Low] `load_profile_from_quote_summary_raw` skips `normalize_symbol`.** At `src/profile/mod.rs:76-89`, unlike `load_profile_with_options_and_diagnostics` (`:108`), currency hints may be stored under an unnormalised symbol key. [needs review — depends on whether the caller in src/ticker/info.rs:265 normalises first]
13. **[Low / missing tests]** Nothing tests: holder rows missing `organization`/`reportDate`; unparseable insider position or transaction text; `NewsTab::All`; search `lang`/`region`; search with no `quotes`; search with no `quoteType`; profile name fallbacks; a missing `assetProfile`/`fundProfile`/`quoteType` module.

<!-- id: INFO-NOTES -->
## Reverse-engineering notes

- **Confidence: high** for the holders, profile, news and search request shapes and projection rules. Non-ignored offline tests with mocked HTTP back them directly.
- **Confidence: medium** for ESG. The mapping code is simple, but the only active tests cover error and unavailable paths, and the involvement mapping is unverified.
- **Confidence: low** for business intent: the blank-text-to-`Exercise` inference, the English major-holder labels, and the GBp/GBP value semantics.
- Tests were **not executed** while writing this spec. The local toolchain is rustc 1.88.0, but the crate requires `rust-version = "1.91"`, so `cargo test` refused to build. `[x]` means a non-`#[ignore]` test exists that asserts the criterion. It does not mean the test was seen to pass in this session.
- Live tests (`tests/*/live.rs`) are `#[ignore]` and need network access. They are listed under `verifies:` only as smoke coverage.
- Test files are compiled as modules through `#[path]` from `tests/holders.rs`, `tests/esg.rs`, `tests/profile.rs`, `tests/news.rs`, `tests/search.rs` and `tests/ticker.rs`. The test paths above refer to the physical files.
- `Ticker::isin()` (src/ticker/isin.rs, the businessinsider lookup) is related to search fixtures but is outside this spec's scope.
- Model types (`MajorHolder`, `InstitutionalHolder`, `EsgSummary`, `Profile`, `NewsArticle`, `SearchResponse`, and others) come from `paft` 0.9 and their field definitions live in that crate. Field names here are inferred from construction sites in this crate.
