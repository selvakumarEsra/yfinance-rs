---
id: STRM-DOC
title: Real-time quote streaming (WebSocket + polling fallback)
owner: unassigned
priority: medium
version: 1
status: draft-reverse-engineered
created: 2026-10-21
reverse_engineered_from:
  - src/stream/mod.rs
  - src/stream/yaticker.proto
  - src/stream/yaticker.rs
  - src/lib.rs
  - src/core/client/mod.rs
  - src/core/client/constants.rs
  - src/core/client/urls.rs
  - src/core/client/retry.rs
  - src/core/call_options.rs
  - src/core/conversions.rs
  - src/core/quotes.rs
  - Cargo.toml
  - tests/stream.rs
  - tests/stream/offline.rs
  - tests/stream/websocket_decoder.rs
  - tests/stream/live.rs
  - tests/core/public_validation.rs
  - tests/fixtures/stream_ws_MULTI.b64
  - examples/01_basic_usage.rs
  - examples/06_realtime_polling.rs
  - examples/16_diagnostics_audit.rs
---

<!-- id: STRM-DOC -->
# Real-time quote streaming (reverse-engineered)

Intent: behind the opt-in Cargo feature `stream`, the crate lets a caller subscribe to live Yahoo Finance quotes for a set of symbols and receive them as `paft::market::quote::QuoteUpdate` values on a bounded tokio `mpsc` channel. Three transports exist: Yahoo's WebSocket streamer (`wss://streamer.finance.yahoo.com/?version=2`, protobuf `Yaticker.PricingData` frames), HTTP polling of the v7 quote endpoint, and the default hybrid that uses the WebSocket and polls between reconnect attempts. Updates carry the latest cumulative day volume as sent by Yahoo; the crate deliberately does not derive per-tick deltas (code comment at src/stream/mod.rs:58-63). The public surface is re-exported from the crate root as `StreamBuilder`, `StreamConfig`, `StreamHandle`, `StreamMethod` (src/lib.rs:203).

<!-- id: REQ-STRM-001 -->
## Builder configuration, defaults and pre-start validation

`StreamBuilder` collects configuration and validates it before any task is spawned or network I/O occurs.

Public API:
- `StreamBuilder::new(client: &YfClient) -> Self` (clones the client)
- `symbols<I, S>(self, I) -> Self where I: IntoIterator<Item = S>, S: Into<String>` (replaces the list); `add_symbol(self, impl Into<String>) -> Self` (appends)
- `method(self, StreamMethod) -> Self`; `interval(self, Duration) -> Self`; `diff_only(self, bool) -> Self`
- `websocket_idle_timeout(self, Duration) -> Self`; `websocket_connect_timeout(self, Duration) -> Self`
- `cache_mode(self, CacheMode) -> Self`; `retry_policy(self, Option<RetryConfig>) -> Self` (documented as affecting polling only)
- `pub struct StreamConfig { pub interval: Duration, pub diff_only: bool }`

Defaults: `interval = 1s`, `diff_only = true`, `method = StreamMethod::WebsocketWithFallback`, connect timeout 30s, idle timeout 90s, `CallOptions::default()` (`CacheMode::Default`, which bypasses the cache for the quote endpoint).

Validation in `start()` (returns `YfError::InvalidParams`): empty symbol list; zero interval; zero idle timeout; zero connect timeout; any symbol failing `normalize_symbols` (Yahoo symbol charset, at least one ASCII alphanumeric, not `.`/`..`). Symbols are normalized (e.g. upper-cased/trimmed by `paft::domain::Symbol`) before subscription. [needs review — normalization behaviour is inferred from `normalize_symbol`, no stream-specific test]. Duplicate symbols are not removed. [needs review — no dedup in code; unclear whether intended]

implementations:
  - src/stream/mod.rs:StreamConfig
  - src/stream/mod.rs:StreamConfig.default
  - src/stream/mod.rs:StreamBuilder.new
  - src/stream/mod.rs:StreamBuilder.symbols
  - src/stream/mod.rs:StreamBuilder.add_symbol
  - src/stream/mod.rs:StreamBuilder.method
  - src/stream/mod.rs:StreamBuilder.interval
  - src/stream/mod.rs:StreamBuilder.diff_only
  - src/stream/mod.rs:StreamBuilder.websocket_idle_timeout
  - src/stream/mod.rs:StreamBuilder.websocket_connect_timeout
  - src/stream/mod.rs:StreamBuilder.cache_mode
  - src/stream/mod.rs:StreamBuilder.retry_policy
  - src/stream/mod.rs:StreamBuilder.validated_symbols
  - src/core/client/urls.rs:normalize_symbols

verifies:
  - tests/core/public_validation.rs:stream_builder_rejects_zero_interval_before_starting
  - tests/core/public_validation.rs:stream_builder_rejects_zero_websocket_idle_timeout_before_starting
  - tests/core/public_validation.rs:stream_builder_rejects_zero_websocket_connect_timeout_before_starting

