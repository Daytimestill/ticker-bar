use std::collections::HashMap;

use rust_decimal::Decimal;
use tickerbar_core::{
    AppConfig, ConnectionState, Position, ProviderUpdate, QuickViewSort, QuoteSnapshot,
    StockConfig, build_quick_view_snapshot,
};

fn stock(symbol: &str, name: &str, position: Option<(&str, &str)>) -> StockConfig {
    StockConfig {
        symbol: symbol.into(),
        short_name: name.into(),
        currency: "CNY".into(),
        position: position.map(|(quantity, cost)| {
            Position::try_new(quantity, cost).expect("valid fixture position")
        }),
    }
}

fn update(symbol: &str, name: &str, price: &str, previous_close: &str) -> ProviderUpdate {
    ProviderUpdate {
        quote: QuoteSnapshot::try_new(price, previous_close, "CNY", 1)
            .expect("valid quote")
            .with_identity(symbol, name)
            .with_updated_time("10:23:06"),
        connection: ConnectionState::Live,
    }
}

#[test]
fn builds_a_glanceable_snapshot_without_requerying_the_provider() {
    let config = AppConfig {
        stocks: vec![
            stock("600756.SH", "浪潮软件", Some(("100", "12"))),
            stock("000977.SZ", "浪潮信息", None),
        ],
        active_stock: 0,
        ..AppConfig::default()
    };

    let quotes = HashMap::from([
        (
            "600756.SH".into(),
            update("600756.SH", "浪潮软件", "14.82", "14.48"),
        ),
        (
            "000977.SZ".into(),
            update("000977.SZ", "浪潮信息", "52.16", "52.55"),
        ),
    ]);

    let snapshot = build_quick_view_snapshot(
        &config,
        &quotes,
        ConnectionState::Live,
        Some("10:23".into()),
        None,
    );

    assert_eq!(snapshot.rows.len(), 2);
    assert_eq!(snapshot.rows[0].short_name, "浪潮软件");
    assert_eq!(snapshot.rows[0].last_price, Some(Decimal::new(1482, 2)));
    assert_eq!(
        snapshot.rows[0]
            .daily_change_percent
            .expect("change should be available")
            .round_dp(2),
        Decimal::new(235, 2)
    );
    assert_eq!(snapshot.rows[0].position_profit, Some(Decimal::from(282)));
    assert_eq!(snapshot.rows[1].position_profit, None);
    assert_eq!(snapshot.totals.len(), 1);
    assert_eq!(snapshot.last_success_at.as_deref(), Some("10:23"));
    assert_eq!(snapshot.updated_at.as_deref(), Some("10:23:06"));
}

#[test]
fn keeps_missing_quotes_visible_and_does_not_invent_percentages() {
    let config = AppConfig {
        stocks: vec![
            stock("600756.SH", "浪潮软件", Some(("100", "12"))),
            stock("000977.SZ", "浪潮信息", None),
        ],
        active_stock: 0,
        ..AppConfig::default()
    };
    let quotes = HashMap::from([(
        "000977.SZ".into(),
        update("000977.SZ", "浪潮信息", "52.16", "0"),
    )]);

    let snapshot = build_quick_view_snapshot(
        &config,
        &quotes,
        ConnectionState::Disconnected,
        None,
        Some("网络连接失败".into()),
    );

    assert_eq!(snapshot.rows.len(), 2);
    assert_eq!(snapshot.rows[0].last_price, None);
    assert_eq!(snapshot.rows[1].daily_change_percent, None);
    assert_eq!(snapshot.connection, ConnectionState::Disconnected);
    assert_eq!(snapshot.missing_quotes, 1);
    assert_eq!(snapshot.last_error.as_deref(), Some("网络连接失败"));
}

#[test]
fn puts_positions_first_by_default_and_keeps_the_configured_order_within_each_group() {
    let config = AppConfig {
        stocks: vec![
            stock("000977.SZ", "浪潮信息", None),
            stock("600756.SH", "浪潮软件", Some(("100", "12"))),
            stock("000338.SZ", "潍柴动力", Some(("200", "20"))),
            stock("600875.SH", "东方电气", None),
        ],
        active_stock: 0,
        ..AppConfig::default()
    };

    let snapshot = build_quick_view_snapshot(
        &config,
        &HashMap::new(),
        ConnectionState::Connecting,
        None,
        None,
    );

    assert_eq!(
        snapshot
            .rows
            .iter()
            .map(|row| row.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["600756.SH", "000338.SZ", "000977.SZ", "600875.SH"]
    );
}

#[test]
fn can_follow_the_stock_settings_order_exactly() {
    let config = AppConfig {
        stocks: vec![
            stock("000977.SZ", "浪潮信息", None),
            stock("600756.SH", "浪潮软件", Some(("100", "12"))),
        ],
        active_stock: 0,
        quick_view_sort: QuickViewSort::StockOrder,
        ..AppConfig::default()
    };

    let snapshot = build_quick_view_snapshot(
        &config,
        &HashMap::new(),
        ConnectionState::Connecting,
        None,
        None,
    );

    assert_eq!(snapshot.rows[0].symbol, "000977.SZ");
    assert_eq!(snapshot.rows[1].symbol, "600756.SH");
}
