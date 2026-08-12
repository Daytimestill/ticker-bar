use tickerbar_core::{
    AppConfig, LongbridgeQuoteProvider, MockQuoteProvider, ProviderError, QuoteProvider,
    StockConfig, TencentQuoteProvider,
};

fn default_stock() -> StockConfig {
    AppConfig::default().stocks[0].clone()
}

#[test]
fn mock_provider_emits_a_complete_quote_for_the_configured_stock() {
    let stock = default_stock();
    let mut provider = MockQuoteProvider::default();

    let update = provider
        .next_quote(&stock, 1_722_412_338, "14:32")
        .expect("mock quote should be available");

    assert_eq!(update.quote.symbol.as_deref(), Some("01810.HK"));
    assert_eq!(update.quote.short_name.as_deref(), Some("小米"));
    assert_eq!(update.quote.currency, "HKD");
    assert_eq!(update.quote.updated_time.as_deref(), Some("14:32"));
    assert!(update.quote.day_high.is_some());
    assert!(update.quote.day_low.is_some());
}

#[test]
fn mock_provider_changes_price_without_drifting_away_forever() {
    let stock = default_stock();
    let mut provider = MockQuoteProvider::default();
    let first = provider
        .next_quote(&stock, 1, "09:30")
        .expect("first quote should work")
        .quote
        .last_price;

    for timestamp in 2..=50 {
        provider
            .next_quote(&stock, timestamp, "09:31")
            .expect("subsequent quote should work");
    }
    let later = provider
        .next_quote(&stock, 51, "09:32")
        .expect("later quote should work")
        .quote
        .last_price;

    assert_ne!(first, later);
    assert!(later >= "40".parse().expect("fixture decimal should parse"));
    assert!(later <= "45".parse().expect("fixture decimal should parse"));
}

#[test]
fn longbridge_boundary_fails_explicitly_until_credentials_are_configured() {
    let stock = default_stock();
    let mut provider = LongbridgeQuoteProvider;

    let error = provider
        .next_quote(&stock, 1, "09:30")
        .expect_err("unfinished provider must not pretend to be live");

    assert_eq!(error, ProviderError::NotConfigured);
}

#[tokio::test]
#[ignore = "requires the live Tencent quote endpoint"]
async fn tencent_provider_fetches_the_configured_stock() {
    let config = AppConfig::default();
    let provider = TencentQuoteProvider::new().expect("HTTP client should initialize");

    let mut results = provider
        .next_quotes(&config.stocks, 1_775_184_040)
        .await
        .expect("live quote request should succeed");
    let update = results.remove(0).expect("live quote should be available");

    assert_eq!(update.quote.symbol.as_deref(), Some("01810.HK"));
    assert!(update.quote.last_price.is_sign_positive());
    assert!(update.quote.previous_close.is_sign_positive());
    assert!(update.quote.updated_time.is_some());
}

#[tokio::test]
#[ignore = "requires the live Tencent stock search endpoint"]
async fn tencent_provider_searches_stocks_by_name() {
    let provider = TencentQuoteProvider::new().expect("HTTP client should initialize");

    let results = provider
        .search_stocks("浪潮软件")
        .await
        .expect("live stock search should be available");

    assert!(results.iter().any(|result| {
        result.symbol == "600756.SH"
            && result.name == "浪潮软件"
            && result.market == "沪市"
            && result.currency == "CNY"
    }));
}