<!-- id: REQ-STRM-001.A1 -->
- [x] A zero `interval` makes `start()` fail with `InvalidParams` mentioning "interval" before startup.
<!-- id: REQ-STRM-001.A2 -->
- [x] A zero `websocket_idle_timeout` makes `start()` fail with `InvalidParams` mentioning "websocket idle timeout".
<!-- id: REQ-STRM-001.A3 -->
- [x] A zero `websocket_connect_timeout` makes `start()` fail with `InvalidParams` mentioning "websocket connect timeout".
<!-- id: REQ-STRM-001.A4 -->
- [ ] An empty symbol list makes `start()` fail with `InvalidParams("symbols list cannot be empty")`. [needs review — no test backs this]
<!-- id: REQ-STRM-001.A5 -->
- [ ] An invalid symbol (unsupported characters) makes `start()` fail with `InvalidParams` before spawning. [needs review — no test backs this]
<!-- id: REQ-STRM-001.A6 -->
- [ ] `StreamConfig::default()` is `{ interval: 1s, diff_only: true }` and `StreamMethod::default()` is `WebsocketWithFallback`. [needs review — no test backs this]

<!-- id: REQ-STRM-002 -->
## Transport selection and startup contract

`StreamBuilder::start(&self) -> Result<(StreamHandle, tokio::sync::mpsc::Receiver<QuoteUpdate>), YfError>` spawns one tokio task and returns a receiver on a channel of fixed capacity 1024.

`pub enum StreamMethod { #[default] WebsocketWithFallback, Websocket, Polling }`

- `Websocket`: `start()` awaits a oneshot startup report; WebSocket handshake, subscription-write and connect-timeout errors are returned from `start()`. If the task ends before reporting, `start()` returns `YfError::InvalidData("websocket stream task ended before reporting startup")`. Runtime failures after startup only log (`trace_warn!`) and close the receiver.
- `WebsocketWithFallback` and `Polling`: `start()` returns as soon as the task is spawned; all connection/poll failures are handled inside the task and never surfaced to the caller.

implementations:
  - src/stream/mod.rs:StreamMethod
  - src/stream/mod.rs:StreamBuilder.start
  - src/stream/mod.rs:report_websocket_startup_error

verifies:
  - tests/stream/offline.rs:stream_websocket_reports_initial_connection_failure
  - tests/stream/offline.rs:stream_websocket_maps_numeric_quote_type_without_cached_instrument
  - tests/stream/offline.rs:stream_websocket_fallback_to_polling_offline
  - tests/stream/offline.rs:stream_polling_explicitly_offline

<!-- id: REQ-STRM-002.A1 -->
- [x] With `Websocket` and an unresolvable stream host, `start()` returns `Err(YfError::Websocket(_))`.
<!-- id: REQ-STRM-002.A2 -->
- [x] With `Websocket` and a reachable server, `start()` returns `Ok` and updates arrive on the receiver.
<!-- id: REQ-STRM-002.A3 -->
- [x] With `WebsocketWithFallback` and an unresolvable stream host, `start()` returns `Ok` and polling-sourced updates are delivered.
<!-- id: REQ-STRM-002.A4 -->
- [x] With `Polling`, `start()` returns `Ok` and an update is delivered from the v7 quote endpoint.
<!-- id: REQ-STRM-002.A5 -->
- [ ] With `Websocket`, a runtime failure after a successful start closes the receiver (`recv()` yields `None`) without delivering an error value. [needs review — no test backs this]

<!-- id: REQ-STRM-003 -->
## WebSocket connection handshake and subscription

The WebSocket is opened by issuing an HTTP/1.1 GET upgrade through the client's own `reqwest` client (so proxies and custom clients apply), converting `wss`→`https` / `ws`→`http` on the configured `YfClientBuilder::base_stream(Url)` (default `wss://streamer.finance.yahoo.com/?version=2`). Headers sent: `Origin: https://finance.yahoo.com`, the client user agent, `Upgrade: websocket`, `Connection: Upgrade`, a generated `Sec-WebSocket-Key`, `Sec-WebSocket-Version: 13`.

- Non-101 status → `YfError::Status { status, url }` with the URL redacted (`RedactedUrl`).
- Missing/mismatched `Sec-WebSocket-Accept` → `YfError::Websocket(Protocol(SecWebSocketAcceptKeyMismatch))`.
- Transport errors are wrapped as `YfError::Websocket(Io(..))` using `RedactedHttpError`.
- Unsupported scheme → `YfError::InvalidParams("unsupported websocket URL scheme: ..")`.
- After upgrade, a single text frame `{"subscribe":["SYM1","SYM2",...]}` is sent.
- The whole connect + subscribe phase is bounded by `websocket_connect_timeout`; on expiry the error is `YfError::Websocket(Io(TimedOut, "websocket startup did not complete within .."))`.

implementations:
  - src/stream/mod.rs:connect_websocket_stream
  - src/stream/mod.rs:websocket_http_upgrade_url
  - src/stream/mod.rs:websocket_http_transport_error
  - src/stream/mod.rs:websocket_connect_timeout_error
  - src/stream/mod.rs:run_websocket_stream
  - src/stream/mod.rs:WsSubscribe

