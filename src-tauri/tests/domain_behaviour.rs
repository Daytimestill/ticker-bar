use tickerbar_core::{
    CompactStyle, ConnectionState, DisplayConfig, DisplayMetric, MetricConfig, Position,
    QuoteSnapshot, calculate_position, render_tray_title,
};

fn quote() -> QuoteSnapshot {
    QuoteSnapshot::try_new("42.85", "42.20", "HKD", 1_722_412_338)
        .expect("fixture quote should be valid")
}

fn position() -> Position {
    Position::try_new("250", "39.46").expect("fixture position should be valid")
}

#[test]
fn calculates_unrealized_profit_and_return_with_decimal_precision() {
    let pnl = calculate_position(&quote(), &position()).expect("position should be calculable");

    assert_eq!(pnl.market_value.to_string(), "10712.50");
    assert_eq!(pnl.unrealized_profit.to_string(), "847.50");
    assert_eq!(pnl.return_percent.round_dp(2).to_string(), "8.59");
}

#[test]
fn renders_selected_metrics_in_configured_order() {
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
            MetricConfig::new(DisplayMetric::DailyChangePercent)
                .with_precision(2)
                .with_direction_arrow(),
            MetricConfig::new(DisplayMetric::PositionProfit)
                .with_precision(0)
                .with_sign(),
        ],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(
        &quote(),
        Some(&position()),
        &display,
        &ConnectionState::Live,
    )
    .expect("title should render");

    assert_eq!(title, "42.85 ↑1.54% +848");
}

#[test]
fn preserves_user_order_for_profit_first_layout() {
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::PositionProfit)
                .with_precision(0)
                .with_sign(),
            MetricConfig::new(DisplayMetric::PositionReturnPercent)
                .with_precision(2)
                .with_direction_arrow(),
        ],
        separator: " · ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(
        &quote(),
        Some(&position()),
        &display,
        &ConnectionState::Live,
    )
    .expect("title should render");

    assert_eq!(title, "+848 · ↑8.59%");
}

#[test]
fn hides_position_metrics_when_no_position_is_configured() {
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
            MetricConfig::new(DisplayMetric::PositionReturnPercent).with_precision(2),
        ],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(&quote(), None, &display, &ConnectionState::Live)
        .expect("price should still render");

    assert_eq!(title, "42.85");
}

#[test]
fn always_marks_disconnected_quotes_even_when_status_fields_are_disabled() {
    let display = DisplayConfig {
        items: vec![MetricConfig::new(DisplayMetric::LastPrice).with_precision(2)],
        separator: " ".into(),
        append_closed_status: false,
        append_delayed_status: false,
    };

    let title = render_tray_title(
        &quote(),
        Some(&position()),
        &display,
        &ConnectionState::Disconnected,
    )
    .expect("cached title should render");

    assert_eq!(title, "42.85 !");
}

#[test]
fn rejects_an_empty_metric_selection() {
    let display = DisplayConfig {
        items: vec![],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let error = render_tray_title(&quote(), None, &display, &ConnectionState::Live)
        .expect_err("empty selection must be rejected");

    assert_eq!(error.to_string(), "at least one display metric is required");
}

#[test]
fn renders_identity_day_range_and_update_time_in_user_order() {
    let quote = quote()
        .with_identity("01810.HK", "小米")
        .with_day_range("43.10", "41.90")
        .expect("day range should be valid")
        .with_updated_time("14:32");
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::ShortName),
            MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
            MetricConfig::new(DisplayMetric::DayHigh)
                .with_precision(2)
                .with_label("高"),
            MetricConfig::new(DisplayMetric::DayLow)
                .with_precision(2)
                .with_label("低"),
            MetricConfig::new(DisplayMetric::UpdatedTime),
        ],
        separator: " · ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(&quote, None, &display, &ConnectionState::Live)
        .expect("title should render");

    assert_eq!(title, "小米 · 42.85 · 高43.10 · 低41.90 · 14:32");
}

#[test]
fn supports_all_position_metrics() {
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::MarketValue).with_precision(2),
            MetricConfig::new(DisplayMetric::AverageCost).with_precision(2),
            MetricConfig::new(DisplayMetric::Quantity).with_precision(0),
            MetricConfig::new(DisplayMetric::ProfitPerShare)
                .with_precision(2)
                .with_sign(),
        ],
        separator: " | ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(
        &quote(),
        Some(&position()),
        &display,
        &ConnectionState::Live,
    )
    .expect("title should render");

    assert_eq!(title, "10712.50 | 39.46 | 250 | +3.39");
}

