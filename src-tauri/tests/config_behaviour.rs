use tickerbar_core::{
    AppConfig, CompactStyle, DisplayMetric, DisplayPreset, ProviderKind, QuickViewSort,
    apply_display_preset,
};

#[test]
fn default_config_uses_a_real_quote_source() {
    let config = AppConfig::default();

    assert_eq!(config.provider, ProviderKind::Tencent);
    assert_eq!(config.stocks.len(), 1);
    assert_eq!(config.stocks[0].symbol, "01810.HK");
    assert_eq!(config.stocks[0].short_name, "小米");
    assert_eq!(config.active_stock, 0);
    assert_eq!(config.tray_throttle_ms, 3_000);
    assert!(config.quick_view_enabled);
    assert_eq!(config.quick_view_shortcut, "Control+Command+T");
    assert_eq!(config.quick_view_sort, QuickViewSort::PositionFirst);
    assert_eq!(config.display.items.len(), 2);
    assert_eq!(config.display.items[0].metric, DisplayMetric::LastPrice);
    assert_eq!(
        config.display.items[1].metric,
        DisplayMetric::DailyChangePercent
    );
    config.validate().expect("default config must be valid");
}

#[test]
fn older_configs_receive_safe_quick_view_defaults() {
    let mut stored = serde_json::to_value(AppConfig::default()).expect("config serializes");
    stored
        .as_object_mut()
        .expect("config should be an object")
        .remove("quickViewEnabled");
    stored
        .as_object_mut()
        .expect("config should be an object")
        .remove("quickViewShortcut");
    stored
        .as_object_mut()
        .expect("config should be an object")
        .remove("quickViewSort");

    let restored: AppConfig =
        serde_json::from_value(stored).expect("older config should still load");

    assert!(restored.quick_view_enabled);
    assert_eq!(restored.quick_view_shortcut, "Control+Command+T");
    assert_eq!(restored.quick_view_sort, QuickViewSort::PositionFirst);
}

#[test]
fn json_round_trip_preserves_display_order_and_formatting() {
    let default_config = AppConfig::default();
    let mut config = AppConfig {
        display: tickerbar_core::DisplayConfig {
            items: apply_display_preset(DisplayPreset::Position),
            separator: " · ".into(),
            ..default_config.display
        },
        ..default_config
    };
    config.display.items[0].compact_style = CompactStyle::Chinese;

    let json = serde_json::to_string_pretty(&config).expect("config should serialize");
    let restored: AppConfig = serde_json::from_str(&json).expect("config should deserialize");

    assert_eq!(restored, config);
    assert_eq!(
        restored.display.items[0].metric,
        DisplayMetric::PositionProfit
    );
    assert_eq!(
        restored.display.items[1].metric,
        DisplayMetric::PositionReturnPercent
    );
}

#[test]
fn older_configs_enable_display_items_by_default() {
    let mut stored = serde_json::to_value(AppConfig::default()).expect("config serializes");
    for item in stored["display"]["items"]
        .as_array_mut()
        .expect("display items should be an array")
    {
        item.as_object_mut()
            .expect("metric should be an object")
            .remove("enabled");
    }

    let restored: AppConfig =
        serde_json::from_value(stored).expect("older display items should still load");

    assert!(restored.display.items.iter().all(|item| item.enabled));
}

#[test]
fn presets_create_expected_field_combinations() {
    let price = apply_display_preset(DisplayPreset::Price);
    assert_eq!(
        price.iter().map(|item| item.metric).collect::<Vec<_>>(),
        vec![DisplayMetric::LastPrice]
    );

    let price_change = apply_display_preset(DisplayPreset::PriceChange);
    assert_eq!(
        price_change
            .iter()
            .map(|item| item.metric)
            .collect::<Vec<_>>(),
        vec![DisplayMetric::LastPrice, DisplayMetric::DailyChangePercent]
    );

    let position = apply_display_preset(DisplayPreset::Position);
    assert_eq!(
        position.iter().map(|item| item.metric).collect::<Vec<_>>(),
        vec![
            DisplayMetric::PositionProfit,
            DisplayMetric::PositionReturnPercent
        ]
    );
}

#[test]
fn rejects_empty_symbol_and_empty_display_selection() {
    let mut no_symbol = AppConfig::default();
    no_symbol.stocks[0].symbol = "  ".into();
    assert_eq!(
        no_symbol
            .validate()
            .expect_err("blank symbol must fail")
            .to_string(),
        "symbol is required"
    );

    let default_config = AppConfig::default();
    let no_metrics = AppConfig {
        display: tickerbar_core::DisplayConfig {
            items: Vec::new(),
            ..default_config.display
        },
        ..default_config
    };
    assert_eq!(
        no_metrics
            .validate()
            .expect_err("empty display selection must fail")
            .to_string(),
        "at least one display metric is required"
    );

    let mut all_paused = AppConfig::default();
    for item in &mut all_paused.display.items {
        item.enabled = false;
    }
    assert_eq!(
        all_paused
            .validate()
            .expect_err("at least one metric must stay enabled")
            .to_string(),
        "at least one display metric must be enabled"
    );
}

#[test]
fn provider_config_does_not_contain_credentials() {
    let config = AppConfig::default();
    let json = serde_json::to_string(&config).expect("config should serialize");

    assert!(!json.contains("access_token"));
    assert!(!json.contains("app_secret"));
    assert!(!json.contains("password"));
}