verifies:
  - tests/stream/offline.rs:stream_websocket_status_error_redacts_sensitive_query_params
  - tests/stream/offline.rs:stream_websocket_startup_respects_connect_timeout
  - tests/stream/offline.rs:stream_websocket_maps_numeric_quote_type_without_cached_instrument
  - tests/core/public_validation.rs:general_proxy_routes_websocket_startup_through_proxy
  - tests/core/public_validation.rs:custom_client_proxy_routes_websocket_startup_through_proxy

<!-- id: REQ-STRM-003.A1 -->
- [x] A non-upgrade HTTP 200 response yields `YfError::Status`, and its message redacts `crumb` and `token` query values (`crumb=REDACTED`, `token=REDACTED`).
<!-- id: REQ-STRM-003.A2 -->
- [x] A server that stalls the upgrade beyond a 50 ms connect timeout makes `start()` fail within 1 s with a `YfError::Websocket` whose message contains "startup".
<!-- id: REQ-STRM-003.A3 -->
- [x] The first frame sent after upgrade is a text frame whose JSON `subscribe` field equals the requested symbol list.
<!-- id: REQ-STRM-003.A4 -->
- [x] With a general proxy (`try_proxy`) or a `custom_client` carrying a proxy, the WebSocket startup is sent to the proxy as an HTTP `CONNECT` to the target authority and does not connect directly.
<!-- id: REQ-STRM-003.A5 -->
- [ ] A 101 response with a wrong `Sec-WebSocket-Accept` header is rejected with a protocol error. [needs review — no test backs this]

<!-- id: REQ-STRM-004 -->
## WebSocket frame decoding

Each inbound frame is decoded into the protobuf `Yaticker.PricingData` message (src/stream/yaticker.proto, 33 fields; generated code in src/stream/yaticker.rs).

- Text frame: trimmed; if it starts with `{` and parses as JSON, the string field `message` is taken as base64 (a JSON object without `message` is an error → ignored); otherwise the whole text is treated as standard base64. The base64 bytes are protobuf-decoded.
- Binary frame: first tried as UTF-8 text through the same path; if that fails, decoded as raw protobuf bytes.
- Decode failures are logged at debug level and the frame is dropped (comment: acks/heartbeats may lack `message`).
- Ping/Pong/raw Frame messages are ignored by the loop; tungstenite queues pong replies automatically and the loop keeps polling so they flush.
- Test-only (`cfg(any(test, feature = "test-mode"))`, `#[doc(hidden)]`): `pub fn decode_and_map_message(text: &str) -> Result<QuoteUpdate, YfError>` decodes one base64 message and maps it (without the client instrument cache), returning errors rather than skipping. With `test-mode` and `YF_RECORD=1`, the first text frame is recorded as fixture `stream_ws_MULTI.b64`.

implementations:
  - src/stream/mod.rs:decode_ws_pricing
  - src/stream/mod.rs:run_websocket_stream
  - src/stream/mod.rs:decode_and_map_message
  - src/stream/yaticker.rs:PricingData

verifies:
  - tests/stream/websocket_decoder.rs:decode_real_websocket_message
  - tests/stream/offline.rs:stream_websocket_maps_numeric_quote_type_without_cached_instrument
  - tests/stream/offline.rs:stream_websocket_replies_to_ping

<!-- id: REQ-STRM-004.A1 -->
- [x] The recorded live fixture `stream_ws_MULTI.b64` decodes to an update with a non-empty symbol, a positive price and a present volume.
<!-- id: REQ-STRM-004.A2 -->
- [x] A JSON text frame `{"message": "<base64 protobuf>"}` is decoded and emitted as an update.
<!-- id: REQ-STRM-004.A3 -->
- [x] A server Ping with payload P receives a Pong with the same payload P.
<!-- id: REQ-STRM-004.A4 -->
- [ ] A raw protobuf binary frame is decoded and emitted. [needs review — no test backs this]
<!-- id: REQ-STRM-004.A5 -->
- [ ] An undecodable text or binary frame is dropped without terminating the stream. [needs review — no test backs this]

<!-- id: REQ-STRM-005 -->
## Mapping a WebSocket PricingData message to QuoteUpdate

`QuoteUpdate { instrument, currency, price, previous_close, volume, ts, provider: () }` is built from `PricingData` as follows:

- `instrument`: resolved via REQ-STRM-007 using `id` and `quote_type` mapped by `stream_quote_type_to_asset_kind`: 8→Equity, 9→Index, 11/12/20→Fund, 13→Option, 14→Forex, 15→Warrant, 17→Bond, 18→Future, 23→Commodity, 41→Crypto, anything else→`None` (→ untyped fallback).
- `currency`: `currency` field if non-blank, else the listing currency inferred from `exchange` via `yahoo_exchange_to_listing_currency`; if neither resolves, the message is skipped (debug log). Minor-unit codes such as `GBp` are supported through `ResolvedCurrencyUnit`.
- `price` / `previous_close`: `None` when the f32 is non-finite or `<= 0` (so protobuf default `0` is not projected); otherwise converted to `Decimal`, rounded to `price_hint` decimal places (capped at 28) when `price_hint >= 0`, then minor-unit scaled.
- `volume`: `day_volume` (cumulative) via `quantity_from_i64`; negative → `None`. No delta/session logic.
- `ts`: `time` interpreted as epoch milliseconds; an out-of-range value skips the message (debug log).

implementations:
  - src/stream/mod.rs:map_ws_pricing_to_update
  - src/stream/mod.rs:ws_pricing_to_update
  - src/stream/mod.rs:stream_quote_type_to_asset_kind
  - src/stream/mod.rs:ws_currency_unit
  - src/stream/mod.rs:nonempty_str
  - src/stream/mod.rs:ws_price_from_f32
  - src/stream/mod.rs:ws_pricing_timestamp

verifies:
  - src/stream/mod.rs:stream_quote_type_to_asset_kind_maps_known_yahoo_codes
  - src/stream/mod.rs:stream_quote_type_to_asset_kind_rejects_non_instrument_codes
  - src/stream/mod.rs:websocket_price_uses_price_hint_to_trim_f32_noise
  - src/stream/mod.rs:websocket_price_hint_rounds_before_minor_unit_scaling
  - tests/stream/websocket_decoder.rs:decode_equity_websocket_message_infers_currency_from_exchange
  - tests/stream/websocket_decoder.rs:decode_unknown_websocket_quote_type_uses_untyped_fallback
  - tests/stream/offline.rs:stream_websocket_maps_numeric_quote_type_without_cached_instrument

<!-- id: REQ-STRM-005.A1 -->
- [x] Each known Yahoo quote-type code maps to the listed `AssetKind`; codes 0, 7, 1000 and `i32::MAX` map to `None`.
<!-- id: REQ-STRM-005.A2 -->
- [x] An f32 price carrying float noise (bits `0x3f8be76d`) with `price_hint = 4` yields exactly `1.0930` USD.
<!-- id: REQ-STRM-005.A3 -->
- [x] For a `GBp` unit, `123.45` with `price_hint = 2` yields `1.2345` (rounding happens before minor-unit scaling).
<!-- id: REQ-STRM-005.A4 -->
- [x] An empty `currency` with exchange `NMS` resolves to `USD`; price `314.6` renders as `"314.6"`; `previous_close = 0` maps to `None`; `day_volume = 26248990` is exposed as volume `"26248990"`.
<!-- id: REQ-STRM-005.A5 -->
- [x] An unknown quote type (1001) produces instrument kind `YAHOO_STREAM_UNTYPED`, and `previous_close = 313.0` renders as `"313"`.
<!-- id: REQ-STRM-005.A6 -->
- [x] Over a live socket, `quote_type = 8` with no cached instrument yields `AssetKind::Equity`.
<!-- id: REQ-STRM-005.A7 -->
- [ ] A message with no resolvable currency, or an out-of-range timestamp, is skipped in the live stream rather than terminating it. [needs review — no test backs this; only the test-only `decode_and_map_message` error path is exercised indirectly]

<!-- id: REQ-STRM-006 -->
## Cumulative volume semantics

`QuoteUpdate::volume` is the latest cumulative day volume as reported by Yahoo — `day_volume` on the WebSocket path, `regularMarketVolume` on the polling path. The crate does not infer per-update deltas, session boundaries, resets, or provider-side adjustments; callers derive those themselves (src/stream/mod.rs:58-63).

implementations:
  - src/stream/mod.rs:map_ws_pricing_to_update
  - src/stream/mod.rs:handle_polling_quotes
  - src/core/conversions.rs:quantity_from_i64
  - src/core/conversions.rs:quantity_from_u64

verifies:
  - tests/stream/websocket_decoder.rs:decode_real_websocket_message
  - tests/stream/websocket_decoder.rs:decode_equity_websocket_message_infers_currency_from_exchange
  - tests/stream/offline.rs:stream_polling_omits_malformed_optional_price_and_keeps_siblings
  - tests/stream/offline.rs:stream_polling_diff_only_ignores_volume_only_change

<!-- id: REQ-STRM-006.A1 -->
- [x] A WebSocket message's `day_volume` appears unchanged as `QuoteUpdate::volume`.
<!-- id: REQ-STRM-006.A2 -->
- [x] A polling quote's `regularMarketVolume` (numeric or numeric string, e.g. `"1000"`) appears unchanged as `QuoteUpdate::volume`.
<!-- id: REQ-STRM-006.A3 -->
- [ ] A WebSocket message that omits `day_volume` (protobuf default 0) reports volume `Some(0)` rather than `None`. [needs review — no test backs this; inconsistent with the `previous_close` treatment of 0, see review finding F5]

<!-- id: REQ-STRM-007 -->
## Instrument resolution and type caching

