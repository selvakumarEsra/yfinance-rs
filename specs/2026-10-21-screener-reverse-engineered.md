---
id: SCRN-DOC
title: Yahoo Finance screener (predefined and custom typed queries)
owner: unassigned
priority: medium
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/screener/mod.rs
  - src/screener/builder.rs
  - src/screener/fields.rs
  - src/screener/predefined.rs
  - src/screener/query.rs
  - src/screener/response.rs
  - src/lib.rs
  - src/core/call_options.rs
  - src/core/client/retry.rs
  - src/core/net.rs
  - tests/screener.rs
  - tests/screener/offline.rs
  - tests/screener/live.rs
  - tests/auth/live.rs
  - tests/core/public_validation.rs
  - tests/fixtures/screener_custom_equity.json
  - tests/fixtures/screener_predefined_day_gainers.json
  - examples/15_screeners.rs
  - README.md
  - CHANGELOG.md
---

<!-- id: SCRN-DOC -->
# Yahoo Finance screener (reverse-engineered)

Intent: the crate lets callers run Yahoo Finance screeners in two ways: by naming one of Yahoo's predefined saved screens (`PredefinedScreener`), or by building a custom equity, mutual fund or ETF query from typed fields, operators and value vocabularies (`EquityQuery`, `FundQuery`, `EtfQuery`). Inputs are strongly typed, so an unsupported Yahoo field or value needs a crate change and the crate never sends a malformed provider payload without warning (module doc, `src/screener/mod.rs:1-5`). Responses are projected into `ScreenerResponse` / `ScreenerResult`. Where possible these use provider-agnostic `paft` identity and money types, and data-quality diagnostics are reported. The custom POST body is documented as matching the Python `yfinance` wire shape (test names `*_python_wire_shape`). The business reason for the specific set of supported fields and vocabularies is `[purpose unclear]`. It looks like it mirrors what the predefined screens need, plus a few extras.

<!-- id: REQ-SCRN-001 -->
## Predefined screener catalogue

The crate exposes a closed catalogue of Yahoo predefined screens. Raw Yahoo screen IDs are deliberately not accepted (doc comment, `src/screener/predefined.rs:9-12`).

- `#[non_exhaustive] pub enum PredefinedScreener` has 19 variants. 9 are equity screens (`AggressiveSmallCaps`, `DayGainers`, `DayLosers`, `GrowthTechnologyStocks`, `MostActives`, `MostShortedStocks`, `SmallCapGainers`, `UndervaluedGrowthStocks`, `UndervaluedLargeCaps`), 6 are mutual fund screens (`ConservativeForeignFunds`, `HighYieldBond`, `PortfolioAnchors`, `SolidLargeGrowthFunds`, `SolidMidcapGrowthFunds`, `TopMutualFunds`) and 4 are ETF screens (`TopEtfsUs`, `TopPerformingEtfs`, `TechnologyEtfs`, `BondEtfs`).
- `pub const fn id(self) -> &'static str` returns the snake_case Yahoo `scrIds` value, for example `DayGainers -> "day_gainers"` and `TopEtfsUs -> "top_etfs_us"`.

implementations:
  - src/screener/predefined.rs:PredefinedScreener
  - src/screener/predefined.rs:PredefinedScreener.id

verifies:
  - tests/screener/offline.rs:offline_predefined_day_gainers_uses_get_with_expected_params

<!-- id: REQ-SCRN-001.A1 -->
- [x] `PredefinedScreener::DayGainers.id()` is sent as `scrIds=day_gainers`.
<!-- id: REQ-SCRN-001.A2 -->
- [ ] Every variant maps to a unique, Yahoo-recognised `scrIds` string. [needs review — no test backs this; only `day_gainers` is exercised offline, and no test checks the other 18 IDs against Yahoo]
<!-- id: REQ-SCRN-001.A3 -->
- [ ] The public API has no constructor that accepts an arbitrary predefined screen ID string. [needs review — no test backs this; enforced only by the type design]

<!-- id: REQ-SCRN-002 -->
## Fetch predefined screener via GET

A predefined screen with no offset is fetched with an HTTP GET to Yahoo's saved-screener endpoint.

- Entry points:
  - `pub async fn screen(client: &YfClient, screener: PredefinedScreener) -> Result<ScreenerResponse, YfError>`
  - `pub async fn screen_with_diagnostics(client, screener) -> Result<YfResponse<ScreenerResponse>, YfError>`
  - `ScreenerBuilder::predefined(client, screener)`, or its alias `ScreenerBuilder::new`, followed by `.fetch()` / `.fetch_with_diagnostics()`
