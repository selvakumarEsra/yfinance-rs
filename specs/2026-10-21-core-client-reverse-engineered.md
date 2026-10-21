---
id: CORE-DOC
title: Core client (YfClient, auth, retry, caching, errors, diagnostics, currency) — reverse-engineered
owner: unassigned
priority: high
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/lib.rs
  - src/core/mod.rs
  - src/core/client/mod.rs
  - src/core/client/auth.rs
  - src/core/client/retry.rs
  - src/core/client/constants.rs
  - src/core/client/urls.rs
  - src/core/net.rs
  - src/core/error.rs
  - src/core/call_options.rs
  - src/core/redaction.rs
  - src/core/quotesummary.rs
  - src/core/quotes.rs
  - src/core/services.rs
  - src/core/models.rs
  - src/core/logging.rs
  - src/core/fixtures.rs
  - src/core/conversions.rs
  - src/core/yahoo_vocab.rs
  - src/core/currency.rs
  - src/core/wire.rs
  - src/core/wire/number.rs
  - src/core/wire/raw.rs
  - src/core/wire/value.rs
  - src/core/diagnostics/mod.rs
  - src/core/diagnostics/context.rs
  - src/core/diagnostics/currency.rs
  - src/core/diagnostics/issue.rs
  - src/core/diagnostics/monetary.rs
  - src/core/diagnostics/numeric.rs
  - src/core/diagnostics/projection.rs
  - src/core/diagnostics/response.rs
  - src/core/diagnostics/warning.rs
  - src/core/currency_resolver/mod.rs
  - src/core/currency_resolver/cache.rs
  - src/core/currency_resolver/enrichment.rs
  - src/core/currency_resolver/evidence.rs
  - src/core/currency_resolver/hints.rs
  - src/core/currency_resolver/inference.rs
  - src/core/currency_resolver/policy.rs
  - src/core/currency_resolver/resolver.rs
  - src/core/currency_resolver/types.rs
  - src/core/currency_resolver/unit.rs
  - src/core/currency_resolver/tests.rs
  - tests/common.rs
  - tests/auth.rs
  - tests/auth/crumb_retry_synthetic.rs
  - tests/auth/negative_cookie_crumb.rs
  - tests/auth/live.rs
  - tests/currency.rs
  - tests/currency/inference_offline.rs
  - tests/currency/inference_live.rs
  - tests/currency/verification.rs
  - tests/core.rs
  - tests/core/public_validation.rs
  - tests/core/conversions.rs
  - tests/history/caching_synthetic.rs
  - tests/history/retry_synthetic.rs
  - tests/history/http_status_error.rs
  - tests/quotes_status_mapping.rs
  - tests/quotes/offline.rs
  - tests/fundamentals/status_synthetic.rs
  - tests/ticker/actions.rs
  - tests/news/offline.rs
  - tests/stream/offline.rs
  - examples/11_builder_configuration.rs
  - examples/12_advanced_client.rs
  - examples/13_custom_client_and_proxy.rs
  - examples/16_diagnostics_audit.rs
---

<!-- id: CORE-DOC -->
# Core client: YfClient, auth, retry, caching, errors, diagnostics, currency (reverse-engineered)

Intent: `src/core` is the shared foundation every feature module (history, quote, fundamentals, analysis, holders, news, screener, stream, ticker) builds on. It provides a cloneable, shareable async `YfClient` configured through `YfClientBuilder`; transparent acquisition and refresh of Yahoo's cookie + crumb credentials; configurable retry with exponential backoff; an opt-in in-memory response cache with per-endpoint TTLs and per-call cache modes; a single public error type (`YfError`) whose display never leaks credentials; a data-quality diagnostics model (`DataQuality`, `YfResponse`, `YfWarning`) that lets callers choose between best-effort and strict projection of Yahoo wire data; and a currency resolver that attaches a currency to monetary values using provider fields first and heuristics as a fallback. The crate root (`src/lib.rs`) re-exports these core types and selected `paft` model types. [needs review — intent inferred from module docs, code, and tests; no product/design documentation exists in the repo]

<!-- id: REQ-CORE-001 -->
## Client construction, defaults and endpoint configuration

`YfClient` is `Clone` (all shared state is behind `Arc`) and is built via `YfClient::builder() -> YfClientBuilder` or `YfClient::default()` (which calls `builder().build().expect("default client")`).

Public builder surface (all `#[must_use]`, consuming `self`):
- `user_agent(impl Into<String>)` — default `constants::USER_AGENT` (a Chrome 122 desktop UA). Applied per-request in `send_with_retry`, not at the reqwest client level.
- Base URL overrides, each `fn(Url) -> Self`: `base_chart` (default `https://query1.finance.yahoo.com/v8/finance/chart/`), `base_quote_api` (`.../v10/finance/quoteSummary/`), `base_quote_v7` (`.../v7/finance/quote`), `base_options_v7` (`.../v7/finance/options/`), `base_stream` (`wss://streamer.finance.yahoo.com/?version=2`), `base_news` (`https://finance.yahoo.com`), `base_insider_search` (Business Insider `SearchController_Suggest`), `base_timeseries` (`https://query2.../fundamentals-timeseries/v1/finance/timeseries/`), `cookie_url` (`https://fc.yahoo.com/consent`), `crumb_url` (`https://query1.../v1/test/getcrumb`).
- `timeout(Duration)` (default 30 s) and `connect_timeout(Duration)` (default 10 s).
- `build(self) -> Result<YfClient, YfError>` — fails with `YfError::Url` if a default URL fails to parse, `YfError::Http` if the reqwest client fails to build, or `YfError::InvalidParams` if the retry config is invalid (see REQ-CORE-003).
- `cache_enabled(&self) -> bool`, `retry_config(&self) -> &RetryConfig` are read accessors on the built client.
- Test-only: `_preauth(cookie, crumb)` (`#[doc(hidden)]`, `cfg(any(test, feature = "test-mode"))`) seeds initial credentials.

The default reqwest client is built **without** a cookie store; Yahoo cookies are attached explicitly by the auth layer (REQ-CORE-005).

implementations:
  - src/core/client/mod.rs:YfClient
  - src/core/client/mod.rs:YfClient.builder
  - src/core/client/mod.rs:YfClientBuilder
  - src/core/client/mod.rs:YfClientBuilder.build
  - src/core/client/mod.rs:YfClientBuilder.http_timeouts
  - src/core/client/mod.rs:YfClientBuilder.configured_url
  - src/core/client/mod.rs:YfClientBuilder.user_agent
  - src/core/client/mod.rs:YfClientBuilder.timeout
  - src/core/client/mod.rs:YfClientBuilder.connect_timeout
  - src/core/client/mod.rs:YfClientBuilder._preauth
  - src/core/client/mod.rs:YfClient.cache_enabled
  - src/core/client/mod.rs:YfClient.retry_config
  - src/core/client/constants.rs:USER_AGENT
verifies:
  - src/core/client/mod.rs:builder_defaults_to_bounded_http_timeouts
  - src/core/client/mod.rs:builder_http_timeouts_are_overridable
  - src/core/client/mod.rs:builder_default_http_client_does_not_store_ambient_cookies
  - src/core/client/mod.rs:preauth_sets_initial_credentials_in_normal_test_builds
  - tests/core/public_validation.rs:configured_timeout_retries_hanging_quote_requests

<!-- id: REQ-CORE-001.A1 -->
- [x] With no overrides, the builder resolves request timeout = 30 s and connect timeout = 10 s; `timeout()`/`connect_timeout()` override them.
<!-- id: REQ-CORE-001.A2 -->
- [x] A `Set-Cookie` returned by an arbitrary response is not stored and replayed by the default HTTP client.
<!-- id: REQ-CORE-001.A3 -->
- [x] `_preauth(cookie, crumb)` seeds the client's credential state before any request.
<!-- id: REQ-CORE-001.A4 -->
- [x] A configured request timeout bounds a hanging request (the request fails with `YfError::Http` instead of hanging).
<!-- id: REQ-CORE-001.A5 -->
- [ ] The configured `user_agent` is sent on every HTTP request, including cookie/crumb requests. [needs review — no test backs this]
<!-- id: REQ-CORE-001.A6 -->
- [ ] `YfClient::default()` panics if the underlying HTTP client cannot be built. [needs review — no test backs this; panic-on-default may be unintended]

<!-- id: REQ-CORE-002 -->
## Custom HTTP client and proxy configuration

- `custom_client(reqwest::Client) -> Self` — the supplied client is used as-is; builder `timeout`, `connect_timeout` and proxy settings are **ignored** (documented). WebSocket startup also uses it for the HTTP upgrade request.
- `try_proxy(&str) -> Result<Self, YfError>` — validates with `Url::parse` and `reqwest::Proxy::all`; invalid input → `YfError::InvalidParams("invalid proxy URL format: ...")`.
- `try_https_proxy(&str) -> Result<Self, YfError>` — same with `reqwest::Proxy::https`.
- Both proxy setters write the same single `proxy` slot (last call wins).

implementations:
  - src/core/client/mod.rs:YfClientBuilder.custom_client
  - src/core/client/mod.rs:YfClientBuilder.try_proxy
  - src/core/client/mod.rs:YfClientBuilder.try_https_proxy
  - src/core/client/mod.rs:YfClientBuilder.build
verifies:
  - tests/core/public_validation.rs:general_proxy_routes_https_requests_through_proxy
  - tests/core/public_validation.rs:general_proxy_routes_websocket_startup_through_proxy
  - tests/core/public_validation.rs:custom_client_proxy_routes_websocket_startup_through_proxy
  - tests/auth/crumb_retry_synthetic.rs:custom_client_without_cookie_store_sends_auth_cookie