Both transports resolve `Instrument` through `resolve_stream_instrument(client, symbol, kind)`:

1. If the client's instrument cache has the symbol, that cached instrument is used (even if the current message carries a different type).
2. Otherwise build `Instrument::from_symbol(symbol, kind.unwrap_or(AssetKind::Other("YAHOO_STREAM_UNTYPED")))`; an invalid symbol (e.g. empty) skips the update.
3. Only typed instruments (a known `kind`) are written to the cache; the untyped fallback is never cached, so later typed data can replace it.

Polling maps the `quoteType` string with `parse_yahoo_quote_type`; WebSocket maps the numeric code (REQ-STRM-005).

implementations:
  - src/stream/mod.rs:resolve_stream_instrument
  - src/stream/mod.rs:untyped_stream_asset_kind

verifies:
  - tests/stream/offline.rs:stream_polling_does_not_cache_untyped_instrument_fallback
  - tests/stream/offline.rs:stream_websocket_maps_numeric_quote_type_without_cached_instrument

<!-- id: REQ-STRM-007.A1 -->
- [x] A polled quote without `quoteType` emits kind `YAHOO_STREAM_UNTYPED`; a later poll with `quoteType: "EQUITY"` emits `AssetKind::Equity` for the same symbol.
<!-- id: REQ-STRM-007.A2 -->
- [ ] Once a typed instrument is cached, subsequent updates reuse the cached instrument regardless of the message's type field. [needs review — no test backs this]

<!-- id: REQ-STRM-008 -->
## WebSocket liveness: idle timeout and remote close

After startup the read loop treats the socket as dead and returns an error when:
- no frame of any kind (including Ping) arrives within `websocket_idle_timeout` (default 90s; the timer restarts after every frame) → `YfError::Websocket(Io(TimedOut, "websocket stream received no frames for .."))`;
- the server sends a Close frame (pending writes are flushed first) or the stream ends → `YfError::Websocket(ConnectionClosed)`, unless a stop was already requested, in which case it ends cleanly;
- any tungstenite read error → `YfError::Websocket(e)`.

In `Websocket` mode these end the stream; in `WebsocketWithFallback` they trigger REQ-STRM-009.

implementations:
  - src/stream/mod.rs:run_websocket_stream
  - src/stream/mod.rs:websocket_idle_timeout_error
  - src/stream/mod.rs:websocket_remote_closed_error
  - src/stream/mod.rs:websocket_stop_requested

verifies:
  - tests/stream/offline.rs:stream_websocket_fallback_to_polling_after_idle_timeout
  - tests/stream/offline.rs:stream_websocket_fallback_to_polling_after_remote_close

<!-- id: REQ-STRM-008.A1 -->
- [x] With an 80 ms idle timeout and a server that goes silent after the subscription, the fallback stream polls the v7 quote endpoint and emits an AAPL update.
<!-- id: REQ-STRM-008.A2 -->
- [x] A server Close frame right after the subscription makes the fallback stream poll and emit an AAPL update.
<!-- id: REQ-STRM-008.A3 -->
- [ ] In `Websocket` mode, an idle timeout or remote close closes the receiver. [needs review — no test backs this]

<!-- id: REQ-STRM-009 -->
## Fallback: polling between WebSocket reconnect attempts

`StreamMethod::WebsocketWithFallback` loops: run the WebSocket session (no startup report); on error, increment a failure counter, then poll the v7 quote endpoint on the configured `interval` until a reconnect deadline, then try the WebSocket again. It ends when the session ends cleanly (stop), the receiver is dropped, or a stop is requested.

Reconnect delay: `websocket_reconnect_backoff(interval, failures) = min(max(interval, 1ms) * 2^(min(failures-1, 16)), 30s)`. The failure counter is never reset after a successful reconnect. [needs review — may be unintended; see finding F2]

Polling during fallback reuses REQ-STRM-010 / REQ-STRM-011 logic, including `diff_only` filtering, with a `last_price` map that persists across reconnects. The same `tokio::time::Interval` is reused across all fallback windows.

implementations:
  - src/stream/mod.rs:run_websocket_stream_with_fallback
  - src/stream/mod.rs:websocket_reconnect_backoff
  - src/stream/mod.rs:PollReconnectContext
  - src/stream/mod.rs:poll_until_websocket_reconnect

verifies:
  - src/stream/mod.rs:websocket_reconnect_backoff_grows_and_caps
  - tests/stream/offline.rs:stream_websocket_fallback_to_polling_offline
  - tests/stream/offline.rs:stream_websocket_fallback_reconnects_after_polling
  - tests/stream/offline.rs:stream_websocket_fallback_to_polling_after_idle_timeout
  - tests/stream/offline.rs:stream_websocket_fallback_to_polling_after_remote_close

