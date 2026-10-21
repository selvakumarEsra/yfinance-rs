# yfinance-rs Constitution

## What this codebase is

`yfinance-rs` is a published Rust library crate (crates.io, MIT) that wraps
Yahoo Finance's unofficial HTTP and WebSocket APIs behind an async,
Python-`yfinance`-like API. Upstream: Yahoo endpoints on `query1`/`query2.finance.yahoo.com`,
`fc.yahoo.com` (cookie), `streamer.finance.yahoo.com` (WebSocket), and Business
Insider (ISIN lookup). Downstream: any Rust consumer; public models are mostly
re-exported from the provider-agnostic `paft` crate family.

**Project type:** `library`

**Backend language:** `none` (Rust library; no HTTP server)

**Frontend language:** `none`

## Non-negotiable invariants

- The public API is a semver contract. Adding public items is fine; removing,
  renaming, or changing signatures/field types needs a version bump and a
  `CHANGELOG.md` entry. New public structs/enums are `#[non_exhaustive]`.
- MSRV is `rust-version = 1.91` (edition 2024). CI checks MSRV with
  `cargo check --all-targets --all-features --locked`; do not use newer std/language features.
- Every fetch builder exposes `fetch()` and `fetch_with_diagnostics()` returning
  `YfResponse<T>`, and takes per-call `CallOptions` via `impl_call_option_setters!()`.
  `Ticker` convenience methods come in `x()` / `x_with_diagnostics()` pairs.
- Projection from Yahoo's wire shape goes through `ProjectionContext`. Malformed,
  missing, or coerced fields are recorded as `YfWarning`s (best-effort mode) and
  become errors in `DataQuality::Strict`. Never silently `unwrap_or_default` a
  field Yahoo sent but we could not parse.
- Wire structs (`wire.rs`) are private and use `WireValue<T>` / `RawNum<T>` so a
  single bad field does not fail the whole response. Public models never expose
  Yahoo's raw JSON shape.
- All HTTP goes through `core::net` (`fetch_text_with_auth_retry` and friends) so
  retries, cookie/crumb auth, caching, fixture record/replay and redaction apply.
  Do not call `reqwest` directly from feature modules.
- Every Yahoo base URL is overridable (client builder `base_*` or a builder-level
  `*_base(Url)` setter) so tests can point at `httpmock`. Default URLs are `const`s.
- Secrets (cookies, crumbs, proxy credentials) are never logged or put in error
  text unredacted; use `core::redaction`.
- Default `cargo test` must not hit the network. Live tests are `#[ignore]` and
  gated on `YF_LIVE`/`YF_RECORD`; offline tests replay `tests/fixtures/`.
- Money values carry a currency (`paft::money::Money`); never emit bare floats for
  monetary amounts in public models. Prefer `paft::Decimal` over `f64` for prices.

## Conventions worth following

- Feature module layout: `src/<area>/{mod.rs (builder + re-exports), api.rs or fetch.rs
  (fetch + projection), wire.rs (serde), model.rs (public types)}`.
- Reuse `paft` types when one fits; otherwise add a local `#[non_exhaustive]`
  `Serialize + Deserialize + Clone + Debug + PartialEq` model in the module's `model.rs`.
- quoteSummary features use `core::quotesummary::fetch_module_result`.
- Tests: one `tests/<area>.rs` entry file that `#[path]`-includes `tests/<area>/*.rs`;
  synthetic tests build a preauthed client (`._preauth("cookie", "crumb")`) against `httpmock`.
- Fixture filenames: `<caller>_api_<modules>_<SYMBOL>.json` (see `tests/common.rs`).
- Conventional Commits; update `CHANGELOG.md` under `Unreleased` for user-visible changes.
- Specs live in `specs/` (SpecShip format); link code via `implementations:` blocks.

## How to verify work is done

- `cargo fmt --all -- --check` clean
- `just lint` clean (clippy all/pedantic/nursery/cargo, `-D warnings`, all features)
- `just test-offline` passes (`cargo test --features test-mode,dataframe,stream`)
- `cargo +1.91.0 check --workspace --all-targets --all-features --locked` passes (MSRV)
- New public API has rustdoc with `# Errors`, a synthetic httpmock test, and is
  listed in `README.md` Features if user-visible

## Where things live

- `src/core/` — `YfClient` + builder, auth (cookie/crumb), `net` (HTTP, retry, cache),
  errors, diagnostics (`ProjectionContext`, `YfWarning`, `DataQuality`), currency resolution
- `src/ticker/` — `Ticker` facade; `info`, `fast_info`, ISIN, options
- `src/{history,download,quote,stream,fundamentals,analysis,holders,esg,profile,news,search,screener}/` — one feature area each
- `tests/` — integration tests; `tests/fixtures/` recorded Yahoo responses; `tests/common.rs` helpers
- `examples/` — numbered runnable examples (some need `--features stream`/`dataframe`)
- `specs/` — SpecShip specs (reverse-engineered `draft-reverse-engineered` + feature specs)
- `justfile` — test/lint recipes; `.github/workflows/` — CI (MSRV, fmt, clippy, offline tests, live smoke)

## Domain glossary

- crumb — anti-CSRF token from `/v1/test/getcrumb`, required (with cookie) by most Yahoo JSON endpoints
- quoteSummary — `v10/finance/quoteSummary/{symbol}?modules=…`; one endpoint, many named modules
- fundamentals-timeseries — `ws/fundamentals-timeseries/v1/finance/timeseries/{symbol}`; statement line items by `annual*`/`quarterly*`/`trailing*` keys
- visualization — `v1/finance/visualization` POST API backing Yahoo's earnings/IPO/splits/economic calendars
- paft — provider-agnostic finance model crates this library projects into
- projection — mapping Yahoo wire JSON into public models, with diagnostics
- best-effort vs strict — `DataQuality` modes: warn-and-continue vs fail on any projection issue
- fixture record/replay — `test-mode` feature; `YF_RECORD=1` writes, default replays `tests/fixtures/`