<!-- id: REQ-CORE-002.A1 -->
- [x] With `try_proxy`, HTTPS quote requests are sent to the proxy as `CONNECT <target>` and never connect directly to the target.
<!-- id: REQ-CORE-002.A2 -->
- [x] With `try_proxy` (feature `stream`), WebSocket startup is routed through the proxy.
<!-- id: REQ-CORE-002.A3 -->
- [x] A custom reqwest client's own proxy is honoured for WebSocket startup.
<!-- id: REQ-CORE-002.A4 -->
- [x] A custom client without a cookie store still sends the Yahoo auth cookie on crumb and API requests.
<!-- id: REQ-CORE-002.A5 -->
- [ ] `try_proxy("invalid-url")` and `try_https_proxy("not-a-url")` return `YfError::InvalidParams`. [needs review — no test backs this; only demonstrated in examples/13_custom_client_and_proxy.rs]
<!-- id: REQ-CORE-002.A6 -->
- [ ] When `custom_client` is set, builder `timeout`/`connect_timeout`/proxy have no effect. [needs review — no test backs this; documented only]

<!-- id: REQ-CORE-003 -->
## Retry policy model and validation

Public types (`src/core/client/retry.rs`, re-exported at crate root):

```rust
pub enum Backoff {
    Fixed(Duration),
    Exponential { base: Duration, factor: f64, max: Duration, jitter: bool },
}
pub struct RetryConfig {
    pub enabled: bool,              // default true
    pub max_retries: u32,           // default 4 (total attempts = max_retries + 1)
    pub backoff: Backoff,           // default Exponential{200ms, 2.0, 3s, jitter: true}
    pub retry_on_status: Vec<u16>,  // default [408, 429, 500, 502, 503, 504]
    pub retry_on_timeout: bool,     // default true
    pub retry_on_connect: bool,     // default true
}
impl RetryConfig { pub const MAX_RETRIES: u32 = 100; pub fn validate(&self) -> Result<(), YfError>; }
impl Backoff { pub fn validate(&self) -> Result<(), YfError>; }
```

- `Backoff::validate` rejects an exponential `factor` that is non-finite or `<= 0.0` with `InvalidParams("...factor must be finite and greater than zero")`.
- `RetryConfig::validate` skips all checks when `enabled == false`; otherwise rejects `max_retries > 100` and delegates to `Backoff::validate`.
- Validation runs at `YfClientBuilder::build` (client default) **and** at the start of every `send_with_retry` (covers per-call overrides).
- Builder helpers: `retry_config(RetryConfig)`, `retry_enabled(bool)` (mutates the existing or default config).

implementations:
  - src/core/client/retry.rs:Backoff
  - src/core/client/retry.rs:Backoff.validate
  - src/core/client/retry.rs:RetryConfig
  - src/core/client/retry.rs:RetryConfig.validate
  - src/core/client/retry.rs:RetryConfig.default
  - src/core/client/mod.rs:YfClientBuilder.retry_config
  - src/core/client/mod.rs:YfClientBuilder.retry_enabled
verifies:
  - tests/core/public_validation.rs:client_builder_rejects_invalid_retry_backoff_factors
  - tests/core/public_validation.rs:client_builder_rejects_excessive_retry_counts
  - tests/core/public_validation.rs:per_call_retry_override_is_validated_before_request

<!-- id: REQ-CORE-003.A1 -->
- [x] `build()` fails with `InvalidParams` mentioning "factor" for factor ∈ {NaN, +∞, −∞, −1.0, 0.0}.
<!-- id: REQ-CORE-003.A2 -->
- [x] `build()` fails with `InvalidParams` mentioning "max_retries" when `max_retries = MAX_RETRIES + 1`.
<!-- id: REQ-CORE-003.A3 -->
- [x] An invalid per-call `retry_policy(Some(..))` override fails with `InvalidParams` before any request is sent.
<!-- id: REQ-CORE-003.A4 -->
- [ ] A disabled policy (`enabled: false`) with otherwise-invalid fields is accepted. [needs review — no test backs this]

<!-- id: REQ-CORE-004 -->
## Retry execution and backoff computation

`YfClient::send_with_retry(&self, req: RequestBuilder, override_retry: Option<&RetryConfig>) -> Result<Response, YfError>` (crate-private) is the single send path for all HTTP traffic.

- Always sets the `User-Agent` header; uses `override_retry` or the client default; validates it.
- If retries are disabled: one `send()`; transport errors map to `YfError::Http`.
- Otherwise loops: each attempt `try_clone()`s the request (failure → `YfError::RequestNotCloneable`). A response whose status is in `retry_on_status` is retried while `attempt < max_retries`; transport errors are retried when `(retry_on_timeout && is_timeout) || (retry_on_connect && is_connect)`. After exhausting retries the final response (even a retryable status) is returned `Ok` and callers map the status (REQ-CORE-011); the final transport error is returned as `YfError::Http`.
- Backoff (`compute_backoff_duration`): `Fixed(d)` → `d`. `Exponential` → `min(base * factor^attempt, max)`, saturating to `max` on overflow/non-finite. With `jitter`, the delay is drawn uniformly (rejection sampling over `getrandom` bytes) from `[d/2, min(1.5·d, max)]`; if randomness is unavailable, the un-jittered delay is used.
- With feature `tracing`, each retry emits an INFO event with redacted error text.

implementations:
  - src/core/client/mod.rs:YfClient.send_with_retry
  - src/core/client/mod.rs:sleep_backoff
  - src/core/client/mod.rs:compute_backoff_duration
  - src/core/client/mod.rs:compute_backoff_duration_with_random
  - src/core/client/mod.rs:jitter_duration
  - src/core/client/mod.rs:random_nanos_inclusive
  - src/core/client/mod.rs:random_u128
  - src/core/client/mod.rs:duration_from_nanos
verifies:
  - tests/history/retry_synthetic.rs:history_retries_on_persistent_5xx
  - tests/core/public_validation.rs:configured_timeout_retries_hanging_quote_requests
  - tests/history/http_status_error.rs:history_returns_status_error_on_non_2xx
  - src/core/client/mod.rs:exponential_jitter_uses_random_input
  - src/core/client/mod.rs:exponential_jitter_respects_max_delay
  - src/core/client/mod.rs:exponential_backoff_saturates_huge_attempts_at_max_delay

<!-- id: REQ-CORE-004.A1 -->
- [x] A persistently-503 endpoint with `max_retries = 3` is called exactly 4 times and the caller receives `YfError::ServerError { status: 503, .. }`.
<!-- id: REQ-CORE-004.A2 -->
- [x] Request timeouts are retried: with `max_retries = 2`, three attempts reach the server, then `YfError::Http` is returned.
<!-- id: REQ-CORE-004.A3 -->
- [x] With retries disabled, a 500 surfaces immediately as `ServerError { status: 500 }`.
<!-- id: REQ-CORE-004.A4 -->
- [x] Exponential jitter for base 100 ms, attempt 0, spans exactly 50 ms..150 ms depending on the random input.
<!-- id: REQ-CORE-004.A5 -->
- [x] Jittered delay never exceeds `max`.
<!-- id: REQ-CORE-004.A6 -->
- [x] `attempt = u32::MAX` saturates to `max` without panicking.
<!-- id: REQ-CORE-004.A7 -->
- [ ] A non-cloneable request body returns `YfError::RequestNotCloneable` when retries are enabled. [needs review — no test backs this]
<!-- id: REQ-CORE-004.A8 -->
- [ ] `Retry-After` response headers are not consulted. [needs review — observed absence in code; confirm whether intentional]

<!-- id: REQ-CORE-005 -->
## Cookie and crumb acquisition

Credentials live in `Arc<RwLock<ClientState { cookie: Option<String>, crumb: Option<String> }>>`, shared by all clones of a client.

- `ensure_credentials()` (crate-private): fast path returns if a crumb is present; otherwise takes a dedicated `tokio::sync::Mutex` (single-flight), re-checks, then fetches cookie then crumb.
- Cookie fetch: `GET cookie_url` via `send_with_retry`; the HTTP status is **not** checked. All `Set-Cookie` headers are reduced to their `name=value` pair and joined with `"; "`. A non-UTF-8 or pair-less header → `Auth("Invalid cookie header format")`; no headers → `Auth("No cookie received from fc.yahoo.com")`.
- Crumb fetch: `GET crumb_url` with `Cookie` header; non-2xx → status error (REQ-CORE-011); body is trimmed and validated: rejected (`Auth("Received invalid crumb response")`, never echoing the body) if empty, > 128 bytes, contains control/whitespace chars, `<`/`>`, starts with `{`/`[`, or contains (case-insensitive) any of: "too many requests", "unauthorized", "forbidden", "invalid crumb", "invalid credentials", "rate limit", "not found", "error".
- `with_auth_cookie(req)` attaches the stored cookie; `clear_crumb()` drops only the crumb.
- `ClientState` and `YfClient` `Debug` impls never print cookie/crumb values.

implementations:
  - src/core/client/auth.rs:YfClient.ensure_credentials
  - src/core/client/auth.rs:YfClient.get_cookie
  - src/core/client/auth.rs:YfClient.get_crumb_internal
  - src/core/client/auth.rs:YfClient.clear_crumb
  - src/core/client/auth.rs:YfClient.crumb
  - src/core/client/auth.rs:YfClient.with_auth_cookie
  - src/core/client/auth.rs:cookie_header
  - src/core/client/auth.rs:validate_crumb_response
verifies:
  - tests/auth/crumb_retry_synthetic.rs:api_fetches_cookie_and_crumb_first
  - tests/auth/negative_cookie_crumb.rs:missing_set_cookie_header_is_an_error
  - tests/auth/negative_cookie_crumb.rs:invalid_crumb_body_is_an_error
  - tests/auth/negative_cookie_crumb.rs:crumb_error_phrase_body_is_not_used_as_credential
  - tests/auth/negative_cookie_crumb.rs:non_success_crumb_response_is_not_stored
  - tests/auth/negative_cookie_crumb.rs:successful_crumb_response_is_trimmed
  - src/core/client/auth.rs:crumb_response_validation_accepts_compact_tokens
  - src/core/client/auth.rs:crumb_response_validation_rejects_error_shapes
  - src/core/client/auth.rs:crumb_response_validation_rejects_suspiciously_long_values
  - src/core/client/mod.rs:client_state_debug_reports_presence_without_values