<!-- id: REQ-STRM-009.A1 -->
- [x] Backoff for a 40 ms interval is 40 ms after 1 failure, 80 ms after 2, and capped at 30 s for `u32::MAX` failures.
<!-- id: REQ-STRM-009.A2 -->
- [x] When the WebSocket host is unreachable, the stream polls quotes and emits an update with a positive price.
<!-- id: REQ-STRM-009.A3 -->
- [x] After a first session is closed by the server, the stream polls at least once, then reconnects, re-sends the subscription, and emits the WebSocket-sourced price (314.6).
<!-- id: REQ-STRM-009.A4 -->
- [ ] The backoff counter resets after a healthy reconnect. [needs review — no test backs this; code does NOT reset it]

<!-- id: REQ-STRM-010 -->
## HTTP polling stream

`StreamMethod::Polling` (and fallback windows) call `crate::core::quotes::fetch_v7_quotes(client, symbols, options)` once per `interval` tick (the first tick fires immediately), honouring the builder's `cache_mode` and `retry_policy`. For each returned quote node:

- `ts` = `regularMarketTime` (epoch seconds) if present, else `Utc::now()`.
- price = `regularMarketPrice`, falling back to `regularMarketPreviousClose` when the price is absent or malformed. [needs review — falling back to previous close as "price" may surprise callers; purpose unclear]
- `previous_close` = `regularMarketPreviousClose`; `volume` = `regularMarketVolume`.
- Missing/unknown `currency` → the node is skipped (debug log) and polling continues.
- Malformed optional fields on one symbol do not drop sibling symbols.
- A fetch error is logged at debug level and swallowed; polling continues while the receiver is open.

implementations:
  - src/stream/mod.rs:run_polling_stream
  - src/stream/mod.rs:wait_for_poll_tick
  - src/stream/mod.rs:poll_stream_once
  - src/stream/mod.rs:handle_polling_quotes
  - src/core/quotes.rs:fetch_v7_quotes

verifies:
  - tests/stream/offline.rs:stream_polling_explicitly_offline
  - tests/stream/offline.rs:stream_polling_omits_malformed_optional_price_and_keeps_siblings
  - tests/stream/offline.rs:stream_polling_uses_regular_market_time_as_update_timestamp
  - tests/stream/offline.rs:stream_polling_diff_only_does_not_track_skipped_updates

<!-- id: REQ-STRM-010.A1 -->
- [x] A polling stream for MSFT hits `/v7/finance/quote?symbols=MSFT` and delivers an update within 3 s.
<!-- id: REQ-STRM-010.A2 -->
- [x] Given AAPL with a non-numeric `regularMarketPrice` and MSFT valid, the stream emits AAPL with `price = None` followed by MSFT with volume `"1000"`.
<!-- id: REQ-STRM-010.A3 -->
- [x] `regularMarketTime = 1761768001` becomes `QuoteUpdate::ts` exactly.
<!-- id: REQ-STRM-010.A4 -->
- [x] A first response lacking `currency` emits nothing, the stream keeps polling, and the next valid response is emitted.
<!-- id: REQ-STRM-010.A5 -->
- [ ] A transient HTTP error from the quote endpoint does not terminate the polling stream. [needs review — no test backs this]
<!-- id: REQ-STRM-010.A6 -->
- [ ] When `regularMarketPrice` is absent but `regularMarketPreviousClose` is present, `price` equals the previous close. [needs review — no test backs this]

<!-- id: REQ-STRM-011 -->
## Change-only filtering (`diff_only`)

When `diff_only` is `true` (default), a polled quote is emitted only if its effective price (`Option<f64>`, see REQ-STRM-010) differs from the last *emitted* price for that symbol; the first observation (no prior entry) always emits. The last price is recorded only after a successful send, so skipped nodes (e.g. no currency) do not suppress the next valid one. Volume-only or previous-close-only changes do not emit. When `false`, every polled node is emitted.

Scope: applied in `Polling` and in the polling windows of `WebsocketWithFallback`; never applied to WebSocket-sourced updates. The doc comment on `StreamBuilder::diff_only` says "Only used for `Polling` method". [needs review — doc/behaviour mismatch for fallback mode]

implementations:
  - src/stream/mod.rs:handle_polling_quotes
  - src/stream/mod.rs:StreamBuilder.diff_only

verifies:
  - tests/stream/offline.rs:stream_polling_diff_only_ignores_volume_only_change
  - tests/stream/offline.rs:stream_polling_diff_only_does_not_track_skipped_updates

<!-- id: REQ-STRM-011.A1 -->
- [x] With default `diff_only`, a second poll with the same price but higher volume (1000→1500) emits nothing within 350 ms.
<!-- id: REQ-STRM-011.A2 -->
- [x] A skipped first update (missing currency) is not recorded, so the next valid quote with the same price is still emitted.
<!-- id: REQ-STRM-011.A3 -->
- [ ] With `diff_only(false)`, an unchanged price is emitted on every tick. [needs review — no test backs this; only example 06 uses it]

<!-- id: REQ-STRM-012 -->
## Stream lifecycle: stop, abort, and consumer drop

