use crate::core::error::YfError;

const MAX_RETRIES: u32 = 100;

/// Specifies the backoff strategy for retrying failed requests.
#[derive(Clone, Debug)]
pub enum Backoff {
    /// Uses a fixed delay between retries.
    Fixed(std::time::Duration),
    /// Uses an exponential delay between retries.
    /// The delay is calculated as `base * (factor ^ attempt)`.
    Exponential {
        /// The initial backoff duration.
        base: std::time::Duration,
        /// The multiplicative factor for each subsequent retry.
        factor: f64,
        /// The maximum duration to wait between retries.
        max: std::time::Duration,
        /// Whether to apply random jitter (+/- 50%) to the delay.
        jitter: bool,
    },
}

impl Backoff {
    /// Validates that this backoff strategy can be used without panicking during retry scheduling.
    ///
    /// # Errors
    ///
    /// Returns `YfError::InvalidParams` when exponential backoff has a non-finite or
    /// non-positive factor.
    pub fn validate(&self) -> Result<(), YfError> {
        match self {
            Self::Exponential { factor, .. } if !factor.is_finite() || *factor <= 0.0 => {
                Err(YfError::InvalidParams(
                    "retry exponential backoff factor must be finite and greater than zero".into(),
                ))
            }
            Self::Fixed(_) | Self::Exponential { .. } => Ok(()),
        }
    }

    /// The longest delay this strategy will wait between two attempts.
    ///
    /// For [`Backoff::Fixed`] this is the fixed delay; for [`Backoff::Exponential`]
    /// it is `max`. A server-supplied `Retry-After` is capped at this value.
    #[must_use]
    pub const fn max_delay(&self) -> std::time::Duration {
        match *self {
            Self::Fixed(d) => d,
            Self::Exponential { max, .. } => max,
        }
    }
}

/// Configuration for the automatic retry mechanism.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct RetryConfig {
    /// Enables or disables the retry mechanism.
    pub enabled: bool,
    /// The maximum number of retries to attempt. The total number of attempts will be `max_retries + 1`.
    pub max_retries: u32,
    /// The backoff strategy to use between retries.
    pub backoff: Backoff,
    /// A list of HTTP status codes that should trigger a retry.
    pub retry_on_status: Vec<u16>,
    /// Whether to retry on request timeouts.
    pub retry_on_timeout: bool,
    /// Whether to retry on connection errors.
    pub retry_on_connect: bool,
    /// Whether to honour a `Retry-After` header on `429` and `503` responses.
    ///
    /// When `true` (the default) and a retried `429`/`503` response carries a
    /// `Retry-After` header (delta-seconds or HTTP-date), the next attempt waits at
    /// least that long, capped at [`Backoff::max_delay`]. The wait is never shorter
    /// than the regular backoff delay. When `false`, only the backoff strategy is used.
    pub respect_retry_after: bool,
}

impl RetryConfig {
    /// Highest accepted retry count for enabled retry policies.
    pub const MAX_RETRIES: u32 = MAX_RETRIES;

    /// Validates that this retry policy can be used safely.
    ///
    /// Disabled retry policies skip retry-specific validation because the retry
    /// fields are not observed while sending requests.
    ///
    /// # Errors
    ///
    /// Returns `YfError::InvalidParams` when an enabled policy has too many
    /// retries or an invalid backoff strategy.
    pub fn validate(&self) -> Result<(), YfError> {
        if !self.enabled {
            return Ok(());
        }

        if self.max_retries > Self::MAX_RETRIES {
            return Err(YfError::InvalidParams(format!(
                "retry max_retries cannot exceed {}",
                Self::MAX_RETRIES
            )));
        }

        self.backoff.validate()
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 4,
            backoff: Backoff::Exponential {
                base: std::time::Duration::from_millis(200),
                factor: 2.0,
                max: std::time::Duration::from_secs(3),
                jitter: true,
            },
            retry_on_status: vec![408, 429, 500, 502, 503, 504],
            retry_on_timeout: true,
            retry_on_connect: true,
            respect_retry_after: true,
        }
    }
}

/// Statuses whose `Retry-After` header is honoured by the retry loop.
const RETRY_AFTER_STATUSES: [u16; 2] = [429, 503];