<!-- id: REQ-CORE-005.A1 -->
- [x] The first crumb-requiring call fetches the cookie, then the crumb, then calls the API with `crumb=<value>`.
<!-- id: REQ-CORE-005.A2 -->
- [x] Missing `Set-Cookie` → `YfError::Auth` containing "No cookie received"; crumb endpoint and API are not called.
<!-- id: REQ-CORE-005.A3 -->
- [x] A malformed crumb body (e.g. `secret-crumb<`) → `Auth("Received invalid crumb...")` and the body is not leaked into the error.
<!-- id: REQ-CORE-005.A4 -->
- [x] An error-phrase crumb body ("Too Many Requests") is never used as a credential.
<!-- id: REQ-CORE-005.A5 -->
- [x] A non-2xx (429) crumb response yields `YfError::RateLimited` and the API is not called with its body.
<!-- id: REQ-CORE-005.A6 -->
- [x] Surrounding whitespace on a valid crumb is trimmed before use.
<!-- id: REQ-CORE-005.A7 -->
- [x] Compact tokens like `abc-DEF_123./~` are accepted; values > 128 chars are rejected.
<!-- id: REQ-CORE-005.A8 -->
- [ ] Concurrent tasks needing credentials trigger only one cookie+crumb fetch. [needs review — no test backs this; single-flight is implemented via `credential_fetch_lock`]

<!-- id: REQ-CORE-006 -->
## Authenticated fetch with stale-crumb recovery

`net::fetch_text_with_auth_retry(client, base_url, AuthFetchConfig, build_request) -> Result<(String, Url), YfError>` (crate-private) handles endpoints needing a crumb.

- `AuthMode::RequiredCrumb` (quoteSummary): cache read → `ensure_credentials` → request with `crumb` appended.
- `AuthMode::OptionalCrumb` (v7 quote, others): cache read → if a crumb is already cached use it → otherwise a bare request; on 401/403 acquire credentials and retry with crumb; other non-2xx → status error.
- "Stale crumb" = 401/403 on a crumb request, or (when `retry_on_invalid_crumb_body`) a body ≤ 200 bytes containing (case-insensitive) "invalid crumb", "invalid credentials" or "unauthorized". Recovery (`retry_with_fresh_crumb`): evict the cache key, clear the crumb, re-acquire, retry once with cache mode `Refresh` (if the original mode writes) or `Bypass`. A second stale response → status error or `Auth("Yahoo returned an invalid crumb response after refreshing credentials")`.
- The cache key is the **base URL without crumb** (or `POST <url>\n<body>` when `cache_body` is set); a cached body that contains the invalid-crumb marker is evicted and triggers a refresh.
- Successful bodies are written to cache only if caching is enabled and the cache mode writes, after passing the optional `cache_validator`.

implementations:
  - src/core/net.rs:fetch_text_with_auth_retry
  - src/core/net.rs:fetch_optional_crumb_auth
  - src/core/net.rs:fetch_required_crumb_auth
  - src/core/net.rs:read_cached_or_refresh
  - src/core/net.rs:read_cached_auth_attempt
  - src/core/net.rs:fetch_text_auth_attempt
  - src/core/net.rs:finish_crumb_attempt
  - src/core/net.rs:retry_with_fresh_crumb
  - src/core/net.rs:retry_cache_mode
  - src/core/net.rs:ensure_crumb
  - src/core/net.rs:url_with_crumb
  - src/core/net.rs:should_retry_invalid_crumb_body
  - src/core/net.rs:has_invalid_crumb_marker
  - src/core/net.rs:is_auth_status
  - src/core/net.rs:AuthMode
  - src/core/net.rs:AuthFetchConfig
verifies:
  - tests/auth/crumb_retry_synthetic.rs:api_retries_on_invalid_crumb_then_succeeds
  - tests/auth/crumb_retry_synthetic.rs:api_retries_on_stale_crumb_http_status_then_succeeds
  - src/core/net.rs:invalid_crumb_body_detection_is_case_insensitive_and_size_bounded
  - src/core/net.rs:required_crumb_cache_hit_does_not_fetch_credentials
  - src/core/net.rs:optional_crumb_uses_cached_crumb_before_bare_request
  - src/core/net.rs:optional_crumb_refreshes_stale_cached_crumb_without_bare_retry
  - src/core/net.rs:cached_invalid_crumb_body_is_evicted_before_fresh_crumb_retry
  - src/core/net.rs:invalid_crumb_body_after_refresh_returns_auth_error

<!-- id: REQ-CORE-006.A1 -->
- [x] A 200 body "Invalid Crumb" with a stale crumb triggers exactly one cookie+crumb refresh and a successful retry with the fresh crumb.
<!-- id: REQ-CORE-006.A2 -->
- [x] A 403 with a stale crumb triggers refresh and a successful retry.
<!-- id: REQ-CORE-006.A3 -->
- [x] Invalid-crumb body detection is case-insensitive and ignores bodies > 200 bytes.
<!-- id: REQ-CORE-006.A4 -->
- [x] A RequiredCrumb cache hit returns the cached body without calling cookie/crumb endpoints.
<!-- id: REQ-CORE-006.A5 -->
- [x] OptionalCrumb with a cached crumb never sends a bare (crumb-less) request.
<!-- id: REQ-CORE-006.A6 -->
- [x] OptionalCrumb with a stale cached crumb (401) refreshes and retries with the fresh crumb, without a bare request.
<!-- id: REQ-CORE-006.A7 -->
- [x] A cached invalid-crumb body is evicted, refreshed, and the fresh good body replaces it in cache.
<!-- id: REQ-CORE-006.A8 -->
- [x] An invalid-crumb body after refresh yields `YfError::Auth` and nothing is cached.
<!-- id: REQ-CORE-006.A9 -->
- [ ] OptionalCrumb bare request returning 401/403 acquires credentials and retries with crumb. [needs review — no direct test of the bare→401→crumb path]

<!-- id: REQ-CORE-007 -->
## In-memory response cache: enablement, TTLs, bounds, expiry

- The response cache (`CacheStore`, backed by `moka::sync::Cache<String, CacheEntry { body: Arc<str>, expires_at: Instant }>`) exists only if `cache_ttl(Duration)` or at least one `cache_ttl_for(CacheEndpoint, Duration)` was set. `no_cache()` clears both.
- TTL precedence: per-write override → per-endpoint TTL → global TTL; no TTL → the write is skipped.
- Capacity: `cache_max_entries(NonZeroUsize)` (default 1024) via moka `max_capacity`.
- Reads (`cache_get`/`cache_get_key`) return a shared `Arc<str>`; an expired entry is removed on read without scanning others. Writes (`cache_put`/`cache_put_key`) prune **all** expired entries, then insert.
- Keys: GET → full URL string; POST → `"POST <url>\n<body>"`.
- Public maintenance: `clear_cache(&self)` (clears the response cache **and** the currency/hint/instrument side caches, even if response caching is disabled); `invalidate_cache_entry(&self, &Url)` (removes GET and POST keys for that URL; no-op without cache).

implementations:
  - src/core/client/mod.rs:CacheStore
  - src/core/client/mod.rs:CacheStore.ttl_for
  - src/core/client/mod.rs:CacheStore.get_fresh_shared
  - src/core/client/mod.rs:CacheStore.insert
  - src/core/client/mod.rs:CacheStore.remove_url
  - src/core/client/mod.rs:CacheStore.prune_expired
  - src/core/client/mod.rs:YfClient.cache_get
  - src/core/client/mod.rs:YfClient.cache_get_key
  - src/core/client/mod.rs:YfClient.cache_put
  - src/core/client/mod.rs:YfClient.cache_put_key
  - src/core/client/mod.rs:YfClient.cache_remove_key
  - src/core/client/mod.rs:YfClient.clear_cache
  - src/core/client/mod.rs:YfClient.invalidate_cache_entry
  - src/core/client/mod.rs:YfClientBuilder.cache_ttl
  - src/core/client/mod.rs:YfClientBuilder.cache_ttl_for
  - src/core/client/mod.rs:YfClientBuilder.cache_max_entries
  - src/core/client/mod.rs:YfClientBuilder.no_cache
  - src/core/client/mod.rs:post_cache_key
  - src/core/client/mod.rs:cache_key_url
verifies:
  - src/core/client/mod.rs:cache_get_removes_expired_entry
  - src/core/client/mod.rs:cache_get_does_not_prune_unrelated_expired_entries
  - src/core/client/mod.rs:cache_get_fresh_hits_do_not_require_async_lock
  - src/core/client/mod.rs:cache_get_reuses_cached_body_allocation
  - src/core/client/mod.rs:cache_put_prunes_expired_entries
  - src/core/client/mod.rs:cache_put_bounds_entry_count
  - src/core/client/mod.rs:endpoint_ttl_enables_only_that_endpoint_without_global_ttl
  - tests/history/caching_synthetic.rs:history_serves_from_cache_on_second_call

<!-- id: REQ-CORE-007.A1 -->
- [x] Reading an expired entry returns `None` and removes it; unrelated expired entries are untouched by reads.
<!-- id: REQ-CORE-007.A2 -->
- [x] Two hits on the same key return the same `Arc` allocation.
<!-- id: REQ-CORE-007.A3 -->
- [x] A write prunes previously-expired entries.
<!-- id: REQ-CORE-007.A4 -->
- [x] With `cache_max_entries(2)`, at most 2 entries are retained after 3 writes.
<!-- id: REQ-CORE-007.A5 -->
- [x] `cache_ttl_for(Quote, ..)` alone caches Quote writes but not Chart writes.
<!-- id: REQ-CORE-007.A6 -->
- [x] With `cache_ttl` set, a repeated identical history request is served from cache (1 network call).
<!-- id: REQ-CORE-007.A7 -->
- [ ] `clear_cache()` empties response and side caches; `invalidate_cache_entry(url)` removes both GET and POST entries for that URL. [needs review — no test backs this]