`pub struct StreamHandle` exposes:
- `pub async fn stop(self)` — sends the stop signal and awaits the task. Stop is observed while waiting for WebSocket startup, while waiting on frames, while waiting for a poll tick, while a quote fetch is in flight, and between fallback reconnects. No WebSocket Close frame is sent on stop. [needs review — inferred from code, no graceful-close logic]
- `pub fn abort(self)` — aborts the task immediately.

The task also ends when the receiver is dropped (detected via `tx.is_closed()` or a failed `send`). Dropping the `StreamHandle` drops the stop sender, which the task observes as a stop request. [needs review — implicit behaviour, undocumented; see finding F3]

implementations:
  - src/stream/mod.rs:StreamHandle
  - src/stream/mod.rs:StreamHandle.stop
  - src/stream/mod.rs:StreamHandle.abort
  - src/stream/mod.rs:websocket_stop_requested
  - src/stream/mod.rs:wait_for_poll_tick
  - src/stream/mod.rs:poll_stream_once
  - src/stream/mod.rs:poll_until_websocket_reconnect

verifies:
  - tests/stream/offline.rs:stream_polling_stop_cancels_in_flight_quote_fetch
  - tests/stream/offline.rs:stream_websocket_fallback_stop_cancels_startup
  - tests/stream/offline.rs:stream_websocket_replies_to_ping

<!-- id: REQ-STRM-012.A1 -->
- [x] `stop()` completes within 250 ms while a polling quote request is stalled for 2 s.
<!-- id: REQ-STRM-012.A2 -->
- [x] `stop()` completes within 250 ms while a fallback-mode WebSocket upgrade is stalled for 2 s (connect timeout 5 s).
<!-- id: REQ-STRM-012.A3 -->
- [x] `stop()` on an established `Websocket` stream returns.
<!-- id: REQ-STRM-012.A4 -->
- [ ] Dropping the receiver terminates the background task. [needs review — no test backs this]
<!-- id: REQ-STRM-012.A5 -->
- [ ] Dropping the `StreamHandle` without calling `stop()` terminates the stream. [needs review — no test backs this; may be unintended]

<!-- id: STRM-NONGOALS -->
## Non-goals

- Inferring per-update volume deltas, session boundaries, resets, or provider adjustments (explicit comment, src/stream/mod.rs:58-63).
- Exposing the remaining `PricingData` fields (bid/ask, day high/low, change, market hours, options fields, crypto supply, etc.) — decoded but not projected into `QuoteUpdate` because `QuoteUpdate` has no slot for them. [needs review — inferred from code shape, not an explicit statement]
- Surfacing runtime stream errors to the consumer; failures after startup are only logged (doc comment on `StreamBuilder::start`).
- Dynamic subscribe/unsubscribe after start: `[fill in — not derivable from code]`.

<!-- id: STRM-OPEN -->
## Open questions

- Should `diff_only` apply to WebSocket updates too, or only to polling? Current docs say Polling only; code also applies it to fallback polling.
- Should the reconnect backoff counter reset after a session that stayed healthy for some time?
- Should a WebSocket `day_volume` of 0 be reported as `None` (consistent with `previous_close`) or as `Some(0)`?
- Should a WebSocket `time` of 0 (protobuf default) be skipped or fall back to `Utc::now()` like the polling path, instead of emitting a 1970 timestamp?
- Is polling's use of `regularMarketPreviousClose` as `price` when the price is missing intended?
- Is "dropping the handle stops the stream" an intended contract? If so it should be documented; if not, `StreamHandle` should detach.
- Should the channel capacity (1024) be configurable?
- Recent commit 2a62c0b ("deflake hanging-quote retry assertion") touches only tests/core/public_validation.rs (quote retry against a hanging server), not the stream module. Confirm no stream test was meant to be included.

<!-- id: STRM-REVIEW -->
## Code review findings

Ranked by severity.

<!-- id: STRM-REVIEW-F1 -->
### F1 (high) — `stop()` can hang when the consumer is not draining the channel

src/stream/mod.rs:536, :554, :563 (WebSocket) and :1035-1053 (polling) await `tx.send(update)` inside a `select!` arm body, so `stop_rx` is not polled while blocked. If the receiver is alive but not read and the 1024-slot channel is full, `StreamHandle::stop()` (src/stream/mod.rs:94-99) awaits the join handle forever. The idle timer is also not polled then. Missing test.

<!-- id: STRM-REVIEW-F2 -->
### F2 (medium) — Polling bursts after a long WebSocket session (tokio `Interval` default `MissedTickBehavior::Burst`)

src/stream/mod.rs:616 creates the fallback `Interval` once and only ticks it inside `poll_until_websocket_reconnect` (:689). After a WebSocket session of N intervals, the next fallback window fires back-to-back catch-up ticks, issuing a rapid burst of v7 quote requests (rate-limit risk) until the reconnect deadline. Same pattern for a slow consumer in `run_polling_stream` (:917). Suggest `set_missed_tick_behavior(Delay/Skip)` or `reset()` before each window. Missing test.

<!-- id: STRM-REVIEW-F3 -->
### F3 (medium) — Dropping `StreamHandle` stops the stream