/// Computes the delay before the next retry of a response with `status`.
///
/// `backoff` is the delay from the configured backoff strategy; `retry_after` is the
/// parsed `Retry-After` header, if any. The result is never below `backoff`, and a
/// `Retry-After` hint is capped at the backoff strategy's maximum delay.
pub fn retry_delay_for_status(
    cfg: &RetryConfig,
    status: u16,
    backoff: std::time::Duration,
    retry_after: Option<std::time::Duration>,
) -> std::time::Duration {
    if !wants_retry_after(cfg, status) {
        return backoff;
    }
    retry_after.map_or(backoff, |hint| {
        backoff.max(hint.min(cfg.backoff.max_delay()))
    })
}

/// Whether the `Retry-After` header should be inspected for a response with `status`.
pub fn wants_retry_after(cfg: &RetryConfig, status: u16) -> bool {
    cfg.respect_retry_after && RETRY_AFTER_STATUSES.contains(&status)
}

/// Parses a `Retry-After` header value (RFC 9110 section 10.2.3).
///
/// Accepts delta-seconds (`"120"`) and HTTP-dates (IMF-fixdate, plus the obsolete
/// RFC 850 and asctime forms). Dates in the past yield a zero duration. Returns
/// `None` when the value is not understood.
pub fn parse_retry_after(
    value: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<std::time::Duration> {
    use chrono::{DateTime, NaiveDateTime, Utc};

    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.bytes().all(|b| b.is_ascii_digit()) {
        // Overflowing delta-seconds are "very long"; the caller caps them anyway.
        let secs = value.parse::<u64>().unwrap_or(u64::MAX);
        return Some(std::time::Duration::from_secs(secs));
    }

    let at: DateTime<Utc> = DateTime::parse_from_rfc2822(value)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%A, %d-%b-%y %H:%M:%S GMT")
                .or_else(|_| NaiveDateTime::parse_from_str(value, "%a %b %e %H:%M:%S %Y"))
                .map(|naive| naive.and_utc())
        })
        .ok()?;

    Some((at - now).to_std().unwrap_or(std::time::Duration::ZERO))
}

/// Defines the behavior of the in-memory cache for an API call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CacheMode {
    /// Use the endpoint's default cache policy.
    ///
    /// Volatile endpoints, such as quotes, options, news, screeners, market
    /// status/summary, and visualization calendars, bypass the cache by default.
    /// More stable endpoints, such as historical charts, quote summaries,
    /// fundamentals, profile HTML, search, and symbol lookup, use the cache
    /// when client-side caching is enabled.
    #[default]
    Default,
    /// Read from the cache if a non-expired entry is present; otherwise, fetch from the network
    /// and write the response to the cache.
    Use,
    /// Always fetch from the network, bypassing any cached entry, and write the new response to the cache.
    Refresh,
    /// Always fetch from the network and do not read from or write to the cache.
    Bypass,
}

/// Endpoint buckets used by the in-memory response cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CacheEndpoint {
    /// The v7 quote endpoint.
    Quote,
    /// The historical chart endpoint.
    Chart,
    /// The v10 quoteSummary endpoint.
    QuoteSummary,
    /// The fundamentals-timeseries endpoint.
    Fundamentals,
    /// The v7 options endpoint.
    Options,
    /// The news endpoint.
    News,
    /// The search endpoint.
    Search,
    /// Screener endpoints.
    Screener,
    /// Market time and market summary endpoints.
    Market,
    /// The symbol lookup endpoint.
    Lookup,
    /// The v1 visualization endpoint (event calendars and earnings dates).
    Visualization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EffectiveCacheMode {
    Use,
    Refresh,
    Bypass,
}

impl CacheEndpoint {
    const fn default_mode(self) -> EffectiveCacheMode {
        match self {
            Self::Quote
            | Self::Options
            | Self::News
            | Self::Screener
            | Self::Market
            | Self::Visualization => EffectiveCacheMode::Bypass,
            Self::Chart | Self::QuoteSummary | Self::Fundamentals | Self::Search | Self::Lookup => {
                EffectiveCacheMode::Use
            }
        }
    }
}

impl CacheMode {
    const fn effective(self, endpoint: CacheEndpoint) -> EffectiveCacheMode {
        match self {
            Self::Default => endpoint.default_mode(),
            Self::Use => EffectiveCacheMode::Use,
            Self::Refresh => EffectiveCacheMode::Refresh,
            Self::Bypass => EffectiveCacheMode::Bypass,
        }
    }

    pub(crate) const fn reads(self, endpoint: CacheEndpoint) -> bool {
        matches!(self.effective(endpoint), EffectiveCacheMode::Use)
    }