<!-- id: REQ-CORE-008 -->
## Cache modes, endpoint defaults and body validation

```rust
#[derive(Default)] pub enum CacheMode { #[default] Default, Use, Refresh, Bypass }
pub enum CacheEndpoint { Quote, Chart, QuoteSummary, Fundamentals, Options, News, Search, Screener }
```

- `CacheMode::Default` resolves per endpoint: `Quote | Options | News | Screener` → Bypass (volatile); `Chart | QuoteSummary | Fundamentals | Search` → Use.
- `Use`: read fresh entry, else fetch and write. `Refresh`: always fetch, then write. `Bypass`: neither read nor write.
- `net::fetch_text_cached` (GET) and `net::fetch_json_post_cached` (POST) implement read/validate/fetch/write. An optional `CacheBodyValidator` (`fn(&str) -> Result<(), YfError>`) is applied to cached bodies (failing entries are evicted and refetched) and to fresh bodies before writing; error bodies (Yahoo API errors, non-2xx) are never cached. POST cached bodies that fail JSON deserialization are evicted.

implementations:
  - src/core/client/retry.rs:CacheMode
  - src/core/client/retry.rs:CacheMode.reads
  - src/core/client/retry.rs:CacheMode.writes
  - src/core/client/retry.rs:CacheEndpoint
  - src/core/client/retry.rs:CacheEndpoint.default_mode
  - src/core/net.rs:fetch_text_cached
  - src/core/net.rs:fetch_json_post_cached
  - src/core/net.rs:cache_write_enabled
  - src/core/net.rs:cached_body_is_valid
  - src/core/net.rs:validate_cache_body
  - src/core/net.rs:CacheFetchConfig
verifies:
  - tests/quotes/offline.rs:default_quote_cache_mode_bypasses_response_cache
  - tests/quotes/offline.rs:explicit_quote_cache_mode_uses_response_cache
  - tests/history/caching_synthetic.rs:history_cache_refresh_bypasses_cache_get_but_updates_cache
  - tests/news/offline.rs:default_news_cache_mode_bypasses_response_cache
  - tests/news/offline.rs:explicit_news_cache_mode_uses_body_aware_response_cache
  - tests/ticker/actions.rs:ticker_actions_respect_ticker_cache_bypass
  - tests/fundamentals/status_synthetic.rs:quote_summary_http_error_maps_status_and_is_not_cached
  - tests/fundamentals/status_synthetic.rs:quote_summary_api_error_is_not_cached
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_http_error_maps_status_and_is_not_cached
  - tests/fundamentals/status_synthetic.rs:fundamentals_timeseries_api_error_is_not_cached

<!-- id: REQ-CORE-008.A1 -->
- [x] With a global TTL and `CacheMode::Default`, two quote fetches make 2 network calls; with `CacheMode::Use`, 1 call.
<!-- id: REQ-CORE-008.A2 -->
- [x] `CacheMode::Refresh` refetches despite a cached entry and updates the cache.
<!-- id: REQ-CORE-008.A3 -->
- [x] News (POST) defaults to bypass; explicit `Use` caches with a body-aware key.
<!-- id: REQ-CORE-008.A4 -->
- [x] A ticker-level `CacheMode::Bypass` is honoured by derived calls.
<!-- id: REQ-CORE-008.A5 -->
- [x] HTTP error responses and Yahoo API-error bodies for quoteSummary and timeseries are not cached.
<!-- id: REQ-CORE-008.A6 -->
- [ ] Configuring `cache_ttl_for(Quote|Options|News|Screener, ..)` has no effect unless calls also pass `CacheMode::Use`. [needs review — derived from code; possibly surprising to users]

<!-- id: REQ-CORE-009 -->
## Bounded side caches (currency, currency hints, instruments)

`BoundedSideMap<K, V>` (crate-private) is a `RwLock<HashMap + VecDeque>` with FIFO-insertion eviction once `len > max_entries`. Three instances per client, sized by `side_cache_max_entries(NonZeroUsize)` (default 4096): `currency_cache` (`CurrencyCacheKey → ResolvedCurrency`), `currency_hints` (`String → CurrencyHints`), `instrument_cache` (`String → paft::domain::Instrument`). `compute_with` provides an atomic read-modify-write under the write lock. Lookups on hit do not clone keys.

implementations:
  - src/core/client/mod.rs:BoundedSideMap
  - src/core/client/mod.rs:BoundedSideMap.insert
  - src/core/client/mod.rs:BoundedSideMap.get_cloned
  - src/core/client/mod.rs:BoundedSideMap.get_str
  - src/core/client/mod.rs:BoundedSideMap.compute_with
  - src/core/client/mod.rs:BoundedSideMap.clear
  - src/core/client/mod.rs:BoundedSideEntries.evict_extra_entries
  - src/core/client/mod.rs:YfClient.cached_instrument
  - src/core/client/mod.rs:YfClient.store_instrument
  - src/core/client/mod.rs:YfClientBuilder.side_cache_max_entries
verifies:
  - src/core/client/mod.rs:bounded_side_map_hit_does_not_clone_keys
  - src/core/client/mod.rs:instrument_side_cache_bounds_entry_count
  - src/core/currency_resolver/tests.rs:currency_side_caches_bound_entry_count

<!-- id: REQ-CORE-009.A1 -->
- [x] A side-map hit performs zero key clones.
<!-- id: REQ-CORE-009.A2 -->
- [x] With `side_cache_max_entries(2)`, instrument and currency side caches hold at most 2 entries.
<!-- id: REQ-CORE-009.A3 -->
- [ ] Eviction is FIFO by first insertion (re-insertion does not refresh position). [needs review — no test backs this; LRU may have been intended]

<!-- id: REQ-CORE-010 -->
## Per-call options (cache mode, retry override, data quality)

`CallOptions { cache_mode: CacheMode (default Default), retry_override: Option<RetryConfig> (default None), data_quality: DataQuality (default BestEffort) }` is crate-private and embedded in feature builders. The `impl_call_option_setters!` macro generates the public builder methods used across feature modules:

```rust
pub const fn cache_mode(self, mode: CacheMode) -> Self;
pub fn retry_policy(self, cfg: Option<RetryConfig>) -> Self;
pub const fn data_quality(self, policy: DataQuality) -> Self;
pub const fn strict(self) -> Self; // = data_quality(DataQuality::Strict)
```

implementations:
  - src/core/call_options.rs:CallOptions
  - src/core/call_options.rs:CallOptions.new
  - src/core/call_options.rs:CallOptions.with_cache_mode
  - src/core/call_options.rs:CallOptions.with_retry_policy
  - src/core/call_options.rs:CallOptions.with_data_quality
  - src/core/call_options.rs:CallOptions.strict
  - src/core/call_options.rs:impl_call_option_setters
verifies:
  - tests/core/public_validation.rs:per_call_retry_override_is_validated_before_request
  - tests/history/caching_synthetic.rs:history_cache_refresh_bypasses_cache_get_but_updates_cache
  - tests/fundamentals/status_synthetic.rs:strict_calendar_errors_on_malformed_date

<!-- id: REQ-CORE-010.A1 -->
- [x] A per-call retry override is used (and validated) instead of the client default.
<!-- id: REQ-CORE-010.A2 -->
- [x] A per-call cache mode overrides the endpoint default.
<!-- id: REQ-CORE-010.A3 -->
- [x] `.strict()` converts the first projection issue into `YfError::DataQuality`.

<!-- id: REQ-CORE-011 -->
## Error taxonomy and HTTP status mapping

`#[non_exhaustive] pub enum YfError` variants: `Http(RedactedHttpError)`, `Websocket(WebsocketError)`, `Protobuf(ProtobufDecodeError)`, `Json(JsonError)`, `Base64(Base64DecodeError)`, `Url(UrlParseError)`, `NotFound { url }`, `RateLimited { url }`, `ServerError { status, url }`, `Status { status, url }`, `Api(String)`, `Auth(String)`, `Scrape(String)`, `MissingData(String)`, `InvalidData(String)`, `OptionUnderlyingTypeUnavailable { symbol, quote_type }`, `DataQuality(Box<YfWarning>)`, `InvalidParams(String)`, `RequestNotCloneable`, `Money(paft::money::MoneyError)`, `InvalidDates`.

- `status_error_code(status, &Url)`: 404 → `NotFound`, 429 → `RateLimited`, 500..=599 → `ServerError`, else `Status`; the URL is stored redacted.
- Foreign errors are wrapped in opaque newtypes so `reqwest`, `serde_json`, `url`, `tungstenite`, `prost`, `base64` are not part of the public type signature; `std::error::Error::source()` exposes the wrapped source for Websocket/Protobuf/Json/Base64/Url/Money (not Http).
- `From<reqwest::Error>` stores only redacted display text.

implementations:
  - src/core/error.rs:YfError
  - src/core/error.rs:RedactedHttpError
  - src/core/error.rs:WebsocketError
  - src/core/error.rs:ProtobufDecodeError
  - src/core/error.rs:JsonError
  - src/core/error.rs:Base64DecodeError
  - src/core/error.rs:UrlParseError
  - src/core/net.rs:status_error_code
  - src/core/net.rs:status_error
  - src/core/net.rs:get_success_text
  - src/core/net.rs:fetch_text
verifies:
  - src/core/error.rs:opaque_json_and_url_errors_preserve_foreign_sources
  - src/core/error.rs:opaque_stream_errors_preserve_foreign_sources
  - tests/quotes_status_mapping.rs:quotes_v7_404_maps_to_not_found
  - tests/quotes_status_mapping.rs:quotes_v7_429_maps_to_rate_limited
  - tests/history/http_status_error.rs:history_returns_status_error_on_non_2xx
  - tests/history/retry_synthetic.rs:history_retries_on_persistent_5xx

