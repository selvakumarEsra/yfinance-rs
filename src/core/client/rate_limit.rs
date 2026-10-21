//! Optional client-side request rate limiting (token bucket).

use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::core::YfError;

/// A client-side limit on outbound HTTP requests, configured with
/// [`YfClientBuilder::rate_limit`](crate::YfClientBuilder::rate_limit).
///
/// The limit is a token bucket: up to `burst` requests may be sent back-to-back,
/// after which requests are spaced at `per_second` per second. The bucket refills
/// while the client is idle. Every HTTP attempt (including retries and cookie/crumb
/// requests) consumes one token; the WebSocket stream is not limited.
///
/// ```
/// use yfinance_rs::{RateLimit, YfClient};
///
/// let client = YfClient::builder()
///     .rate_limit(RateLimit::per_second(5).burst(10))
///     .build()?;
/// # Ok::<(), yfinance_rs::YfError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RateLimit {
    /// Sustained number of requests allowed per second. Must be greater than zero.
    pub per_second: u32,
    /// Number of requests that may be sent back-to-back before spacing applies.
    /// Must be greater than zero. Defaults to `1`.
    pub burst: u32,
}

impl RateLimit {
    /// Creates a limit of `n` requests per second with a burst of one request.
    #[must_use]
    pub const fn per_second(n: u32) -> Self {
        Self {
            per_second: n,
            burst: 1,
        }
    }

    /// Sets how many requests may be sent back-to-back before spacing applies.
    #[must_use]
    pub const fn burst(mut self, burst: u32) -> Self {
        self.burst = burst;
        self
    }

    /// Validates that this limit can be enforced.
    ///
    /// # Errors
    ///
    /// Returns `YfError::InvalidParams` when `per_second` or `burst` is zero.
    pub fn validate(&self) -> Result<(), YfError> {
        if self.per_second == 0 {
            return Err(YfError::InvalidParams(
                "rate limit per_second must be greater than zero".into(),
            ));
        }
        if self.burst == 0 {
            return Err(YfError::InvalidParams(
                "rate limit burst must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}

/// Shared token-bucket state, implemented as GCRA (generic cell rate algorithm):
/// `next_at` is the theoretical arrival time of the next request; a request may go
/// once `now >= next_at - tolerance`, where `tolerance` covers the burst allowance.
#[derive(Debug)]
pub struct RateLimiter {
    limit: RateLimit,
    interval: Duration,
    tolerance: Duration,
    next_at: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(limit: RateLimit) -> Result<Self, YfError> {
        limit.validate()?;
        let interval = Duration::from_secs(1) / limit.per_second;
        Ok(Self {
            limit,
            interval,
            tolerance: interval.saturating_mul(limit.burst - 1),
            next_at: Mutex::new(None),
        })
    }

    pub const fn limit(&self) -> RateLimit {
        self.limit
    }

    /// Reserves a slot and returns how long the caller must wait before sending.
    async fn reserve(&self) -> Duration {
        let mut next_at = self.next_at.lock().await;
        let now = Instant::now();
        let start = next_at.map_or(now, |at| at.max(now));
        let send_at = start
            .checked_sub(self.tolerance)
            .map_or(now, |at| at.max(now));
        *next_at = Some(start + self.interval);
        drop(next_at);
        send_at - now
    }

    /// Waits until a request may be sent.
    pub async fn acquire(&self) {
        let wait = self.reserve().await;
        if !wait.is_zero() {
            crate::core::logging::trace_debug!(
                wait_ms = wait.as_secs_f64() * 1000.0,
                "client-side rate limit delaying request"
            );
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_constructor_defaults_burst_to_one() {
        let limit = RateLimit::per_second(4);
        assert_eq!(limit.per_second, 4);
        assert_eq!(limit.burst, 1);
        assert_eq!(limit.burst(8).burst, 8);
    }

    #[test]
    fn rate_limit_rejects_zero_rate_or_burst() {
        assert!(RateLimit::per_second(0).validate().is_err());
        assert!(RateLimit::per_second(1).burst(0).validate().is_err());
        assert!(RateLimit::per_second(1).burst(1).validate().is_ok());
    }

    #[tokio::test]
    async fn rate_limiter_reserves_burst_then_spaces_requests() {
        let limiter = RateLimiter::new(RateLimit::per_second(10).burst(3)).unwrap();
        let mut waits = Vec::new();
        for _ in 0..6 {
            waits.push(limiter.reserve().await);
        }
        // First three are free; the next three are spaced 100 ms apart.
        for wait in &waits[..3] {
            assert!(*wait < Duration::from_millis(5), "burst wait {wait:?}");
        }
        for (k, wait) in waits[3..].iter().enumerate() {
            let expected = Duration::from_millis(100) * (u32::try_from(k).unwrap() + 1);
            assert!(
                *wait <= expected && *wait + Duration::from_millis(20) >= expected,
                "extra request {k}: wait {wait:?}, expected about {expected:?}"
            );
        }
    }

    #[tokio::test]
    async fn rate_limiter_refills_after_idle() {
        let limiter = RateLimiter::new(RateLimit::per_second(50).burst(2)).unwrap();
        limiter.reserve().await;
        limiter.reserve().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(limiter.reserve().await < Duration::from_millis(5));
        assert!(limiter.reserve().await < Duration::from_millis(5));
    }
}