#[test]
fn supports_chinese_compact_numbers_and_short_labels() {
    let large_position =
        Position::try_new("25000", "39.46").expect("fixture position should be valid");
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::PositionProfit)
                .with_precision(2)
                .with_sign()
                .with_compact_style(CompactStyle::Chinese)
                .with_label("盈"),
        ],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(
        &quote(),
        Some(&large_position),
        &display,
        &ConnectionState::Live,
    )
    .expect("title should render");

    assert_eq!(title, "盈+8.48万");
}

#[test]
fn renders_market_state_as_a_selectable_metric() {
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
            MetricConfig::new(DisplayMetric::MarketStatus),
        ],
        separator: " · ".into(),
        append_closed_status: false,
        append_delayed_status: false,
    };

    let title = render_tray_title(&quote(), None, &display, &ConnectionState::Closed)
        .expect("title should render");

    assert_eq!(title, "42.85 · 收");
}

#[test]
fn never_renders_negative_zero() {
    let unchanged_quote = QuoteSnapshot::try_new("42.20", "42.20", "HKD", 1_722_412_338)
        .expect("fixture quote should be valid");
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::DailyChangePercent)
                .with_precision(2)
                .with_direction_arrow(),
        ],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };

    let title = render_tray_title(&unchanged_quote, None, &display, &ConnectionState::Live)
        .expect("title should render");

    assert_eq!(title, "0.00%");
}

#[test]
fn validates_decimal_input_and_magnitude_bounds() {
    assert_eq!(
        Position::try_new("-1", "39.46")
            .expect_err("negative quantity must fail")
            .to_string(),
        "quantity cannot be negative: -1"
    );
    // 超过 1 万亿的输入直接拒绝，防止后续 Decimal 乘法溢出 panic。
    assert_eq!(
        Position::try_new("1000000000001", "39.46")
            .expect_err("oversized quantity must fail")
            .to_string(),
        "quantity is unreasonably large: 1000000000001"
    );
    assert!(QuoteSnapshot::try_new("9999999999999", "42.20", "HKD", 1_722_412_338).is_err());
}

#[test]
fn skips_unrenderable_metrics_instead_of_failing_the_whole_title() {
    // 新股/停牌昨收为零：跳过涨跌幅，其余指标照常渲染。
    let zero_close = QuoteSnapshot::try_new("42.85", "0", "HKD", 1_722_412_338)
        .expect("zero close is a valid quote");
    let display = DisplayConfig {
        items: vec![
            MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
            MetricConfig::new(DisplayMetric::DailyChangePercent),
        ],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };
    let title = render_tray_title(&zero_close, None, &display, &ConnectionState::Live)
        .expect("price must render even when the percent metric is unavailable");
    assert_eq!(title, "42.85");

    // 持仓成本为零：持仓收益类指标跳过，价格照常渲染。
    let zero_cost = Position::try_new("100", "0").expect("zero cost is accepted as input");
    let title = render_tray_title(
        &QuoteSnapshot::try_new("42.85", "42.20", "HKD", 1_722_412_338).expect("valid quote"),
        Some(&zero_cost),
        &DisplayConfig {
            items: vec![
                MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
                MetricConfig::new(DisplayMetric::PositionProfit).with_precision(0),
            ],
            separator: " ".into(),
            append_closed_status: true,
            append_delayed_status: true,
        },
        &ConnectionState::Live,
    )
    .expect("price must render even when position pnl is unavailable");
    assert_eq!(title, "42.85");

    // 所有选中指标都不可渲染时仍然明确报错，而不是显示空标题。
    let percent_only = DisplayConfig {
        items: vec![MetricConfig::new(DisplayMetric::DailyChangePercent)],
        separator: " ".into(),
        append_closed_status: true,
        append_delayed_status: true,
    };
    assert_eq!(
        render_tray_title(&zero_close, None, &percent_only, &ConnectionState::Live)
            .expect_err("an all-skipped selection must surface an error")
            .to_string(),
        "no selected metric has a value"
    );
}