<!-- id: REQ-CORE-011.A1 -->
- [x] Json/Url (and, with `stream`, Websocket/Protobuf/Base64) errors expose the original error via `source()` and downcast to the foreign type.
<!-- id: REQ-CORE-011.A2 -->
- [x] 404 → `NotFound { url }`, 429 → `RateLimited { url }`, 5xx → `ServerError { status, url }`.
<!-- id: REQ-CORE-011.A3 -->
- [ ] Other non-2xx statuses (e.g. 400, 418) map to `Status { status, url }`. [needs review — no test backs this]

<!-- id: REQ-CORE-012 -->
## Credential redaction in errors, logs and Debug output

- `RedactedUrl<'a>` (`Display`/`Debug`) and `redact_url(&Url)` replace the value of sensitive query parameters with `REDACTED`. A parameter is sensitive if, after lower-casing and mapping `-`/`.` → `_`, it is one of `apikey, api_key, auth, authorization, cookie, csrf, jwt, key, secret, session, sid, sig, signature, token, xsrf`, contains `crumb`, or ends with `_cookie | _key | _secret | _signature | _token`.
- `redact_auth_query_params_in_text(&str)` finds embedded `http://`, `https://`, `ws://`, `wss://` URLs (terminated by whitespace, quotes, `<>`, `)`, `]`, `}`) and redacts each.
- Applied to: all status-error URLs, `RedactedHttpError`, `YfClient`'s `Debug` (base URLs redacted; credential state omitted), tracing span/event fields.

implementations:
  - src/core/redaction.rs:RedactedUrl
  - src/core/redaction.rs:RedactedDisplay
  - src/core/redaction.rs:redact_url
  - src/core/redaction.rs:redact_auth_query_params_in_text
  - src/core/redaction.rs:is_sensitive_query_param
  - src/core/redaction.rs:next_url_start
  - src/core/redaction.rs:url_end
  - src/core/error.rs:RedactedHttpError.new
verifies:
  - src/core/redaction.rs:redacted_url_masks_crumb_and_authish_params
  - src/core/redaction.rs:redacted_text_masks_embedded_urls
  - src/core/redaction.rs:redacted_text_masks_crumb_after_comma_separated_query_param
  - src/core/redaction.rs:redacted_text_masks_websocket_urls
  - src/core/redaction.rs:redacted_text_leaves_non_sensitive_urls_alone
  - src/core/client/mod.rs:yf_client_debug_omits_credentials_and_redacts_auth_query_params
  - tests/fundamentals/status_synthetic.rs:quote_summary_http_error_maps_status_and_is_not_cached
  - tests/stream/offline.rs:stream_websocket_status_error_redacts_sensitive_query_params

<!-- id: REQ-CORE-012.A1 -->
- [x] `crumb`, `api_key`, `token`, `client-secret` values are replaced with `REDACTED`; `symbols`, `period1` are preserved.
<!-- id: REQ-CORE-012.A2 -->
- [x] URLs embedded in free text (including after comma-separated params and ws/wss URLs) are redacted; text without sensitive params is unchanged.
<!-- id: REQ-CORE-012.A3 -->
- [x] `format!("{client:?}")` contains no cookie/crumb/key values and no credential-presence fields.
<!-- id: REQ-CORE-012.A4 -->
- [x] A quoteSummary `ServerError.url` contains `crumb=REDACTED` and not the crumb.
<!-- id: REQ-CORE-012.A5 -->
- [ ] URL userinfo (`user:pass@host`, e.g. in proxy URLs) is redacted. [needs review — NOT implemented; see Code review findings]

<!-- id: REQ-CORE-013 -->
## Symbol normalization and endpoint URL construction

- `normalize_symbol(&str) -> Result<String, YfError>` (crate-private): parses with `paft::domain::Symbol::new` (trims + upper-cases), then rejects `.`/`..`, symbols without any ASCII alphanumeric, and any byte outside `[A-Za-z0-9.\-_=^]`; errors are `InvalidParams`.
- `normalize_symbols` maps over many. `YfClient::symbol_url(SymbolEndpoint::{Chart, OptionsV7, QuoteSummary, Timeseries}, symbol)` appends the normalized symbol as one path segment (handling trailing slash) so a symbol can never retarget host/query/fragment.

implementations:
  - src/core/client/urls.rs:normalize_symbol
  - src/core/client/urls.rs:normalize_symbols
  - src/core/client/urls.rs:is_yahoo_symbol_byte
  - src/core/client/urls.rs:append_symbol_path_segment
  - src/core/client/urls.rs:YfClient.symbol_url
  - src/core/client/urls.rs:SymbolEndpoint
verifies:
  - src/core/client/urls.rs:symbol_segment_keeps_supported_punctuation_without_retargeting_url
  - src/core/client/urls.rs:trailing_slash_base_still_gets_one_symbol_separator
  - src/core/client/urls.rs:rejects_dot_segments_that_url_would_drop
  - src/core/client/urls.rs:rejects_empty_whitespace_and_url_syntax_symbols
  - src/core/client/urls.rs:normalize_symbol_trims_and_uppercases_supported_yahoo_symbols
  - tests/core/public_validation.rs:quote_builder_rejects_invalid_symbols_before_request
  - tests/core/public_validation.rs:quote_symbols_are_normalized_before_request

<!-- id: REQ-CORE-013.A1 -->
- [x] `^gspc` → path `/v8/finance/chart/^GSPC` with unchanged scheme/host and no query/fragment.
<!-- id: REQ-CORE-013.A2 -->
- [x] `" brk.b "` → `BRK.B`; `eurusd=x` → `EURUSD=X`; `btc-usd` → `BTC-USD`.
<!-- id: REQ-CORE-013.A3 -->
- [x] `""`, whitespace, `.`, `..`, `AAPL/MSFT`, and URL-shaped input are rejected with `InvalidParams` before any request.

<!-- id: REQ-CORE-014 -->
## Data-quality diagnostics model

Public types (re-exported at crate root):

```rust
#[derive(Default)] pub enum DataQuality { #[default] BestEffort, Strict }
#[non_exhaustive] pub struct YfResponse<T> { pub data: T, pub diagnostics: YfDiagnostics }
  // new, is_lossless, into_data, map
#[non_exhaustive] pub struct YfDiagnostics { pub warnings: Vec<YfWarning> }
  // new, is_lossless, len, is_empty
#[non_exhaustive] pub enum YfWarning { DroppedItem{..}, OmittedPresentField{..}, CoercedPresentField{..},
  ProviderFeatureUnavailable{..}, RepairedData{..}, CurrencyInferred{..}, SuppressedError{..} }
#[non_exhaustive] pub enum ProjectionIssue { MissingRequiredField{field}, MissingRequiredFields{fields},
  InvalidField{field, details}, ConversionFailed{target}, CurrencyUnresolved, InvalidCurrency{code},
  ProviderError{message}, ProviderUnavailable{feature} }
#[non_exhaustive] pub enum YfCurrencyPurpose { Trading, Reporting, CorporateAction, AnalystEstimate }
#[non_exhaustive] pub enum YfCurrencyInference { ListingHeuristic, ProfileCountryHeuristic }
```

- `ProjectionContext` (crate-private) records warnings: `BestEffort` → push and continue; `Strict` → return `YfError::DataQuality(Box<warning>)` on the first issue. `finish(data)` yields `YfResponse`.
- Projection helpers (`optional_projected`, `required_*`, `optional_money_*_with_currency_issue`, `optional_u32_from_raw_f64`, …) omit a bad present field (sibling fields kept) and record why; non-integer counts are rounded with a `CoercedPresentField` warning.
- `YfDiagnostics::with_key_prefix(prefix)` prefixes keys of keyed variants (used by multi-symbol download).
- `Display` impls give stable human strings (e.g. `"analyst-estimate"`, `"profile-country heuristic"`).

implementations:
  - src/core/diagnostics/response.rs:DataQuality
  - src/core/diagnostics/response.rs:YfResponse
  - src/core/diagnostics/response.rs:YfDiagnostics
  - src/core/diagnostics/response.rs:YfDiagnostics.with_key_prefix
  - src/core/diagnostics/warning.rs:YfWarning
  - src/core/diagnostics/warning.rs:YfWarning.with_key_prefix
  - src/core/diagnostics/issue.rs:ProjectionIssue
  - src/core/diagnostics/currency.rs:YfCurrencyPurpose
  - src/core/diagnostics/currency.rs:YfCurrencyInference
  - src/core/diagnostics/context.rs:ProjectionContext
  - src/core/diagnostics/context.rs:ProjectionContext.record
  - src/core/diagnostics/context.rs:ProjectionContext.currency_resolution
  - src/core/diagnostics/context.rs:ProjectionContext.finish
  - src/core/diagnostics/projection.rs:optional_projected
  - src/core/diagnostics/projection.rs:required_nonempty_string
  - src/core/diagnostics/numeric.rs:optional_u32_from_raw_f64
  - src/core/diagnostics/monetary.rs:optional_money_decimal_with_currency_issue
  - src/core/diagnostics/monetary.rs:optional_price_f64_with_currency_issue
verifies:
  - tests/core/public_validation.rs:public_currency_diagnostics_expose_purpose_and_inference
  - tests/fundamentals/status_synthetic.rs:strict_calendar_errors_on_malformed_date
  - tests/fundamentals/status_synthetic.rs:malformed_share_values_do_not_drop_valid_siblings
  - tests/fundamentals/status_synthetic.rs:missing_earnings_module_is_provider_unavailable
  - tests/quotes/offline.rs:malformed_optional_quote_field_is_omitted_from_batch_item
  - src/core/currency_resolver/tests.rs:listing_heuristic_emits_currency_inferred_and_fails_strict

