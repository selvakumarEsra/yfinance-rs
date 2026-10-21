//! REQ-YGAP-005: `Retry-After` handling on 429/503 responses.

use std::time::{Duration, Instant};

use httpmock::{Method::GET, Mock, MockServer};
use url::Url;
use yfinance_rs::{Backoff, HistoryBuilder, RetryConfig, YfClient, YfError};

const SYM: &str = "RTRY";

fn chart_mock<'a>(server: &'a MockServer, status: u16, retry_after: &str) -> Mock<'a> {
    let retry_after = retry_after.to_string();
    server.mock(move |when, then| {
        when.method(GET).path(format!("/v8/finance/chart/{SYM}"));
        then.status(status)
            .header("retry-after", retry_after)
            .body("slow down");
    })
}

fn client_with(server: &MockServer, retry: RetryConfig) -> YfClient {
    YfClient::builder()
        .base_chart(Url::parse(&format!("{}/v8/finance/chart/", server.base_url())).unwrap())
        .retry_config(retry)
        .build()
        .unwrap()
}

fn exponential(max: Duration, max_retries: u32) -> RetryConfig {
    RetryConfig {
        max_retries,
        backoff: Backoff::Exponential {
            base: Duration::from_millis(1),
            factor: 1.0,
            max,
            jitter: false,
        },
        ..RetryConfig::default()
    }
}

async fn timed_fetch(client: &YfClient) -> (Duration, Result<(), YfError>) {
    let started = Instant::now();
    let result = HistoryBuilder::new(client, SYM).fetch().await.map(|_| ());
    (started.elapsed(), result)
}

#[tokio::test]
async fn retry_after_seconds_on_429_delays_retry_at_least_that_long() {
    let server = MockServer::start();
    let mock = chart_mock(&server, 429, "2");
    let client = client_with(&server, exponential(Duration::from_secs(5), 1));

    let (elapsed, result) = timed_fetch(&client).await;

    mock.assert_calls(2);
    assert!(
        matches!(result, Err(YfError::RateLimited { .. })),
        "{result:?}"
    );
    assert!(
        elapsed >= Duration::from_secs(2),
        "retried after {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "retried after {elapsed:?}"
    );
}

#[tokio::test]
async fn retry_after_http_date_on_503_delays_retry_until_that_date() {
    let server = MockServer::start();
    let at = chrono::Utc::now() + chrono::Duration::seconds(3);
    let http_date = at.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let deadline = chrono::DateTime::parse_from_rfc2822(&http_date).unwrap();
    let mock = chart_mock(&server, 503, &http_date);
    let client = client_with(&server, exponential(Duration::from_secs(5), 1));

    let (elapsed, result) = timed_fetch(&client).await;

    mock.assert_calls(2);
    assert!(
        matches!(result, Err(YfError::ServerError { status: 503, .. })),
        "{result:?}"
    );
    // The retry (the last request) must not happen before the advertised date.
    let finished = chrono::Utc::now();
    assert!(
        finished >= deadline,
        "finished {finished} before {deadline}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "retried after {elapsed:?}"
    );
}

#[tokio::test]
async fn retry_after_longer_than_max_delay_is_capped_at_max_delay() {
    let server = MockServer::start();
    let mock = chart_mock(&server, 429, "120");
    let client = client_with(&server, exponential(Duration::from_millis(300), 2));

    let (elapsed, result) = timed_fetch(&client).await;

    mock.assert_calls(3);
    assert!(
        matches!(result, Err(YfError::RateLimited { .. })),
        "{result:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(600),
        "retried after {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "retried after {elapsed:?}"
    );
}

#[tokio::test]
async fn retry_after_ignored_when_respect_retry_after_is_false() {
    let server = MockServer::start();
    let mock = chart_mock(&server, 429, "120");
    let retry = RetryConfig {
        max_retries: 2,
        backoff: Backoff::Fixed(Duration::from_millis(50)),
        respect_retry_after: false,
        ..RetryConfig::default()
    };
    let client = client_with(&server, retry);

    let (elapsed, result) = timed_fetch(&client).await;

    mock.assert_calls(3);
    assert!(
        matches!(result, Err(YfError::RateLimited { .. })),
        "{result:?}"
    );
    // Two fixed 50 ms backoffs; the 120 s Retry-After plays no part.
    assert!(
        elapsed >= Duration::from_millis(100),
        "retried after {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "retried after {elapsed:?}"
    );
}

#[tokio::test]
async fn retry_after_on_non_rate_limit_status_is_ignored() {
    let server = MockServer::start();
    let mock = chart_mock(&server, 500, "120");
    let client = client_with(&server, exponential(Duration::from_secs(5), 1));

    let (elapsed, result) = timed_fetch(&client).await;

    mock.assert_calls(2);
    assert!(
        matches!(result, Err(YfError::ServerError { status: 500, .. })),
        "{result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "retried after {elapsed:?}"
    );
}
