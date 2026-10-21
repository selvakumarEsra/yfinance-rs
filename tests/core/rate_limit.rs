//! REQ-YGAP-005: optional client-side rate limiting.

use std::time::{Duration, Instant};

use futures::future::join_all;
use httpmock::{Method::GET, Mock, MockServer};
use url::Url;
use yfinance_rs::{HistoryBuilder, RateLimit, YfClient, YfClientBuilder, YfError};

const SYM: &str = "RLIM";

fn not_found_mock(server: &MockServer) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(GET).path(format!("/v8/finance/chart/{SYM}"));
        then.status(404).body("not found");
    })
}

fn builder_for(server: &MockServer) -> YfClientBuilder {
    YfClient::builder()
        .base_chart(Url::parse(&format!("{}/v8/finance/chart/", server.base_url())).unwrap())
}

async fn fetch_once(client: YfClient) {
    let result = HistoryBuilder::new(&client, SYM).fetch().await;
    assert!(
        matches!(result, Err(YfError::NotFound { .. })),
        "{result:?}"
    );
}

/// Sends `count` concurrent requests, each from its own clone of `client`.
async fn timed_concurrent_burst(client: &YfClient, count: usize) -> Duration {
    let started = Instant::now();
    join_all((0..count).map(|_| fetch_once(client.clone()))).await;
    started.elapsed()
}

#[tokio::test]
async fn rate_limit_spreads_requests_beyond_burst_across_cloned_clients() {
    let server = MockServer::start();
    let mock = not_found_mock(&server);
    // N = 10/s, B = 2: 2 + 4 requests -> the extra 4 take about 4/10 s.
    let client = builder_for(&server)
        .rate_limit(RateLimit::per_second(10).burst(2))
        .build()
        .unwrap();

    let elapsed = timed_concurrent_burst(&client, 6).await;

    mock.assert_calls(6);
    assert!(
        elapsed >= Duration::from_millis(380),
        "burst took {elapsed:?}"
    );
    assert!(elapsed < Duration::from_secs(2), "burst took {elapsed:?}");
}

#[tokio::test]
async fn rate_limit_allows_burst_without_waiting() {
    let server = MockServer::start();
    let mock = not_found_mock(&server);
    // At 1 request/s any wait would be >= 1 s, so a fast burst proves no throttling.
    let client = builder_for(&server)
        .rate_limit(RateLimit::per_second(1).burst(4))
        .build()
        .unwrap();

    let elapsed = timed_concurrent_burst(&client, 4).await;

    mock.assert_calls(4);
    assert!(
        elapsed < Duration::from_millis(900),
        "burst took {elapsed:?}"
    );
}

#[tokio::test]
async fn no_rate_limit_configured_adds_no_wait() {
    let server = MockServer::start();
    let mock = not_found_mock(&server);
    let client = builder_for(&server).build().unwrap();

    let elapsed = timed_concurrent_burst(&client, 20).await;

    mock.assert_calls(20);
    // A limiter of even 5/s with burst 1 would need about 3.8 s for these 20 requests.
    assert!(
        elapsed < Duration::from_secs(3),
        "requests took {elapsed:?}"
    );
}

#[test]
fn rate_limit_with_zero_rate_or_burst_is_rejected_at_build() {
    for limit in [RateLimit::per_second(0), RateLimit::per_second(5).burst(0)] {
        let err = YfClient::builder().rate_limit(limit).build().unwrap_err();
        assert!(matches!(err, YfError::InvalidParams(_)), "{err:?}");
    }
}
