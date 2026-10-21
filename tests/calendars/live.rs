use yfinance_rs::{CalendarsBuilder, Ticker, YfClient};

#[tokio::test]
#[ignore = "hits live Yahoo Finance visualization endpoint"]
async fn live_calendars_and_earnings_dates_smoke() {
    if !crate::common::live_or_record_enabled() {
        return;
    }

    let client = YfClient::default();
    let builder = CalendarsBuilder::new(&client).limit(5);
    builder.earnings().await.unwrap();
    builder.ipos().await.unwrap();
    builder.splits().await.unwrap();
    builder.economic_events().await.unwrap();

    let rows = Ticker::new(&client, "AAPL")
        .earnings_dates(Some(8))
        .await
        .unwrap();
    assert!(rows.len() <= 8);
}
