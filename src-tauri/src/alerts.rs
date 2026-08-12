//! 提醒规则：指标 + 条件 + 阈值 + 自定义通知内容。
//!
//! 触发采用穿越语义：上一轮不满足、本轮满足才触发，
//! 避免数值停留在阈值一侧时每轮刷新反复轰炸。

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset};

use crate::{ConnectionState, ProviderUpdate, StockConfig, calculate_position};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertMetric {
    /// 股价（最新价）
    Price,
    /// 今日涨跌幅（%，带符号）
    ChangePercent,
    /// 持仓收益额（带符号）
    PositionProfit,
    /// 持仓收益率（%，带符号）
    PositionReturnPercent,
}

impl AlertMetric {
    fn label(self) -> &'static str {
        match self {
            Self::Price => "股价",
            Self::ChangePercent => "今日涨跌幅",
            Self::PositionProfit => "持仓收益",
            Self::PositionReturnPercent => "持仓收益率",
        }
    }

    fn is_percent(self) -> bool {
        matches!(self, Self::ChangePercent | Self::PositionReturnPercent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertComparator {
    /// 达到或超过阈值（≥）
    Above,
    /// 达到或低于阈值（≤）
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertRepeat {
    /// 每个交易日最多触发一次，次日自动复位。
    DailyOnce,
    /// 触发一次后自动停用，需手动重新开启。
    Once,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRule {
    pub id: String,
    pub symbol: String,
    pub metric: AlertMetric,
    pub comparator: AlertComparator,
    pub threshold: Decimal,
    pub repeat: AlertRepeat,
    pub enabled: bool,
    /// 静默：不播放提示音，只弹横幅，适合不方便发出声音的场合。
    pub silent: bool,
    /// 自定义通知标题/正文；填写后完全替换默认文案（伪装用途）。
    pub custom_title: Option<String>,
    pub custom_body: Option<String>,
    /// 最近一次触发的交易日（UTC+8 日期），用于每日去重与重启防补发。
    pub last_triggered_day: Option<String>,
}

/// 一次触发要发出的通知。
///
/// 同时是推给设置窗口的事件载荷，故需可序列化。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertNotification {
    pub rule_id: String,
    pub title: String,
    pub body: String,
    pub silent: bool,
}

/// 规则语义指纹：symbol/metric/comparator/threshold 任一变化即视为不同规则。
/// 编辑规则后旧穿越记忆作废，按首次评估重新武装——否则会拿旧指标值
/// （可能连单位都不同）与新阈值比出一次假穿越，保存瞬间误发通知。
pub(crate) fn rule_fingerprint(rule: &AlertRule) -> String {
    format!(
        "{}|{:?}|{:?}|{}",
        rule.symbol.trim().to_ascii_uppercase(),
        rule.metric,
        rule.comparator,
        rule.threshold.normalize(),
    )
}

/// 穿越记忆条目：武装时的规则指纹 + 上一轮指标值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlertMemoryEntry {
    pub fingerprint: String,
    pub value: Decimal,
}

fn satisfied(comparator: AlertComparator, value: Decimal, threshold: Decimal) -> bool {
    match comparator {
        AlertComparator::Above => value >= threshold,
        AlertComparator::Below => value <= threshold,
    }
}

/// 从行情与持仓计算规则指标的当前值；算不出（无持仓、昨收为零）返回 None。
pub(crate) fn metric_value(
    metric: AlertMetric,
    stock: &StockConfig,
    update: &ProviderUpdate,
) -> Option<Decimal> {
    match metric {
        AlertMetric::Price => Some(update.quote.last_price),
        AlertMetric::ChangePercent => {
            if update.quote.previous_close.is_zero() {
                None
            } else {
                Some(
                    (update.quote.last_price - update.quote.previous_close)
                        / update.quote.previous_close
                        * Decimal::ONE_HUNDRED,
                )
            }
        }
        AlertMetric::PositionProfit => stock
            .position
            .as_ref()
            .and_then(|position| calculate_position(&update.quote, position).ok())
            .map(|pnl| pnl.unrealized_profit),
        AlertMetric::PositionReturnPercent => stock
            .position
            .as_ref()
            .and_then(|position| calculate_position(&update.quote, position).ok())
            .map(|pnl| pnl.return_percent),
    }
}

fn format_metric(metric: AlertMetric, value: Decimal) -> String {
    let mut rounded = value.round_dp(2);
    rounded.rescale(2);
    if metric.is_percent() {
        format!("{rounded}%")
    } else {
        rounded.to_string()
    }
}

fn default_notification(rule: &AlertRule, stock: &StockConfig, value: Decimal) -> (String, String) {
    let name = if stock.short_name.trim().is_empty() {
        stock.symbol.trim()
    } else {
        stock.short_name.trim()
    };
    let comparator_text = match rule.comparator {
        AlertComparator::Above => "≥",
        AlertComparator::Below => "≤",
    };
    (
        format!("{name} {}", format_metric(rule.metric, value)),
        format!(
            "{} 当前 {}，已{comparator_text} {}",
            rule.metric.label(),
            format_metric(rule.metric, value),
            format_metric(rule.metric, rule.threshold),
        ),
    )
}

/// 判定单条规则本轮是否触发。
///
/// 返回 (当前指标值, 触发的通知)。指标算不出时返回 (None, None)，
/// 且调用方应保留上一轮的记忆值不变（避免临时无值导致重臂误触发）。
///
/// 触发条件（全部满足）：
/// 1. 规则启用，且该股票市场状态为 交易中/延迟（休市陈旧数据不触发）；
/// 2. 上一轮有记忆值且不满足条件，本轮满足（穿越）；
///    首次评估只记忆不触发（建规则时已满足也不立刻响）；
/// 3. DailyOnce 规则当日未触发过。
pub(crate) fn evaluate_rule(
    rule: &AlertRule,
    stock: &StockConfig,
    update: &ProviderUpdate,
    previous: Option<Decimal>,
    today: &str,
) -> (Option<Decimal>, Option<AlertNotification>) {
    if !rule.enabled
        || !matches!(
            update.connection,
            ConnectionState::Live | ConnectionState::Delayed
        )
    {
        return (None, None);
    }
    let Some(current) = metric_value(rule.metric, stock, update) else {
        return (None, None);
    };

    let crossed = match previous {
        Some(previous_value) => {
            !satisfied(rule.comparator, previous_value, rule.threshold)
                && satisfied(rule.comparator, current, rule.threshold)
        }
        // 首次评估：只建立记忆，不触发。
        None => false,
    };
    let already_fired_today =
        rule.repeat == AlertRepeat::DailyOnce && rule.last_triggered_day.as_deref() == Some(today);

    if !crossed || already_fired_today {
        return (Some(current), None);
    }

    (
        Some(current),
        Some(build_notification(rule, stock, current)),
    )
}

/// 按规则组装通知：填了自定义文案就完全替换默认行情文案（伪装用途）。
///
/// 真实触发与「试发」共用这一个入口——试发若走另一套文案拼装，
/// 那测出来的就不是真正会收到的通知，等于没测。
pub(crate) fn build_notification(
    rule: &AlertRule,
    stock: &StockConfig,
    value: Decimal,
) -> AlertNotification {
    let (default_title, default_body) = default_notification(rule, stock, value);
    let title = rule
        .custom_title
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .unwrap_or(default_title);
    let body = rule
        .custom_body
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .unwrap_or(default_body);

    AlertNotification {
        rule_id: rule.id.clone(),
        title,
        body,
        silent: rule.silent,
    }
}

/// 触发后更新规则状态：DailyOnce 记录触发日，Once 直接停用。
pub(crate) fn mark_triggered(rule: &mut AlertRule, today: &str) {
    match rule.repeat {
        AlertRepeat::DailyOnce => rule.last_triggered_day = Some(today.to_owned()),
        AlertRepeat::Once => {
            rule.enabled = false;
            rule.last_triggered_day = Some(today.to_owned());
        }
    }
}

/// 交易所口径的当前日期（UTC+8），用于每日去重。
pub(crate) fn exchange_day(now_unix: i64) -> String {
    let offset = UtcOffset::from_hms(8, 0, 0).unwrap_or(UtcOffset::UTC);
    OffsetDateTime::from_unix_timestamp(now_unix)
        .map(|now| now.to_offset(offset).date().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, QuoteSnapshot};

    fn stock() -> StockConfig {
        StockConfig {
            symbol: "01810.HK".into(),
            short_name: "小米".into(),
            currency: "HKD".into(),
            position: Some(Position::try_new("250", "39.46").expect("fixture position")),
        }
    }

    fn update(last_price: &str, connection: ConnectionState) -> ProviderUpdate {
        ProviderUpdate {
            quote: QuoteSnapshot::try_new(last_price, "28.00", "HKD", 0).expect("fixture quote"),
            connection,
        }
    }

    fn rule(metric: AlertMetric, comparator: AlertComparator, threshold: &str) -> AlertRule {
        AlertRule {
            id: "rule-1".into(),
            symbol: "01810.HK".into(),
            metric,
            comparator,
            threshold: threshold.parse().expect("fixture threshold"),
            repeat: AlertRepeat::DailyOnce,
            enabled: true,
            silent: false,
            custom_title: None,
            custom_body: None,
            last_triggered_day: None,
        }
    }

    const TODAY: &str = "2026-08-05";

    #[test]
    fn fires_only_when_the_threshold_is_crossed() {
        let rule = rule(AlertMetric::Price, AlertComparator::Above, "30");
        let stock = stock();

        // 首次评估：即便已满足也只记忆不触发
        let (value, fired) = evaluate_rule(
            &rule,
            &stock,
            &update("31.00", ConnectionState::Live),
            None,
            TODAY,
        );
        assert_eq!(value, Some("31.00".parse().unwrap()));
        assert!(fired.is_none());

        // 上一轮不满足 → 本轮满足：触发
        let (_, fired) = evaluate_rule(
            &rule,
            &stock,
            &update("30.10", ConnectionState::Live),
            Some("29.50".parse().unwrap()),
            TODAY,
        );
        let notification = fired.expect("crossing must fire");
        assert_eq!(notification.title, "小米 30.10");
        assert_eq!(notification.body, "股价 当前 30.10，已≥ 30.00");

        // 持续满足：不再触发
        let (_, fired) = evaluate_rule(
            &rule,
            &stock,
            &update("30.20", ConnectionState::Live),
            Some("30.10".parse().unwrap()),
            TODAY,
        );
        assert!(fired.is_none());
    }

    #[test]
    fn respects_daily_dedup_and_market_state() {
        let mut daily = rule(AlertMetric::Price, AlertComparator::Above, "30");
        daily.last_triggered_day = Some(TODAY.into());
        let stock = stock();

        // 当日已触发过：静默
        let (_, fired) = evaluate_rule(
            &daily,
            &stock,
            &update("30.10", ConnectionState::Live),
            Some("29.00".parse().unwrap()),
            TODAY,
        );
        assert!(fired.is_none());

        // 新交易日自动复位
        let (_, fired) = evaluate_rule(
            &daily,
            &stock,
            &update("30.10", ConnectionState::Live),
            Some("29.00".parse().unwrap()),
            "2026-08-06",
        );
        assert!(fired.is_some());

        // 休市数据不触发也不更新记忆
        let (value, fired) = evaluate_rule(
            &daily,
            &stock,
            &update("30.10", ConnectionState::Closed),
            Some("29.00".parse().unwrap()),
            "2026-08-06",
        );
        assert!(value.is_none());
        assert!(fired.is_none());
    }

    #[test]
    fn supports_percent_and_position_metrics_with_signed_thresholds() {
        let stock = stock();
        // 跌幅超 3%：changePercent ≤ -3
        let drop_rule = rule(AlertMetric::ChangePercent, AlertComparator::Below, "-3");
        let (_, fired) = evaluate_rule(
            &drop_rule,
            &stock,
            &update("27.00", ConnectionState::Live), // (27-28)/28 = -3.57%
            Some("-2.5".parse().unwrap()),
            TODAY,
        );
        let notification = fired.expect("drop must fire");
        assert_eq!(notification.body, "今日涨跌幅 当前 -3.57%，已≤ -3.00%");

        // 亏损超 2000：positionProfit ≤ -2000（250 股 @39.46，价 27 → 亏 3115）
        let loss_rule = rule(AlertMetric::PositionProfit, AlertComparator::Below, "-2000");
        let (value, fired) = evaluate_rule(
            &loss_rule,
            &stock,
            &update("27.00", ConnectionState::Live),
            Some("-1500".parse().unwrap()),
            TODAY,
        );
        assert_eq!(value, Some("-3115.00".parse().unwrap()));
        assert!(fired.is_some());

        // 无持仓时持仓类指标不评估
        let no_position = StockConfig {
            position: None,
            ..stock
        };
        let (value, fired) = evaluate_rule(
            &loss_rule,
            &no_position,
            &update("27.00", ConnectionState::Live),
            Some("-1500".parse().unwrap()),
            TODAY,
        );
        assert!(value.is_none());
        assert!(fired.is_none());
    }

    #[test]
    fn custom_text_fully_replaces_the_default_notification() {
        let mut disguised = rule(AlertMetric::ChangePercent, AlertComparator::Above, "3");
        disguised.custom_title = Some("今天吃了三斤肉".into());
        disguised.custom_body = Some("记得晚上散步".into());
        disguised.silent = true;

        let (_, fired) = evaluate_rule(
            &disguised,
            &stock(),
            &update("29.00", ConnectionState::Live), // +3.57%
            Some("2".parse().unwrap()),
            TODAY,
        );
        let notification = fired.expect("crossing must fire");
        assert_eq!(notification.title, "今天吃了三斤肉");
        assert_eq!(notification.body, "记得晚上散步");
        assert!(notification.silent);
        assert!(!notification.title.contains("小米"));
        assert!(!notification.body.contains('%'));
    }

    #[test]
    fn once_rules_disable_themselves_after_firing() {
        let mut one_shot = rule(AlertMetric::Price, AlertComparator::Above, "30");
        one_shot.repeat = AlertRepeat::Once;

        mark_triggered(&mut one_shot, TODAY);
        assert!(!one_shot.enabled);
        assert_eq!(one_shot.last_triggered_day.as_deref(), Some(TODAY));

        let mut daily = rule(AlertMetric::Price, AlertComparator::Above, "30");
        mark_triggered(&mut daily, TODAY);
        assert!(daily.enabled);
        assert_eq!(daily.last_triggered_day.as_deref(), Some(TODAY));
    }

    #[test]
    fn editing_the_rule_semantics_changes_its_crossing_fingerprint() {
        let original = rule(AlertMetric::Price, AlertComparator::Above, "30");
        // 阈值写法不同（30 vs 30.00）不算编辑
        let same = rule(AlertMetric::Price, AlertComparator::Above, "30.00");
        assert_eq!(rule_fingerprint(&original), rule_fingerprint(&same));

        // 指标/条件/阈值任一变化都要重新武装
        let edited = rule(AlertMetric::ChangePercent, AlertComparator::Below, "-3");
        assert_ne!(rule_fingerprint(&original), rule_fingerprint(&edited));
        let new_threshold = rule(AlertMetric::Price, AlertComparator::Above, "35");
        assert_ne!(
            rule_fingerprint(&original),
            rule_fingerprint(&new_threshold)
        );

        // 文案/静默等展示属性变化不影响穿越记忆
        let mut disguised = rule(AlertMetric::Price, AlertComparator::Above, "30");
        disguised.custom_title = Some("今天吃了三斤肉".into());
        disguised.silent = true;
        assert_eq!(rule_fingerprint(&original), rule_fingerprint(&disguised));
    }

    #[test]
    fn exchange_day_uses_utc8() {
        // 2026-08-05 23:30 UTC = 2026-08-06 07:30 UTC+8
        assert_eq!(exchange_day(1_785_972_600), "2026-08-06");
    }
}