- Endpoint: `GET https://query1.finance.yahoo.com/v1/finance/screener/predefined/saved`.
- Query parameters: `corsDomain=finance.yahoo.com`, `formatted=false`, `lang=en-US`, `region=US`, `scrIds=<id>`, plus `count=<n>` only when `.count(ScreenerCount)` was set.
- Defaults for a predefined builder: `count = None` (Yahoo's server default applies, and the fixture shows 25), `offset = None`.
- Header: `accept: application/json`.

implementations:
  - src/screener/mod.rs:screen
  - src/screener/mod.rs:screen_with_diagnostics
  - src/screener/builder.rs:ScreenerBuilder.predefined
  - src/screener/builder.rs:ScreenerBuilder.new
  - src/screener/builder.rs:ScreenerBuilder.fetch
  - src/screener/builder.rs:ScreenerBuilder.fetch_with_diagnostics
  - src/screener/builder.rs:ScreenerBuilder.fetch_predefined_get
  - src/screener/builder.rs:append_common_params

verifies:
  - tests/screener/offline.rs:offline_predefined_day_gainers_uses_get_with_expected_params
  - tests/screener/live.rs:live_predefined_and_custom_smoke
  - tests/auth/live.rs:live_cached_crumb_optional_endpoints_smoke

<!-- id: REQ-SCRN-002.A1 -->
- [x] A predefined fetch with no offset sends a GET to `/v1/finance/screener/predefined/saved` with `scrIds`, `corsDomain`, `formatted=false`, `lang=en-US` and `region=US`.
<!-- id: REQ-SCRN-002.A2 -->
- [x] Parsing the recorded `day_gainers` fixture gives `count > 0`, a non-empty `results` list, and a first row with both `symbol` and `quote_type` set.
<!-- id: REQ-SCRN-002.A3 -->
- [ ] When `.count(n)` is set on a predefined builder, the GET carries `count=n`. When it is not set, the GET has no `count` parameter. [needs review — no test backs this]
<!-- id: REQ-SCRN-002.A4 -->
- [ ] `screen()` returns the same data as `ScreenerBuilder::predefined(..).fetch()`. [needs review — only live-ignored tests use `screen()`]

<!-- id: REQ-SCRN-003 -->
## Predefined screener with offset falls back to a locally reconstructed custom POST

Calling `.offset(ResultOffset)` on a predefined builder makes `fetch_with_diagnostics` stop using the saved-screener GET. It sends a custom POST instead (REQ-SCRN-006), using a hard-coded copy of the screen's criteria from `PredefinedScreener::custom_parts`. The trigger is the presence of an offset, so this happens even for `ResultOffset::new(0)`. Each variant fixes a sort field, a sort direction and a typed query. For example, `DayGainers` sorts by `percentchange` DESC with the filter `percentchange > 3`, `region = us`, `intradaymarketcap >= 2e9`, `intradayprice >= 5`, `dayvolume > 15000`. If the builder has no count, the POST body has no `count` key.

implementations:
  - src/screener/builder.rs:ScreenerBuilder.fetch_with_diagnostics
  - src/screener/builder.rs:ScreenerBuilder.offset
  - src/screener/predefined.rs:PredefinedScreener.custom_parts
  - src/screener/builder.rs:CustomParts.equity
  - src/screener/builder.rs:CustomParts.fund
  - src/screener/builder.rs:CustomParts.etf

<!-- id: REQ-SCRN-003.A1 -->
- [ ] A predefined builder with any offset set sends a POST to `/v1/finance/screener`, using that screen's `quoteType`, sort field, sort direction and query. [needs review — no test backs this]
<!-- id: REQ-SCRN-003.A2 -->
- [ ] The reconstructed criteria match Yahoo's server-side definition for each screen. [needs review — the code diverges from the recorded fixture `rawCriteria` for `day_gainers`; see SCRN-REVIEW F2]
<!-- id: REQ-SCRN-003.A3 -->
- [ ] `custom_parts()` succeeds for all 19 variants. It returns `Result` only because the inner builders are fallible. [needs review — no test backs this]

<!-- id: REQ-SCRN-004 -->
## Typed custom query composition and wire encoding

Custom queries are trees of comparisons and logical nodes, with a phantom asset-class marker (`Equity`, `Fund` or `Etf`) so that fields from one asset class cannot be used in another.

- Type aliases: `EquityQuery = ScreenerQuery<Equity>`, `FundQuery = ScreenerQuery<Fund>`, `EtfQuery = ScreenerQuery<Etf>`.
- Field kinds:
  - `NumericField<U>` supports `eq`, `gt`, `gte`, `lt`, `lte`, `between(lo, hi)` and `one_of(iter) -> Result`. Values are `Into<ScreenerNumber>`.
  - `PercentField<U>` supports `gt`, `gte`, `lt` and `lte`, taking `PercentPoints` only. There is no `eq` or `between`.
  - `EnumField<U, V: ScreenerValue>` supports `eq` and `one_of(iter) -> Result`.
  - `SortField<U>` is a key only, used by `sort_by`.
- Field constants:
  - `equity_fields` has 19 constants (ticker, exchange, region, sector, prices, market cap, percent change, volumes, short interest, EPS growth, revenue growth, PE, PEG).
  - `fund_fields` has 10 constants.
  - `etf_fields` has 7 constants.
- Logical operators: `ScreenerQuery::and(iter)` and `ScreenerQuery::or(iter)` return `Result` and need at least 2 operands, otherwise `YfError::InvalidParams`.
- Wire shape:
  - A comparison is encoded as `{"operator": OP, "operands": [field, value...]}`. `OP` is one of `EQ`, `GT`, `GTE`, `LT`, `LTE` or `BTWN`. `BTWN` is flattened to `[field, lo, hi]`.
  - `one_of` expands to `{"operator":"OR","operands":[EQ...]}` and fails on empty input.
- `ScreenerValue` is a sealed trait, so callers cannot add their own value types.

implementations:
  - src/screener/query.rs:ScreenerQuery
  - src/screener/query.rs:ScreenerQuery.and
  - src/screener/query.rs:ScreenerQuery.or
  - src/screener/query.rs:ScreenerQuery.logical
  - src/screener/query.rs:ScreenerQuery.comparison
  - src/screener/query.rs:ScreenerQuery.one_of_values
  - src/screener/query.rs:ScreenerQuery.into_wire_value
  - src/screener/query.rs:QueryNode.to_wire_value
  - src/screener/query.rs:NumericField
  - src/screener/query.rs:PercentField
  - src/screener/query.rs:EnumField
  - src/screener/query.rs:SortField
  - src/screener/query.rs:ScreenerValue
  - src/screener/fields.rs:equity_fields
  - src/screener/fields.rs:fund_fields
  - src/screener/fields.rs:etf_fields

verifies:
  - src/screener/query.rs:one_of_expands_to_or_of_eq_filters
  - src/screener/query.rs:and_rejects_empty_or_single_operand
  - src/screener/query.rs:screener_number_serializes_validated_float
  - src/screener/builder.rs:custom_body_matches_python_wire_shape
  - tests/core/public_validation.rs:crate_root_reexports_common_public_types

<!-- id: REQ-SCRN-004.A1 -->
- [x] `EXCHANGE.one_of([Nms, Nyq])` serialises to an OR of two EQ nodes on `exchange`, with values `"NMS"` and `"NYQ"`.
<!-- id: REQ-SCRN-004.A2 -->
- [x] `EquityQuery::and` with zero or one operand returns an error.
<!-- id: REQ-SCRN-004.A3 -->
- [x] `INTRADAY_PRICE.gt(ScreenerNumber::new(12.5)?)` serialises to `{"operator":"GT","operands":["intradayprice",12.5]}`.
<!-- id: REQ-SCRN-004.A4 -->
- [x] `AND(PERCENT_CHANGE > 3.0, REGION = us)` serialises to the nested AND/GT/EQ structure shown in the custom-body test.
<!-- id: REQ-SCRN-004.A5 -->
- [ ] `between(lo, hi)` serialises to `{"operator":"BTWN","operands":[field, lo, hi]}`. [needs review — no test backs this]
<!-- id: REQ-SCRN-004.A6 -->
- [ ] `one_of` with an empty iterator returns `InvalidParams`. `one_of` with exactly one value returns a single-operand OR. [needs review — no test backs this, and it is unclear whether Yahoo accepts a single-operand OR; see SCRN-REVIEW F5]
<!-- id: REQ-SCRN-004.A7 -->
- [x] `ScreenerQuery<Equity>` is re-exported at `yfinance_rs::screener::query::ScreenerQuery` (type-name assertion).

<!-- id: REQ-SCRN-005 -->
## Validated scalar inputs and closed value vocabularies

All scalar inputs are validated when they are constructed, and every enum value has one fixed Yahoo wire value.

- `ScreenerCount::new(u32) -> Result`: accepts values from 1 to `ScreenerCount::MAX = 250`, otherwise `InvalidParams`. The crate-internal default is 25.
- `ResultOffset::new(u32)` cannot fail. `ResultOffset::try_from_i64(i64) -> Result` rejects negative values and values above `u32::MAX`.
- `ScreenerNumber::new(f64) -> Result` rejects NaN and ±inf. It also has `From<u32 | i32 | u64 | i64>`. Integers are serialised as JSON integers and floats as JSON floats.
- `PercentPoints::new(f64) -> Result` holds percentage points, so `3` means 3%, not `0.03`.
- `SortDirection::{Asc, Desc}` serialise to `"ASC"` and `"DESC"`.
- `Region::Us` serialises to `"us"`. It is the only region.
- `YahooExchangeCode` (`#[non_exhaustive]`, 17 codes from NMS to WCB) serialises to its upper-case code.
- `EquitySector` (11 values), `FundCategory` (10 values) and `EtfCategory` (13 values) serialise to Yahoo display strings such as `"Foreign Small/Mid Growth"`.
- `Rating::One` to `Rating::Five` serialise to the integers 1 to 5.
- `YahooQuoteType::{Equity, MutualFund, Etf}` serialise to `"EQUITY"`, `"MUTUALFUND"` and `"ETF"`.

implementations:
  - src/screener/query.rs:ScreenerCount.new
  - src/screener/query.rs:ResultOffset.new
  - src/screener/query.rs:ResultOffset.try_from_i64
  - src/screener/query.rs:ScreenerNumber.new
  - src/screener/query.rs:FiniteF64.new
  - src/screener/query.rs:PercentPoints.new
  - src/screener/query.rs:SortDirection.as_str
  - src/screener/query.rs:Region
  - src/screener/query.rs:YahooExchangeCode.code
  - src/screener/query.rs:EquitySector
  - src/screener/query.rs:FundCategory
  - src/screener/query.rs:EtfCategory
  - src/screener/query.rs:Rating
  - src/screener/query.rs:YahooQuoteType

verifies:
  - src/screener/query.rs:bounded_values_reject_invalid_inputs
  - src/screener/query.rs:screener_number_serializes_validated_float

<!-- id: REQ-SCRN-005.A1 -->
- [x] `ScreenerCount::new(0)` and `ScreenerCount::new(251)` return an error.
<!-- id: REQ-SCRN-005.A2 -->
- [x] `ScreenerNumber::new(NaN)` and `ScreenerNumber::new(inf)` return an error.
<!-- id: REQ-SCRN-005.A3 -->
- [x] `ResultOffset::try_from_i64(-1)` returns an error.
<!-- id: REQ-SCRN-005.A4 -->
- [ ] The category and sector display strings exactly match the values Yahoo accepts. [needs review — no test backs this; the strings are hard-coded and only some are used by live predefined reconstructions]
<!-- id: REQ-SCRN-005.A5 -->
- [ ] `ScreenerCount::new(250)` and `ScreenerCount::new(1)` succeed. [needs review — no test backs this; the boundary values are unasserted]

<!-- id: REQ-SCRN-006 -->
## Custom screener POST request

Custom equity, fund and ETF screens are sent as a JSON POST.

- Constructors:
  - `ScreenerBuilder::equity(client, EquityQuery) -> ScreenerBuilder<Equity>`
  - `ScreenerBuilder::fund(client, FundQuery) -> ScreenerBuilder<Fund>`
  - `ScreenerBuilder::etf(client, EtfQuery) -> ScreenerBuilder<Etf>`
- `.sort_by(SortField<U>, SortDirection)` exists on each typed builder, and only accepts sort fields of the same asset class. `.count(ScreenerCount)` and `.offset(ResultOffset)` are available too.
- Defaults: `count = 25`, `offset = 0`, `sortField = "ticker"`, `sortType = "DESC"`.
- Endpoint: `POST https://query1.finance.yahoo.com/v1/finance/screener`, with the same four common query parameters as the GET. Headers: `accept: application/json` and `content-type: application/json`.
- Body keys: `offset`, `count`, `sortField`, `sortType`, `userId: ""`, `userIdType: "guid"`, `quoteType` (`EQUITY`, `MUTUALFUND` or `ETF`) and `query`.

implementations:
  - src/screener/builder.rs:ScreenerBuilder.equity
  - src/screener/builder.rs:ScreenerBuilder.fund
  - src/screener/builder.rs:ScreenerBuilder.etf
  - src/screener/builder.rs:ScreenerBuilder.custom
  - src/screener/builder.rs:ScreenerBuilder.sort_by
  - src/screener/builder.rs:ScreenerBuilder.set_sort
  - src/screener/builder.rs:ScreenerBuilder.count
  - src/screener/builder.rs:ScreenerBuilder.offset
  - src/screener/builder.rs:ScreenerBuilder.fetch_custom_post
  - src/screener/builder.rs:ScreenerBuilder.custom_body
  - src/screener/builder.rs:ScreenerBuilder.post_with_auth_retry

verifies:
  - src/screener/builder.rs:custom_body_matches_python_wire_shape
  - tests/screener/offline.rs:offline_custom_equity_query_posts_python_wire_shape
  - tests/screener/live.rs:live_predefined_and_custom_smoke

<!-- id: REQ-SCRN-006.A1 -->
- [x] A default equity builder sends a POST to `/v1/finance/screener` with `content-type: application/json` and the exact body `{offset:0, count:25, sortField:"ticker", sortType:"DESC", userId:"", userIdType:"guid", quoteType:"EQUITY", query:{...}}`.
<!-- id: REQ-SCRN-006.A2 -->
- [x] Parsing the recorded custom-equity fixture gives non-empty results with `symbol` set.
<!-- id: REQ-SCRN-006.A3 -->
- [ ] `.sort_by(field, dir)`, `.count(n)` and `.offset(k)` show up as `sortField`, `sortType`, `count` and `offset` in the body. [needs review — no test backs this; only the example and the ignored live test use them]
<!-- id: REQ-SCRN-006.A4 -->
- [ ] Fund and ETF builders send `quoteType` `MUTUALFUND` and `ETF` respectively. [needs review — no test backs this]

<!-- id: REQ-SCRN-007 -->
## Response envelope parsing and result projection

The Yahoo envelope `{"finance":{"result":[{...}],"error":...}}` is parsed into `ScreenerResponse { count: Option<u32>, results: Vec<ScreenerResult> }`. Both structs are `#[non_exhaustive]` and implement `Serialize`.

- A non-null `finance.error` gives `YfError::Api(<error json>)`, even when the HTTP status is 200.
- A missing or empty `result` gives `YfError::MissingData("screener result missing")`. Missing `quotes` gives `YfError::MissingData("screener quotes missing")`.
- `count` is Yahoo's `result[0].count`. The field doc calls it the "Total count", which conflicts with the fixtures; see SCRN-REVIEW F1.
- `ScreenerResult` field mapping:
  - `symbol` is copied raw from Yahoo.
  - `name` is `shortName`, falling back to `longName`.
  - `quote_type` is `Option<YahooQuoteType>`.
  - `exchange` is `paft::Exchange`, parsed from `exchange`.
  - `yahoo_exchange` is `Option<YahooExchangeCode>`.
  - `raw_exchange`, `exchange_display` (from `exchDisp`) and `type_display` (from `typeDisp`) are copied as strings.
  - `instrument` is `paft::Instrument`, built from the symbol, the asset kind derived from `quoteType`, and the exchange if present.
  - `regular_market_change_percent` is in percentage points.
  - `regular_market_volume: u64` also accepts numeric strings.
  - `fields: BTreeMap<String, Value>` holds every unmapped Yahoo key.

implementations:
  - src/screener/response.rs:ScreenerResponse
  - src/screener/response.rs:ScreenerResult
  - src/screener/response.rs:parse_screener_body_with_diagnostics
  - src/screener/response.rs:reject_screener_error
  - src/screener/response.rs:WireQuote.fields
  - src/screener/response.rs:WireQuote.project
  - src/screener/response.rs:project_instrument
  - src/screener/response.rs:screener_quote_diag_key
  - src/screener/response.rs:nonempty

verifies:
  - tests/screener/offline.rs:offline_predefined_day_gainers_uses_get_with_expected_params
  - tests/screener/offline.rs:predefined_screener_does_not_cache_successful_status_api_error_body
  - tests/screener/offline.rs:predefined_screener_regular_market_volume_accepts_numeric_string
  - tests/screener/offline.rs:screener_yahoo_exchange_codes_normalize_without_diagnostics

<!-- id: REQ-SCRN-007.A1 -->
- [x] An HTTP 200 body with `finance.error = {"code":"Unauthorized",...}` returns `YfError::Api`, and the message contains `Unauthorized`.
<!-- id: REQ-SCRN-007.A2 -->
- [x] `"regularMarketVolume": "12345"` projects to `regular_market_volume == Some(12345)`.
<!-- id: REQ-SCRN-007.A3 -->
- [x] An ETF quote with `exchange: "NGM"` projects to `exchange` displaying as `"NASDAQ"` and `yahoo_exchange == Some(Ngm)`, with no warnings. Strict mode also succeeds.
<!-- id: REQ-SCRN-007.A4 -->
- [ ] A missing or empty `result` and missing `quotes` each give `YfError::MissingData`. [needs review — no test backs this]
<!-- id: REQ-SCRN-007.A5 -->
- [ ] Unmapped Yahoo keys, such as `fiftyTwoWeekHigh`, are kept in `ScreenerResult.fields`. [needs review — no test backs this]
<!-- id: REQ-SCRN-007.A6 -->
- [ ] `name` falls back to `longName` when `shortName` is missing. [needs review — no test backs this]

<!-- id: REQ-SCRN-008 -->
## Currency-aware price and market-cap projection

`price` (`paft::Price`) and `market_cap` (`paft::Money`) are only filled in when Yahoo's `currency` resolves to a known currency unit.

- `price` uses `ResolvedCurrencyUnit::price_from_f64`.
- `market_cap` is converted in the currency's major unit, using `JsonDecimal` to avoid losing precision.
- If the currency is missing, the value is omitted with `ProjectionIssue::CurrencyUnresolved`.
- If the currency is unrecognised, the value is omitted with `ProjectionIssue::InvalidCurrency { code }`.
- If conversion fails, the value is omitted with `ProjectionIssue::ConversionFailed { target }`.

implementations:
  - src/screener/response.rs:parse_screener_currency
  - src/screener/response.rs:optional_screener_price
  - src/screener/response.rs:optional_screener_money
  - src/screener/response.rs:optional_screener_currency_value

verifies:
  - tests/screener/offline.rs:predefined_screener_market_cap_preserves_large_integer_precision
  - tests/screener/offline.rs:predefined_screener_with_diagnostics_reports_invalid_currency_for_present_price

<!-- id: REQ-SCRN-008.A1 -->
- [x] `marketCap: 9007199254740993` with `currency: "USD"` projects to `market_cap.amount() == Decimal::from(9007199254740993)`, with no f64 rounding.
<!-- id: REQ-SCRN-008.A2 -->
- [x] `currency: "!!!"` with a price present leaves `price` and `market_cap` as `None`, and emits `OmittedPresentField { endpoint:"screener", path:"regularMarketPrice", key:"BADCUR", reason: InvalidCurrency{code:"!!!"} }`.
<!-- id: REQ-SCRN-008.A3 -->
- [ ] A minor-unit currency (for example `GBp`) gives a price in the minor unit and a market cap in the major unit (GBP). [needs review — no test backs this; inferred from `major_unit()` in `optional_screener_money` only]

<!-- id: REQ-SCRN-009 -->
## Data-quality diagnostics and strict mode

Projection problems become `YfWarning`s in `YfResponse.diagnostics` (endpoint `"screener"`), keyed by the row's symbol, or `quotes[idx]` when the symbol is missing. In best-effort mode the affected field or row is dropped and parsing continues. `.strict()` or `.data_quality(DataQuality)` turns the same issues into `YfError::DataQuality`.

Warnings that can be raised:
- Malformed optional field: `InvalidField`.
- Unsupported `quoteType`: `InvalidField` on `quoteType`.
- Unparsable exchange: `InvalidField` on `exchange`.
- Symbol present but no asset kind: `MissingRequiredField{quoteType}` on `instrument`.
- Instrument construction failure: `InvalidField{symbol}` on `instrument`.
- A quote that is not an object: dropped item `screener_result`.

implementations:
  - src/screener/response.rs:parse_screener_body_with_diagnostics
  - src/screener/response.rs:WireQuote.project
  - src/screener/response.rs:project_instrument
  - src/screener/builder.rs:ScreenerBuilder.fetch_with_diagnostics

verifies:
  - tests/screener/offline.rs:malformed_screener_optional_field_is_omitted_without_losing_result
  - tests/screener/offline.rs:screener_yahoo_exchange_codes_normalize_without_diagnostics
  - tests/screener/offline.rs:predefined_screener_with_diagnostics_reports_invalid_currency_for_present_price

<!-- id: REQ-SCRN-009.A1 -->
- [x] A row with `regularMarketPrice: "not-a-number"` is kept with `price = None`, other rows are unaffected, and an `OmittedPresentField { path:"regularMarketPrice", reason: InvalidField{..}, key:"BADQUOTE" }` warning is emitted.
<!-- id: REQ-SCRN-009.A2 -->
- [x] The same body fetched with `.strict()` fails with `YfError::DataQuality`.
<!-- id: REQ-SCRN-009.A3 -->
- [ ] An unsupported `quoteType` (for example `"CRYPTOCURRENCY"`) leaves `quote_type = None` and emits a warning, but `instrument` is still built if `parse_yahoo_quote_type` accepts the value. [needs review — no test backs this]
<!-- id: REQ-SCRN-009.A4 -->
- [ ] A non-object entry in `quotes` is dropped with a `screener_result` dropped-item diagnostic. [needs review — no test backs this]

<!-- id: REQ-SCRN-010 -->
## Authentication: optional crumb with stale-crumb refresh

GET and POST both use `AuthMode::OptionalCrumb` with `retry_on_invalid_crumb_body: true`. Cached cookie/crumb credentials are tried first. On 401/403, or on an invalid-crumb body, the crumb is refreshed and the request is retried. A bare request with no crumb is not sent first.

implementations:
  - src/screener/builder.rs:ScreenerBuilder.get_with_auth_retry
  - src/screener/builder.rs:ScreenerBuilder.post_with_auth_retry

verifies:
  - tests/screener/offline.rs:predefined_screener_401_with_stale_cached_crumb_refreshes_before_retry
  - tests/screener/offline.rs:custom_screener_403_with_stale_cached_crumb_refreshes_before_retry

<!-- id: REQ-SCRN-010.A1 -->
- [x] For a predefined GET, a 401 with a stale cached crumb triggers a cookie and crumb refresh, then a successful retry with `crumb=crumb-value`. No bare request is sent.
<!-- id: REQ-SCRN-010.A2 -->
- [x] For a custom POST, a 403 with a stale cached crumb triggers a refresh and a retry with the same JSON body. No bare request is sent.

<!-- id: REQ-SCRN-011 -->
## Response caching policy

Screener endpoints (`CacheEndpoint::Screener`) skip the response cache under `CacheMode::Default`. `CacheMode::Use` or `CacheMode::Refresh` opts them in.

- Predefined GET: the builder handles the cache itself, keyed by the URL without the crumb. It reads from the cache first. On a miss it fetches with the core cache bypassed, parses the body fully, and writes it to the cache only after parsing succeeds.
- Custom POST: the core net layer caches it, keyed by URL plus serialised body (`cache_body`). It validates entries with `validate_screener_body`, which only rejects bodies whose `finance.error` is set.

implementations:
  - src/screener/builder.rs:ScreenerBuilder.fetch_predefined_get
  - src/screener/builder.rs:ScreenerBuilder.post_with_auth_retry
  - src/screener/response.rs:validate_screener_body
  - src/core/client/retry.rs:CacheEndpoint

verifies:
  - tests/screener/offline.rs:predefined_screener_does_not_cache_successful_status_api_error_body
  - tests/screener/offline.rs:explicit_custom_screener_cache_mode_uses_post_body_cache

<!-- id: REQ-SCRN-011.A1 -->
- [x] With `CacheMode::Use`, an HTTP 200 Yahoo error body for a predefined GET is never cached: two fetches give two upstream calls and two `YfError::Api`.
<!-- id: REQ-SCRN-011.A2 -->
- [x] With `CacheMode::Use`, two identical custom POSTs make exactly one upstream call.
<!-- id: REQ-SCRN-011.A3 -->
- [ ] Under `CacheMode::Default`, screener responses are never read from or written to the cache. [needs review — no screener-specific test backs this; the policy lives in `src/core/client/retry.rs`]
<!-- id: REQ-SCRN-011.A4 -->
- [ ] A custom POST body that passes the error-envelope check but cannot be parsed is not cached. [needs review — the code does not appear to guarantee this; see SCRN-REVIEW F3]

<!-- id: REQ-SCRN-012 -->
## Endpoint overrides and per-call options

Every `ScreenerBuilder<U>` has:
- `.screener_base(Url)`, which overrides the POST base.
- `.predefined_screener_base(Url)`, which overrides the GET base.
- Per-call option setters generated by `impl_call_option_setters!`: `.cache_mode(CacheMode)`, `.retry_policy(Option<RetryConfig>)`, `.data_quality(DataQuality)` and `.strict()`.

The base overrides are used as test seams for `httpmock`. Any other purpose (for example a proxy) is `[purpose unclear]`.

implementations:
  - src/screener/builder.rs:ScreenerBuilder.screener_base
  - src/screener/builder.rs:ScreenerBuilder.predefined_screener_base
  - src/core/call_options.rs:impl_call_option_setters

verifies:
  - tests/screener/offline.rs:offline_predefined_day_gainers_uses_get_with_expected_params
  - tests/screener/offline.rs:offline_custom_equity_query_posts_python_wire_shape

<!-- id: REQ-SCRN-012.A1 -->
- [x] Requests go to the overridden base URL when one is set (all offline tests depend on this).
<!-- id: REQ-SCRN-012.A2 -->
- [ ] `.retry_policy(..)` changes the retry behaviour of screener requests. [needs review — no test backs this]

<!-- id: SCRN-NONGOALS -->
## Non-goals

These are taken from explicit code signals:
- Arbitrary or raw Yahoo predefined screen IDs are not accepted (`src/screener/predefined.rs:9-12`).
- Arbitrary field names and free-form values are not accepted. Vocabularies are closed, and `ScreenerValue` is sealed (`src/screener/mod.rs:3-5`, `src/screener/query.rs:543-552`).
- Regions other than US are not supported (`Region` has one variant, `src/screener/query.rs:256-259`). Whether this is deliberate or just not done yet is `[needs review — intent not stated]`.
- Yahoo `must_not` and other operators besides EQ/GT/GTE/LT/LTE/BTWN/AND/OR are not modelled. `[needs review — inferred from absence, not stated]`
- Anything else: `[fill in — not derivable from code]`

<!-- id: SCRN-OPEN -->
## Open questions

1. Should `ScreenerResponse.count` mean Yahoo's page `count` or its `total`? Should `total` and `start` be exposed? (See F1.)
2. Is the offset-triggered fallback to locally reconstructed predefined criteria intended? If so, how are those criteria kept in sync with Yahoo's `rawCriteria`? (See F2.)
3. Does Yahoo accept a single-operand `OR` produced by `one_of([x])`?
4. Should `PercentField` support `eq` and `between`, as `NumericField` does?
5. Should the resolved `currency` be exposed on `ScreenerResult` when `price` and `market_cap` are both missing? Today it is consumed by `WireQuote` and does not appear in `fields`.
6. Should `between(lo, hi)` check that `lo <= hi`?

<!-- id: SCRN-REVIEW -->
## Code review findings

Ranked by severity.

<!-- id: SCRN-REVIEW-F1 -->
### F1 (High): `ScreenerResponse.count` is labelled "total" but holds the page size

- `src/screener/response.rs:24-25` documents the field as "Total count reported by Yahoo".
- `src/screener/response.rs:78` and `:131-135` actually map `result[0].count`.
- In both fixtures `count` is 25, the page size. Yahoo's real total is `total`: 19649 in `tests/fixtures/screener_custom_equity.json` and 174 in `tests/fixtures/screener_predefined_day_gainers.json`.
- `total` and `start` are dropped by `WireResult`, so callers cannot paginate correctly.
- `examples/15_screeners.rs:110-114` prints `count` as "Yahoo reported {total} total results", which is misleading.

<!-- id: SCRN-REVIEW-F2 -->
### F2 (High): The predefined screen with an offset silently runs different criteria

- `src/screener/builder.rs:277-283` switches to a POST of `custom_parts()` as soon as any offset is set, including `ResultOffset::new(0)`.
- The hard-coded `DayGainers` criteria at `src/screener/predefined.rs:94-104` do not match Yahoo's recorded `rawCriteria`. Yahoo also applies a `must_not` exclusion of the OTC exchanges PNK/OQB/OQX/OEM/OGM/XXX/OBB, and expresses market cap as an OR of BTWN ranges.
- Page 2 of a predefined screen can therefore contain OTC tickers that page 1 never would.
- No test covers this path or any of the 19 reconstructions.

<!-- id: SCRN-REVIEW-F3 -->
### F3 (Medium): The custom POST cache can store bodies that cannot be parsed

- `validate_screener_body` (`src/screener/response.rs:107-110`), used at `src/screener/builder.rs:415`, only rejects bodies with `finance.error`.
- A body with `result: null` or missing `quotes` passes that check and is cached under `CacheMode::Use`. Every later call then fails with `MissingData` until the TTL expires.
- The GET path was fixed to parse before writing to the cache (CHANGELOG, "parsed before response-cache writes"). The POST path did not get the same fix.

<!-- id: SCRN-REVIEW-F4 -->
### F4 (Medium): Large test gaps

There are no tests for:
- the fund and ETF builders, or their `MUTUALFUND`/`ETF` `quoteType`;
- `sort_by`, `count` and `offset` in the POST body;
- predefined `count` in the GET query;
- the predefined-with-offset POST path;
- any `custom_parts` variant;
- `between` serialisation;
- the `MissingData` paths;
- unsupported `quoteType` diagnostics, instrument-construction failure, or dropped non-object quotes;
- the `longName` fallback and the `fields` passthrough.

<!-- id: SCRN-REVIEW-F5 -->
### F5 (Low-Medium): `one_of` accepts inputs that `and`/`or` reject

`one_of` with one value (`src/screener/query.rs:821-846`) builds a single-operand `OR`. `ScreenerQuery::logical` (`src/screener/query.rs:881`) rejects one operand with "requires at least two operands". The rules are inconsistent, and Yahoo may reject the single-operand query.

<!-- id: SCRN-REVIEW-F6 -->
### F6 (Low): Recorded fixtures for different custom queries overwrite each other

The fixture key for a custom POST is just the lower-cased quote type (`src/screener/builder.rs:338`). Recording two different equity queries writes to the same `screener_custom_equity.json`, and each recording silently overwrites the previous one.

<!-- id: SCRN-REVIEW-F7 -->
### F7 (Low): Inconsistent trimming during parsing

- `quote_type` and `yahoo_exchange` are parsed from untrimmed strings (`src/screener/response.rs:238-241`, `:280-283`).
- `exchange` and `asset_kind` use the trimmed `nonempty()` value (`:243`, `:257`, `:263`).
- For a value with surrounding whitespace, such as `" NMS "`, `exchange` parses but `yahoo_exchange` is `None`, and `quote_type` can be `None` while `instrument` is still built.

<!-- id: SCRN-REVIEW-F8 -->
### F8 (Low): `between` does not check the order of its bounds

`NumericField::between` (`src/screener/query.rs:647-657`) does not check `lo <= hi`. An inverted range is sent to Yahoo unchanged and silently returns no results. `[needs review — Yahoo's behaviour with inverted BTWN is unverified]`

<!-- id: SCRN-REVIEW-F9 -->
### F9 (Low, design smell): Assorted API inconsistencies

- `ScreenerBuilder<Predefined>` exposes `.screener_base`, and `set_sort` (`src/screener/builder.rs:247-258`) silently does nothing for predefined builders. It is not callable publicly, but the shared `RequestKind` makes the type-state leaky.
- `PredefinedScreener::custom_parts` returns `Result` for inputs that are effectively constant (`src/screener/predefined.rs:84`).
- `YahooQuoteType::asset_kind` uses `expect` (`src/screener/query.rs:58-60`), which relies on two vocabularies staying in sync.

<!-- id: SCRN-NOTES -->
## Reverse-engineering notes

- **High confidence:** the wire shapes for the GET parameters, the POST body and the query encoding, all of which have exact-match `httpmock` and unit tests (REQ-SCRN-002, 004, 006); authentication retry (REQ-SCRN-010); cache error-body behaviour (REQ-SCRN-011.A1/A2); and the currency and diagnostics behaviour that is tested (REQ-SCRN-008, 009).
- **Medium confidence:** the vocabulary wire strings (REQ-SCRN-005) and the response field mapping beyond what tests assert (REQ-SCRN-007). These are read directly from the code but not checked against Yahoo.
- **Low confidence:** whether the predefined offset fallback (REQ-SCRN-003) is intended product behaviour or a workaround. The code makes it deliberate, but whether the divergence from Yahoo's criteria is intended is unknown.
- **Tests not executed.** `cargo test` could not run locally because the installed rustc is 1.88 and the crate MSRV is 1.91 (paft 0.9 needs 1.90). `[x]` marks therefore mean "an existing, non-ignored test asserts this". They do not mean "observed passing in this session". [needs review — re-run `cargo test --test screener` and `cargo test --lib screener` on a supported toolchain]
- Live tests (`tests/screener/live.rs:live_predefined_and_custom_smoke` and `tests/auth/live.rs:live_cached_crumb_optional_endpoints_smoke`) are `#[ignore]` and gated by `live_or_record_enabled()`. They are listed under `verifies:` but are not counted as backing any `[x]` criterion.
- Unit tests in `src/screener/*.rs` are cited as `src/...:test_fn` because that is where they are defined.