<!-- id: REQ-CORE-014.A1 -->
- [x] Public currency diagnostic enums display as stable kebab/phrase strings.
<!-- id: REQ-CORE-014.A2 -->
- [x] In strict mode, a malformed field produces `YfError::DataQuality`; in best-effort mode the same input yields data plus a warning.
<!-- id: REQ-CORE-014.A3 -->
- [x] A malformed optional field is omitted while valid sibling fields/items are kept.
<!-- id: REQ-CORE-014.A4 -->
- [x] Heuristic currency resolution emits `CurrencyInferred` and fails under strict.
<!-- id: REQ-CORE-014.A5 -->
- [ ] `ProjectionContext::extend` merges sub-diagnostics without re-applying the strict policy. [needs review — no test backs this; relies on sub-calls sharing the caller's policy]

<!-- id: REQ-CORE-015 -->
## Currency resolution pipeline

Crate-private `YfClient::resolve_{trading,reporting,corporate_action,analyst_estimate,analyst_price_target}_currency(symbol, override: Option<Currency>, evidence, &CallOptions) -> Result<ResolvedCurrency, YfError>`.

Order of precedence:
1. Caller override (validated via `decimal_places()`; failure → `InvalidParams`) — never written to the inferred cache.
2. Direct provider field on the payload (invalid code → `InvalidData`); stored in the per-symbol cache for Trading/Reporting/CorporateAction, skipped for analyst purposes.
3. Cached resolution, final if trusted or if relevant hints are known-missing.
4. Hints: Trading-like → quote `currency` → (enrich via v7 quote) → listing heuristic (symbol suffix / exchange) → (enrich via profile) → profile-country heuristic. Reporting-like → `financialCurrency` / quoteSummary `financialCurrency` → (enrich via v7 quote, then quoteSummary `financialData,earnings`) → (enrich via profile) → profile-country.
5. Fall back to a provisional cached value, else first invalid-evidence `InvalidData`, else `MissingData("unable to resolve ... currency for {symbol}")`.

Cache replacement uses rank `ProfileHeuristic < ListingHeuristic < EnrichedProvider < DirectProvider < Override`; a weaker candidate never overwrites a stronger cached value. Failed enrichment calls are not cached; a successful response lacking the field caches "missing". `project_currency_resolution` converts outcomes into `ProjectionIssue`s (`InvalidCurrency`, `CurrencyUnresolved`, `ProviderError`).

implementations:
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_trading_currency
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_reporting_currency
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_corporate_action_currency
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_analyst_estimate_currency
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_analyst_price_target_currency
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_currency_from_evidence
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_trading_currency_from_hints
  - src/core/currency_resolver/resolver.rs:YfClient.resolve_reporting_currency_from_hints
  - src/core/currency_resolver/resolver.rs:YfClient.cached_resolution_is_final
  - src/core/currency_resolver/cache.rs:YfClient.store_resolved_currency
  - src/core/currency_resolver/cache.rs:YfClient.store_currency_hints
  - src/core/currency_resolver/enrichment.rs:YfClient.enrich_quote_hints
  - src/core/currency_resolver/enrichment.rs:YfClient.enrich_quote_summary_reporting_hints
  - src/core/currency_resolver/enrichment.rs:YfClient.enrich_profile_hints
  - src/core/currency_resolver/types.rs:CurrencyResolutionSpec
  - src/core/currency_resolver/types.rs:ResolvedCurrency
  - src/core/currency_resolver/hints.rs:CurrencyHints
  - src/core/currency_resolver/policy.rs:project_currency_resolution
verifies:
  - src/core/currency_resolver/tests.rs:direct_provider_replaces_weaker_profile_cache
  - src/core/currency_resolver/tests.rs:direct_provider_replaces_weaker_enriched_cache
  - src/core/currency_resolver/tests.rs:direct_provider_does_not_replace_stronger_override_cache_entry
  - src/core/currency_resolver/tests.rs:override_resolution_does_not_poison_inferred_cache
  - src/core/currency_resolver/tests.rs:override_currency_without_metadata_is_invalid_params
  - src/core/currency_resolver/tests.rs:provider_hint_replaces_cached_listing_heuristic
  - src/core/currency_resolver/tests.rs:cached_reporting_profile_heuristic_retries_unknown_enrichment
  - src/core/currency_resolver/tests.rs:purpose_and_resolution_mode_use_their_own_cache_semantics
  - src/core/currency_resolver/tests.rs:analyst_direct_currency_does_not_poison_symbol_cache
  - src/core/currency_resolver/tests.rs:invalid_direct_currency_is_invalid_data
  - src/core/currency_resolver/tests.rs:invalid_enriched_currency_is_invalid_data
  - src/core/currency_resolver/tests.rs:invalid_direct_currency_projects_as_omitted_data
  - src/core/currency_resolver/tests.rs:profile_country_heuristic_emits_currency_inferred_and_fails_strict
  - src/core/currency_resolver/tests.rs:failed_quote_enrichment_is_not_cached
  - src/core/currency_resolver/tests.rs:successful_missing_quote_currency_is_cached
  - src/core/currency_resolver/tests.rs:successful_empty_quote_response_caches_requested_symbol_missing
  - src/core/currency_resolver/tests.rs:successful_normalized_quote_response_caches_requested_symbol_key
  - tests/currency/inference_offline.rs:offline_currency_inference_uses_timeseries_currency_code_first
  - tests/currency/inference_offline.rs:offline_currency_inference_falls_back_to_profile_country_when_provider_currency_missing
  - tests/currency/inference_offline.rs:offline_gs2c_dual_listing_currency

<!-- id: REQ-CORE-015.A1 -->
- [x] A direct provider currency replaces a cached profile-heuristic or enriched value, but not a cached override.
<!-- id: REQ-CORE-015.A2 -->
- [x] An override never poisons the inferred cache; an override currency without metadata is `InvalidParams`.
<!-- id: REQ-CORE-015.A3 -->
- [x] A provider hint replaces a cached listing heuristic; a cached reporting profile heuristic re-attempts still-unknown enrichment.
<!-- id: REQ-CORE-015.A4 -->
- [x] Analyst direct currencies do not overwrite the per-symbol cache.
<!-- id: REQ-CORE-015.A5 -->
- [x] Invalid direct/enriched codes surface as `InvalidData` and, in projection, as an omitted field with `InvalidCurrency`.
<!-- id: REQ-CORE-015.A6 -->
- [x] Failed quote enrichment is not cached; a successful response missing the currency (or empty) caches "missing" under the requested symbol key.
<!-- id: REQ-CORE-015.A7 -->
- [x] Offline: a timeseries currency code wins first; with no provider currency, the profile country is used; dual listing GS2C.DE resolves correctly.

<!-- id: REQ-CORE-016 -->
## Currency units and heuristics

- `ResolvedCurrencyUnit::from_code`: `GBp`/`GBX` → GBP ×0.01, `ZAc` → ZAR ×0.01, `ILA` → ILS ×0.01, otherwise ISO parse with scale 1. **Prices** (`price_from_f64`, `price_amount_from_f64`) are scaled to the major unit; **Money** (`money_from_*`) is **not** scaled.
- Listing heuristic (`infer_listing_currency`): symbol suffix table (`.DE/.F/.PA/.MI/.AS/.BR/.MC` → EUR, `.SW` → CHF, `.L` → GBp, `.TO/.V` → CAD, `.T` → JPY, `.HK` → HKD, `.SS/.SZ` → CNY, `.AX` → AUD, `.KS/.KQ` → KRW, `.SI` → SGD), then `yahoo_exchange_to_listing_currency` on `fullExchangeName`/`exchange`.
- Country heuristic (`currency_for_country`): NFKD + diacritic stripping + upper-case normalization, exact table lookup, then word-bounded fuzzy match preferring longer keys.

implementations:
  - src/core/currency_resolver/unit.rs:ResolvedCurrencyUnit
  - src/core/currency_resolver/unit.rs:ResolvedCurrencyUnit.from_code
  - src/core/currency_resolver/unit.rs:ResolvedCurrencyUnit.price_from_f64
  - src/core/currency_resolver/unit.rs:ResolvedCurrencyUnit.money_from_decimal
  - src/core/currency_resolver/inference.rs:infer_listing_currency
  - src/core/currency_resolver/inference.rs:suffix_currency
  - src/core/yahoo_vocab.rs:yahoo_exchange_to_listing_currency
  - src/core/currency.rs:currency_for_country
  - src/core/currency.rs:normalize_country
  - src/core/currency.rs:heuristic_currency_match
verifies:
  - tests/core/conversions.rs:yahoo_unit_currencies_are_scaled_to_major_units
  - tests/core/conversions.rs:yahoo_unit_currencies_do_not_scale_major_money_amounts
  - tests/core/conversions.rs:yahoo_exchange_listing_currency_uses_unparsed_aliases
  - src/core/currency_resolver/tests.rs:listing_inference_uses_yahoo_quote_units
  - src/core/currency.rs:country_currency_rules_are_well_formed
  - src/core/currency.rs:exact_country_lookup_uses_country_currency_table
  - src/core/currency.rs:country_normalization_strips_unicode_diacritics
  - src/core/currency.rs:fuzzy_country_lookup_uses_country_currency_table
  - src/core/currency.rs:fuzzy_country_lookup_prefers_specific_word_bounded_matches

<!-- id: REQ-CORE-016.A1 -->
- [x] Minor-unit codes scale prices to major units; money amounts are left unscaled.
<!-- id: REQ-CORE-016.A2 -->
- [x] Listing inference uses Yahoo quote units (e.g. `.L` → GBp).
<!-- id: REQ-CORE-016.A3 -->
- [x] Country lookup strips diacritics, matches exactly first, then prefers specific word-bounded fuzzy matches.
<!-- id: REQ-CORE-016.A4 -->
- [ ] Why money amounts are treated as already-major while prices are scaled. [needs review — behaviour is tested, rationale (Yahoo reports totals in major units?) is not documented]

<!-- id: REQ-CORE-017 -->
## quoteSummary fetch helper

Crate-private `quotesummary::fetch_body(client, symbol, modules, caller, &CallOptions) -> Result<String, YfError>`: normalizes the symbol, builds `{base_quote_api}/{SYMBOL}?modules=<modules>`, fetches with `AuthMode::RequiredCrumb`, `CacheEndpoint::QuoteSummary`, invalid-crumb-body retry enabled and `validate_quote_summary_body` as cache validator, then validates again. A `quoteSummary.error.description` becomes `YfError::Api("yahoo error: <description>")`. `module_result_raw_value`/`parse_module_result`/`fetch_module_result<T>` borrow-parse and return the last `result[]` entry, or `MissingData("empty quoteSummary result")`. Fixture key (test-mode recording): `"{caller}_api_{modules with ','→'-'}"`.

implementations:
  - src/core/quotesummary.rs:fetch_body
  - src/core/quotesummary.rs:validate_quote_summary_body
  - src/core/quotesummary.rs:reject_borrowed_quote_summary_error
  - src/core/quotesummary.rs:module_result_raw_value
  - src/core/quotesummary.rs:parse_module_result
  - src/core/quotesummary.rs:parse_module_result_raw
  - src/core/quotesummary.rs:fetch_module_result
verifies:
  - tests/auth/crumb_retry_synthetic.rs:api_fetches_cookie_and_crumb_first
  - tests/fundamentals/status_synthetic.rs:quote_summary_api_error_is_not_cached
  - tests/fundamentals/status_synthetic.rs:quote_summary_http_error_maps_status_and_is_not_cached

<!-- id: REQ-CORE-017.A1 -->
- [x] quoteSummary requests always carry a crumb.
<!-- id: REQ-CORE-017.A2 -->
- [x] A quoteSummary body carrying `error` is returned as an error and not cached.
<!-- id: REQ-CORE-017.A3 -->
- [ ] An empty/null `result` yields `MissingData("empty quoteSummary result")`. [needs review — no test backs this]
<!-- id: REQ-CORE-017.A4 -->
- [ ] When several results are returned, the **last** one is used (`Vec::pop`). [needs review — no test backs this; "first" may have been intended]

<!-- id: REQ-CORE-018 -->
## v7 quote batching and side effects (core/quotes.rs)

Crate-private `fetch_v7_quote_values(client, &[&str], &CallOptions) -> Result<Vec<Value>, YfError>` and `fetch_v7_quotes(..) -> Result<Vec<V7QuoteNode>, YfError>`:
- Empty input → `InvalidParams("symbols list cannot be empty")`; symbols normalized.
- Chunks to ≤ 100 symbols and ≤ 1800-byte URLs (a single symbol exceeding the limit → `InvalidParams`); chunks run with concurrency 12 (`buffer_unordered`) and results are re-ordered by chunk index. Any chunk failure fails the whole call.
- Each chunk uses `AuthMode::OptionalCrumb`, `CacheEndpoint::Quote`, `validate_v7_quote_body`. `quoteResponse.error` → `Api`; missing `quoteResponse`/`result` → `MissingData`.
- Requested symbols absent from the response produce `DroppedItem{ item: "quote", reason: ProviderUnavailable }`; undeserializable nodes are dropped with `InvalidField`.
- Side effects: per returned node, currency hints and instrument are stored under the provider symbol and case-variant requested aliases; unresolved requested symbols get "missing" hints.

implementations:
  - src/core/quotes.rs:fetch_v7_quotes
  - src/core/quotes.rs:fetch_v7_quote_values
  - src/core/quotes.rs:chunk_v7_quote_symbols
  - src/core/quotes.rs:fetch_v7_quote_chunk
  - src/core/quotes.rs:validate_v7_quote_body
  - src/core/quotes.rs:report_missing_requested_quote_values
  - src/core/quotes.rs:quote_nodes_from_values_with_context
  - src/core/quotes.rs:store_v7_quote_side_effects
verifies:
  - src/core/quotes.rs:quote_symbol_chunks_respect_max_symbol_count
  - src/core/quotes.rs:quote_symbol_chunks_respect_encoded_url_byte_limit
  - tests/quotes/offline.rs:batch_quotes_chunks_large_symbol_lists_by_count
  - tests/quotes/offline.rs:batch_quotes_with_diagnostics_reports_requested_symbols_missing_from_response
  - tests/quotes/offline.rs:malformed_quote_node_missing_symbol_is_dropped_from_batch
  - tests/quotes/offline.rs:malformed_quote_node_invalid_symbol_is_dropped_from_batch

<!-- id: REQ-CORE-018.A1 -->
- [x] Large symbol lists are split by count and by encoded URL length.
<!-- id: REQ-CORE-018.A2 -->
- [x] Requested symbols missing from the response are reported as diagnostics.
<!-- id: REQ-CORE-018.A3 -->
- [x] Malformed nodes are dropped while valid ones are kept.
<!-- id: REQ-CORE-018.A4 -->
- [ ] Result order matches request chunk order despite unordered concurrency. [needs review — no test backs this]

<!-- id: REQ-CORE-019 -->
## Tolerant wire decoding and conversions

- `WireValue<T>` (= `serde_field_result::Field<T>`), `BufferedWireValue<T>`, `BorrowedWireValue<'de, T>` record an invalid field without failing deserialization of the enclosing struct; `RawNum<T>`, `RawDecimal`, `RawDate`, `RawNumU64` decode Yahoo `{raw, fmt}` objects; `JsonU64`/`JsonDecimal` accept numbers or integral numeric strings.
- `conversions` (public only under `test-mode`): `decimal_from_f64/f32` (non-finite → `None`; f32 not widened through f64), money/price helpers (missing/invalid currency → `None`/error), `i64_to_datetime`/`i64_to_date` (invalid → error), token parsers that reject empty input (`MissingData`) and keep unknown non-empty tokens as extensible `Other` values.
- `yahoo_vocab`: `parse_yahoo_exchange` (Yahoo code/display aliases → `paft::domain::Exchange`, rejects `*_market` scopes), `parse_yahoo_quote_type` (`ETF|MUTUALFUND` → Fund, `INDEX`, `CRYPTOCURRENCY` → Crypto, `CURRENCY` → Forex).
- Test-mode fixture recording: with `YF_RECORD=1`, `net::get_text` writes bodies via `fixtures::record_fixture` (JSON prettified when valid).

implementations:
  - src/core/wire/value.rs:WireField
  - src/core/wire/raw.rs:RawNum
  - src/core/wire/raw.rs:decode_raw_map
  - src/core/wire/number.rs:JsonU64
  - src/core/wire/number.rs:JsonDecimal
  - src/core/conversions.rs:decimal_from_f64
  - src/core/conversions.rs:decimal_from_f32
  - src/core/conversions.rs:i64_to_datetime
  - src/core/conversions.rs:parse_required_token
  - src/core/yahoo_vocab.rs:parse_yahoo_exchange
  - src/core/yahoo_vocab.rs:parse_yahoo_quote_type
  - src/core/net.rs:get_text
  - src/core/fixtures.rs:record_fixture
verifies:
  - src/core/wire.rs:invalid_scalar_is_recorded_without_losing_following_fields
  - src/core/wire.rs:invalid_raw_value_is_recorded_without_losing_following_fields
  - src/core/wire.rs:json_u64_accepts_integral_strings_and_rejects_fractional_strings
  - src/core/wire.rs:buffered_wire_value_keeps_composite_recovery_explicit
  - tests/core/conversions.rs:invalid_float_conversions_return_none
  - tests/core/conversions.rs:f32_decimal_conversion_does_not_widen_binary_float_artifacts
  - tests/core/conversions.rs:valid_zero_remains_zero
  - tests/core/conversions.rs:missing_or_invalid_currency_string_conversions_return_none
  - tests/core/conversions.rs:integer_money_conversions_return_errors_for_missing_currency_metadata
  - tests/core/conversions.rs:invalid_timestamps_return_errors
  - tests/core/conversions.rs:yahoo_exchange_codes_normalize_to_provider_agnostic_exchanges
  - tests/core/conversions.rs:missing_or_invalid_required_tokens_do_not_default_to_plausible_values
  - tests/core/conversions.rs:unknown_nonempty_tokens_are_preserved_as_extensible_other_values
  - tests/quotes/offline.rs:quote_v7_counter_fields_accept_numeric_strings
  - src/core/fixtures.rs:fixture_body_prettifies_valid_json
  - src/core/fixtures.rs:fixture_body_leaves_invalid_json_raw
  - src/core/fixtures.rs:fixture_body_leaves_non_json_raw

<!-- id: REQ-CORE-019.A1 -->
- [x] An invalid scalar or `{raw}` field is recorded as `Invalid` while following fields still decode.
<!-- id: REQ-CORE-019.A2 -->
- [x] `JsonU64` accepts `"42"` and rejects `"42.5"` with a "cannot convert decimal" detail.
<!-- id: REQ-CORE-019.A3 -->
- [x] Non-finite floats → `None`; f32 conversion avoids binary widening artefacts; zero stays zero.
<!-- id: REQ-CORE-019.A4 -->
- [x] Invalid timestamps and empty required tokens are errors; unknown non-empty tokens are preserved as `Other`.
<!-- id: REQ-CORE-019.A5 -->
- [x] Yahoo exchange codes normalize to provider-agnostic exchanges.
<!-- id: REQ-CORE-019.A6 -->
- [x] Recorded fixtures are pretty-printed only when valid JSON.

<!-- id: REQ-CORE-020 -->
## Crate-root public surface and feature flags

`src/lib.rs` declares feature modules and re-exports: `Backoff, CacheEndpoint, CacheMode, DataQuality, HistoryRequest, HistoryService, ProjectionIssue, RetryConfig, YfClient, YfClientBuilder, YfCurrencyInference, YfCurrencyPurpose, YfDiagnostics, YfError, YfResponse, YfWarning`, history/model types (`Action, Candle, HistoryMeta, HistoryResponse, Interval, Range, Quote, …`), feature builders, and selected `paft` types (domain, money, fundamentals, market). `HistoryService` (implemented by `YfClient` in `src/history/mod.rs`) abstracts `fetch_full_history(symbol, HistoryRequest)`.

Features: user-facing `stream`, `dataframe` (re-exports `paft::dataframe::*` as `yfinance_rs::dataframe`), `tracing`; internal `test-mode` (exposes `_preauth`, `conversions`, `yahoo_vocab`, fixture recording), `debug-dumps`, `tracing-subscriber` (`init_tracing_for_tests`, `#[doc(hidden)]`). Crate is `#![warn(missing_docs)]`. Logging macros (`trace_debug!`…`trace_error!`) compile to nothing without `tracing`.

implementations:
  - src/lib.rs:init_tracing_for_tests
  - src/core/models.rs:range_as_str
  - src/core/services.rs:HistoryRequest
  - src/core/services.rs:HistoryService
  - src/core/models.rs:FastInfo
  - src/core/models.rs:MovingAverages
  - src/core/logging.rs:trace_debug
verifies:
  - tests/core/public_validation.rs:crate_root_reexports_common_public_types

<!-- id: REQ-CORE-020.A1 -->
- [x] Common public types (client, errors, diagnostics, retry, `HistoryService`, and the paft model types) are nameable from the crate root.
<!-- id: REQ-CORE-020.A2 -->
- [ ] `test-mode` and `debug-dumps` are not part of the supported public API. [needs review — stated in docs only; Cargo cannot enforce it]

<!-- id: CORE-NONGOALS -->
## Non-goals

- Persistent (on-disk) caching: the response cache is in-memory only (`moka::sync::Cache`). [needs review — inferred from implementation, not stated]
- Managing a general cookie jar: the default reqwest client intentionally has no cookie store; only Yahoo auth cookies are replayed (comment at `src/core/client/mod.rs:991`, test `builder_default_http_client_does_not_store_ambient_cookies`).
- Supported public API under internal features: `test-mode`, `debug-dumps`, `tracing-subscriber` are explicitly "not part of the supported user-facing API" (`src/lib.rs` crate docs).
- Applying builder timeouts/proxy to a user-supplied `custom_client` (explicitly documented as ignored).
- Anything else: [fill in — not derivable from code]

<!-- id: CORE-OPEN-QUESTIONS -->
## Open questions

1. Is unconditional crumb clearing on a stale-crumb response acceptable under high concurrency, or should refresh be compare-and-swap on the crumb value that failed?
2. Should `Retry-After` on 429/503 be honoured?
3. Should the cache validator also gate fresh bodies when caching is disabled (current behaviour differs by cache configuration)?
4. Should `cache_ttl_for(Quote|Options|News|Screener)` implicitly opt those endpoints into caching?
5. Why does cookie acquisition ignore the HTTP status of `cookie_url`? (Presumably because `fc.yahoo.com` returns non-2xx with a valid `Set-Cookie`.) [needs review]
6. quoteSummary uses the last `result[]` entry — intended?
7. Is FIFO (not LRU) eviction for side caches intended?
8. Should `YfError::Http` expose `is_timeout()` / `is_connect()` or a source, so callers can classify transport failures?
9. Should money amounts in minor-unit currencies (GBp/ZAc/ILA) ever be scaled?

<!-- id: CORE-REVIEW-FINDINGS -->
## Code review findings

Ranked by severity (High → Low).

1. **High — Vacuous, network-dependent tests run by default.** `tests/currency/verification.rs:212-230` (`test_currency_verification`) and `:232` (`test_currency_precision`) have no `#[ignore]` and no `live_or_record_enabled()` guard, so they hit live Yahoo on every `cargo test`, yet the `check_*` helpers only `println!` "❌ INCORRECT"/"❌ Error" (e.g. `:37`) and never assert. They cannot fail on wrong currencies and make CI depend on the network.
2. **Medium — Crumb-refresh stampede under concurrency.** `src/core/net.rs:422-424` (`retry_with_fresh_crumb`) calls `client.clear_crumb()` unconditionally. If N tasks receive a stale-crumb response, each clears the shared crumb — including a fresh crumb another task just fetched — so refreshes serialize behind `credential_fetch_lock` (`src/core/client/auth.rs:16`) and repeat up to N times. A compare-and-clear on the failed crumb value would avoid this.
3. **Medium — Cache validation depends on cache configuration.** `src/core/net.rs:151-153` and `:378-380` run `cache_validator` on fresh bodies only when cache writes are enabled; with caching off, an invalid body passes through `fetch_text_cached` untouched, whereas the POST path validates unconditionally (`:195`). Same input, different error behaviour depending on `cache_ttl`. (quoteSummary and v7 quote re-validate downstream, which hides it today: `src/core/quotesummary.rs:90`, `src/core/quotes.rs:1509-1516`.)
4. **Medium — Full expired-entry scan on every cache write.** `src/core/client/mod.rs:464` calls `prune_expired`, which runs `invalidate_entries_if` plus `run_pending_tasks` (`:234-239`) on every `cache_put_key`; `insert` also calls `run_pending_tasks` (`:213`). That is O(n) work per write on a cache of up to `cache_max_entries`. Moka's native per-entry expiry (`Expiry`) would remove the custom `expires_at` bookkeeping.
5. **Medium — Redaction misses URL userinfo.** `src/core/redaction.rs:56-83` only rewrites query parameters; `http://user:pass@proxy:8080` keeps the password. Proxy URLs set with `try_proxy` (`src/core/client/mod.rs:922`) could leak through `RedactedHttpError` text if reqwest includes them in error messages. [needs review — whether reqwest includes proxy URLs in error Display is unverified]
6. **Medium — `YfError::Http` drops its source.** `src/core/error.rs:342-345` stores only the redacted string, and `source()` (`:328-339`) returns `None` for `Http`. Callers cannot tell timeout from connect from TLS errors, though the retry layer itself makes that distinction (`src/core/client/mod.rs:574-575`).
7. **Low — No `Retry-After` handling.** `src/core/client/mod.rs:549-570` uses the fixed backoff for 429/503 regardless of server hints, which makes rate-limit lockouts more likely with the default policy (4 retries, 3 s cap).
8. **Low — Error-string parsing for control flow.** `src/core/currency_resolver/policy.rs:88-99` recovers an `InvalidCurrency` code by matching `" currency code "` and `rsplit_once(": ")` on an `InvalidData` message produced in `resolver.rs:444-447`/`:469-473`. Rewording either message silently breaks diagnostics; a typed error would be safer.
9. **Low — Proxy setters share one slot.** `src/core/client/mod.rs:929` and `:958` both assign `self.proxy`, so `try_proxy(..).try_https_proxy(..)` silently discards the first. Separate HTTP and HTTPS proxies cannot be configured without `custom_client`.
10. **Low — Jitter doc mismatch.** `src/core/client/retry.rs:19` says "+/- 50%", but `jitter_duration` (`src/core/client/mod.rs:1117-1121`) caps the upper bound at `max`, so once the delay saturates, jitter only shortens it.
11. **Low — `YfClient::default()` panics** on HTTP-client build failure (`src/core/client/mod.rs:376-380`), for example when the TLS backend fails to initialise.
12. **Low — Redundant parsing.** A quoteSummary body is parsed up to 3 times: the cache validator (`src/core/quotesummary.rs:76`), the post-fetch validation (`:90`), and `parse_module_result` (`:149`). `chunk_v7_quote_symbols` clones the current chunk and rebuilds the URL for every symbol (`src/core/quotes.rs:1457-1462`), which is O(n²) per chunk.
13. **Low — False-positive stale-crumb detection.** `src/core/net.rs:513-524` treats any body ≤ 200 bytes containing "unauthorized" as an invalid crumb, which forces a credential refresh and one retry.
14. **Low — Side-map robustness.** `BoundedSideMap` evicts in FIFO order (`src/core/client/mod.rs:62-77`) and panics on lock poisoning (`.expect("side cache lock poisoned")`, `:99`, `:106`, `:113`). `CurrencyCacheKey::new` stores the symbol un-normalized (`src/core/currency_resolver/types.rs:331-335`). [needs review — callers appear to pass normalized symbols]
15. **Low — Missing tests:** `clear_cache`, `invalidate_cache_entry` (GET+POST), `try_https_proxy`, invalid-proxy errors, `user_agent` propagation, `RequestNotCloneable`, generic `Status` mapping, single-flight credential fetch under concurrency, the OptionalCrumb bare→401→crumb path, the empty quoteSummary result, and chunk re-ordering in `fetch_v7_quote_values`.

<!-- id: CORE-RE-NOTES -->
## Reverse-engineering notes

- Generated 2026-10-21 from the files listed in frontmatter (v0.9.1, branch `main`, commit `bad87d5`). No source files were modified; no `§ref:specs` comments were added.
- Confidence:
  - HIGH: retry/backoff semantics, retry validation, cookie/crumb flow and stale-crumb recovery, cache modes/TTL/bounds, status mapping, redaction rules, symbol normalization. Each has direct unit or integration tests.
  - HIGH: currency resolver precedence and cache ranking, from 20 unit tests in `src/core/currency_resolver/tests.rs`.
  - MEDIUM: diagnostics model semantics. Exercised mainly through feature-module tests (fundamentals, quotes), not core-local tests.
  - MEDIUM: v7 quote batching. It lives in `src/core/quotes.rs` but is functionally part of the quote feature; it may belong in a QUOTE spec.
  - LOW: intent/business rationale (why money vs price scaling differs, why the cookie status is ignored, why the last quoteSummary result is used).
- `verifies:` entries include in-crate unit tests (`src/**/*.rs:<test fn>`) as well as `tests/**` integration tests. `tests/auth/live.rs:live_cached_crumb_optional_endpoints_smoke` and `tests/currency/inference_live.rs:*` are `#[ignore]`d live tests and are intentionally not listed as verifiers. `tests/currency/verification.rs` tests are not listed because they assert nothing (finding 1).
- Examples 11, 12, 13 and 16 only demonstrate builder, retry, cache, proxy and diagnostics usage. They add no behaviour beyond what is specified here.