`StreamHandle` holds `stop_tx: Option<oneshot::Sender<()>>` (src/stream/mod.rs:89). Dropping it closes the oneshot, which `&mut *stop_rx` (:496, :599, :705, :947, :968) and `websocket_stop_requested` (:393-395) treat as a stop. `let (_, mut rx) = builder.start().await?;` therefore stops immediately — an undocumented footgun. Missing test.

<!-- id: STRM-REVIEW-F4 -->
### F4 (medium) — Reconnect backoff never resets

`websocket_failures` (src/stream/mod.rs:618) is only incremented (:637). After about 10 disconnects over a long-running stream (1s interval), every later outage waits the full 30s cap before reconnecting, even after hours of healthy streaming.

<!-- id: STRM-REVIEW-F5 -->
### F5 (medium) — Protobuf defaults projected as real data on the WebSocket path

- `quantity_from_i64(ticker.day_volume)` (src/stream/mod.rs:857) turns an omitted `day_volume` (default 0) into `Some(0)`, while `previous_close = 0` becomes `None` (:817). Inconsistent.
- `ws_pricing_timestamp` (:758-765) accepts `time = 0` and emits a 1970-01-01 timestamp; polling falls back to `Utc::now()` (:997).

<!-- id: STRM-REVIEW-F6 -->
### F6 (low) — `diff_only` doc/behaviour mismatch and stale state across transports

The doc at src/stream/mod.rs:194 says Polling only, but fallback polling also filters (:972). WebSocket updates never update `last_price` (:617), so after a reconnect cycle the comparison uses a price that may be stale. NaN prices compare unequal and always emit (:1006).

<!-- id: STRM-REVIEW-F7 -->
### F7 (low) — Production mapping is not the path the decoder tests exercise

`decode_and_map_message` (src/stream/mod.rs:871-907) duplicates `map_ws_pricing_to_update` (:827-866) with different semantics: it bypasses the instrument cache and returns errors instead of skipping. tests/stream/websocket_decoder.rs therefore tests a parallel path. The skip-on-missing-currency and skip-on-bad-timestamp branches of the live path are untested.

<!-- id: STRM-REVIEW-F8 -->
### F8 (low) — Errors are invisible to consumers

Polling fetch errors are logged at debug level only (src/stream/mod.rs:973-981), and WebSocket runtime errors at warn level (:301-307, :638-641). Without `tracing`, a permanently failing stream (bad symbols, auth or rate limit) looks the same as a quiet market. There is no status channel.

<!-- id: STRM-REVIEW-F9 -->
### F9 (low) — Smaller items

- No Close frame is sent on `stop()` (src/stream/mod.rs:599-600), so the server sees an abrupt disconnect.
- Polling substitutes previous close for a missing price (:999-1003). Consumers cannot tell a stale price from a live one.
- A polled node with no `symbol` defaults to `""` (:998) and is silently dropped by `Instrument::from_symbol`.
- Duplicate symbols are not removed before subscribing or polling (:243).
- `StreamBuilder::start` docs (:248-250) list only the empty-symbols error, but it also returns errors for zero durations and invalid symbols.
- Missing tests: raw-binary protobuf frames, accept-key mismatch, receiver-drop termination, `diff_only(false)`, `Websocket`-mode runtime closure, empty symbol list.

<!-- id: STRM-NOTES -->
## Reverse-engineering notes

- Confidence: **high** for REQ-STRM-001 to 005 and 008 to 012, which are directly backed by offline tests with explicit assertions. **Medium** for REQ-STRM-006 (volume `Some(0)` edge) and REQ-STRM-007 (cached-instrument reuse, which no test covers).
- The tests were **not executed** for this spec. The local toolchain is rustc 1.88, but the crate needs 1.91 and `paft` 0.9 needs 1.90. `[x]` marks criteria that a test asserts. Their pass status is assumed from the clean `main` branch. [needs review — confirm with `cargo test --features stream,test-mode --test stream --test core` on rustc >= 1.91]
- `tests/stream/live.rs:live_stream_smoke` is `#[ignore]` and gated on live/record env. It is not cited as a verifier. It checks that BTC-USD emits a positively priced tick within 2 minutes over the live WebSocket.
- Examples: 01 uses `WebsocketWithFallback` defaults for 10 s. 06 uses `Polling`, a 5 s interval and `diff_only(false)`. 16 audits all three methods with a 2 s interval, `diff_only(false)` and `CacheMode::Use`. Examples are illustrative and are not treated as acceptance evidence.
- The `stream` feature pulls in `base64`, `futures-util`, `prost` and `tokio-tungstenite` (Cargo.toml:55). `yaticker.rs` is checked-in generated prost code. How it is regenerated is not visible in `src/stream`. [needs review — build.rs/regeneration process not inspected]
- Business purpose of the default 90 s idle timeout comes from the code comment ("multiple missed heartbeat windows"). Yahoo's actual ping cadence is not documented in the repo. [needs review — provider heartbeat interval unverified]