    pub(crate) const fn writes(self, endpoint: CacheEndpoint) -> bool {
        matches!(
            self.effective(endpoint),
            EffectiveCacheMode::Use | EffectiveCacheMode::Refresh
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::time::Duration;

    fn fixed_now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2015, 10, 21, 7, 28, 0).unwrap()
    }

    fn exp_config(max: Duration, respect: bool) -> RetryConfig {
        RetryConfig {
            backoff: Backoff::Exponential {
                base: Duration::from_millis(10),
                factor: 2.0,
                max,
                jitter: false,
            },
            respect_retry_after: respect,
            ..RetryConfig::default()
        }
    }

    #[test]
    fn retry_after_parses_delta_seconds() {
        let now = fixed_now();
        assert_eq!(parse_retry_after("2", now), Some(Duration::from_secs(2)));
        assert_eq!(parse_retry_after("  0 ", now), Some(Duration::ZERO));
        assert_eq!(parse_retry_after("120", now), Some(Duration::from_mins(2)));
    }

    #[test]
    fn retry_after_overflowing_delta_seconds_is_very_long() {
        let parsed = parse_retry_after("99999999999999999999999", fixed_now()).unwrap();
        assert_eq!(parsed, Duration::from_secs(u64::MAX));
    }

    #[test]
    fn retry_after_parses_imf_fixdate() {
        let parsed = parse_retry_after("Wed, 21 Oct 2015 07:28:05 GMT", fixed_now());
        assert_eq!(parsed, Some(Duration::from_secs(5)));
    }

    #[test]
    fn retry_after_parses_obsolete_http_date_forms() {
        let now = fixed_now();
        assert_eq!(
            parse_retry_after("Wednesday, 21-Oct-15 07:28:03 GMT", now),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            parse_retry_after("Wed Oct 21 07:28:04 2015", now),
            Some(Duration::from_secs(4))
        );
    }

    #[test]
    fn retry_after_http_date_in_the_past_is_zero() {
        let parsed = parse_retry_after("Wed, 21 Oct 2015 07:00:00 GMT", fixed_now());
        assert_eq!(parsed, Some(Duration::ZERO));
    }

    #[test]
    fn retry_after_rejects_garbage() {
        let now = fixed_now();
        assert_eq!(parse_retry_after("", now), None);
        assert_eq!(parse_retry_after("soon", now), None);
        assert_eq!(parse_retry_after("-5", now), None);
        assert_eq!(parse_retry_after("1.5", now), None);
    }

    #[test]
    fn retry_delay_uses_retry_after_when_within_max() {
        let cfg = exp_config(Duration::from_secs(3), true);
        let delay = retry_delay_for_status(
            &cfg,
            429,
            Duration::from_millis(10),
            Some(Duration::from_secs(2)),
        );
        assert_eq!(delay, Duration::from_secs(2));
    }

    #[test]
    fn retry_delay_caps_retry_after_at_max_delay() {
        let cfg = exp_config(Duration::from_secs(3), true);
        let delay = retry_delay_for_status(
            &cfg,
            503,
            Duration::from_millis(10),
            Some(Duration::from_secs(600)),
        );
        assert_eq!(delay, Duration::from_secs(3));
    }

    #[test]
    fn retry_delay_never_shorter_than_backoff() {
        let cfg = exp_config(Duration::from_secs(3), true);
        let delay = retry_delay_for_status(&cfg, 429, Duration::from_secs(1), Some(Duration::ZERO));
        assert_eq!(delay, Duration::from_secs(1));
    }

    #[test]
    fn retry_delay_ignores_retry_after_when_disabled() {
        let cfg = exp_config(Duration::from_secs(3), false);
        let delay = retry_delay_for_status(
            &cfg,
            429,
            Duration::from_millis(10),
            Some(Duration::from_secs(2)),
        );
        assert_eq!(delay, Duration::from_millis(10));
        assert!(!wants_retry_after(&cfg, 429));
    }

    #[test]
    fn retry_delay_ignores_retry_after_on_other_statuses() {
        let cfg = exp_config(Duration::from_secs(3), true);
        let delay = retry_delay_for_status(
            &cfg,
            500,
            Duration::from_millis(10),
            Some(Duration::from_secs(2)),
        );
        assert_eq!(delay, Duration::from_millis(10));
        assert!(wants_retry_after(&cfg, 429));
        assert!(wants_retry_after(&cfg, 503));
        assert!(!wants_retry_after(&cfg, 502));
    }

    #[test]
    fn retry_config_respects_retry_after_by_default() {
        assert!(RetryConfig::default().respect_retry_after);
    }
}
