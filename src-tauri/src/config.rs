use serde::{Deserialize, Serialize};

use crate::{
    AlertMetric, AlertRule, CompactStyle, DisplayConfig, DisplayMetric, DomainError, MetricConfig,
    Position,
};

pub const CONFIG_SCHEMA_VERSION: u32 = 3;
/// 免费行情接口友好上限：批量查询一次带全部股票，菜单列表也保持可读。
pub const MAX_STOCKS: usize = 8;
/// 提醒规则上限：限制每轮刷新的评估开销，单机场景绰绰有余。
pub const MAX_ALERTS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Mock,
    Tencent,
    Longbridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayPreset {
    Price,
    PriceChange,
    Position,
}

/// 单只股票的配置：标的与本地持仓。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockConfig {
    pub symbol: String,
    pub short_name: String,
    pub currency: String,
    pub position: Option<Position>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub provider: ProviderKind,
    pub stocks: Vec<StockConfig>,
    /// 菜单栏当前置顶显示的股票下标。
    pub active_stock: usize,
    /// 提醒规则。v2 及更早的配置文件没有该字段，默认空列表。
    #[serde(default)]
    pub alerts: Vec<AlertRule>,
    pub display: DisplayConfig,
    pub launch_at_login: bool,
    pub tray_throttle_ms: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            provider: ProviderKind::Tencent,
            // 示例股票只带行情不带持仓：首启不该展示编造的盈亏数据。
            stocks: vec![StockConfig {
                symbol: "01810.HK".into(),
                short_name: "小米".into(),
                currency: "HKD".into(),
                position: None,
            }],
            active_stock: 0,
            alerts: Vec::new(),
            display: DisplayConfig {
                items: apply_display_preset(DisplayPreset::PriceChange),
                separator: " ".into(),
                // 「·收/·延」默认不展示：清楚市场状态的用户不需要，设置页可开。
                append_closed_status: false,
                append_delayed_status: false,
            },
            launch_at_login: false,
            tray_throttle_ms: 3_000,
        }
    }
}

impl AppConfig {
    pub fn active(&self) -> Option<&StockConfig> {
        self.stocks.get(self.active_stock)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.stocks.is_empty() {
            return Err(DomainError::EmptyStocks);
        }
        if self.stocks.len() > MAX_STOCKS {
            return Err(DomainError::TooManyStocks(self.stocks.len()));
        }
        if self.active_stock >= self.stocks.len() {
            return Err(DomainError::ActiveStockOutOfRange);
        }
        if self.display.items.is_empty() {
            return Err(DomainError::EmptyDisplayMetrics);
        }
        let mut seen = Vec::with_capacity(self.stocks.len());
        for stock in &self.stocks {
            let symbol = stock.symbol.trim().to_ascii_uppercase();
            if symbol.is_empty() {
                return Err(DomainError::SymbolRequired);
            }
            if seen.contains(&symbol) {
                return Err(DomainError::DuplicateSymbol(stock.symbol.clone()));
            }
            seen.push(symbol);
            // 反序列化不经过字符串解析入口，须在这里兜住越界数值，
            // 否则超大持仓在 Decimal 乘法时会 panic 并导致每次启动崩溃。
            if let Some(position) = &stock.position {
                crate::domain::ensure_supported_magnitude("quantity", position.quantity)?;
                crate::domain::ensure_supported_magnitude("average cost", position.average_cost)?;
            }
        }
        if self.alerts.len() > MAX_ALERTS {
            return Err(DomainError::TooManyAlerts(self.alerts.len()));
        }
        let mut alert_ids = Vec::with_capacity(self.alerts.len());
        for alert in &self.alerts {
            // 配置是用户可手编的明文 JSON：id 重复会让两条规则共享同一个
            // 穿越记忆槽位、互相污染判定，必须在这里兜住。
            let id = alert.id.trim();
            if id.is_empty() {
                return Err(DomainError::AlertIdRequired);
            }
            if alert_ids.contains(&id) {
                return Err(DomainError::DuplicateAlertId(alert.id.clone()));
            }
            alert_ids.push(id);

            let symbol = alert.symbol.trim().to_ascii_uppercase();
            let Some(stock) = self
                .stocks
                .iter()
                .find(|stock| stock.symbol.trim().to_ascii_uppercase() == symbol)
            else {
                return Err(DomainError::AlertSymbolUnknown(alert.symbol.clone()));
            };
            // 持仓类指标依赖持仓数据，否则规则永远不会触发（死规则）。
            if matches!(
                alert.metric,
                AlertMetric::PositionProfit | AlertMetric::PositionReturnPercent
            ) && stock.position.is_none()
            {
                return Err(DomainError::AlertPositionRequired(alert.symbol.clone()));
            }
            // 阈值允许为负（亏损/跌幅），只约束数量级，防 Decimal 溢出。
            crate::domain::ensure_bounded_magnitude("alert threshold", alert.threshold)?;
        }
        Ok(())
    }
}

/// v1 配置：单股票平铺字段。仅用于旧文件迁移。
/// 已废弃的字段（如 extendedHours）不再声明，serde 默认忽略未知字段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAppConfigV1 {
    provider: ProviderKind,
    symbol: String,
    short_name: String,
    currency: String,
    position: Option<Position>,
    display: DisplayConfig,
    launch_at_login: bool,
    tray_throttle_ms: u64,
}

impl From<LegacyAppConfigV1> for AppConfig {
    fn from(legacy: LegacyAppConfigV1) -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            provider: legacy.provider,
            stocks: vec![StockConfig {
                symbol: legacy.symbol,
                short_name: legacy.short_name,
                currency: legacy.currency,
                position: legacy.position,
            }],
            active_stock: 0,
            alerts: Vec::new(),
            display: legacy.display,
            launch_at_login: legacy.launch_at_login,
            tray_throttle_ms: legacy.tray_throttle_ms,
        }
    }
}

/// 解析配置 JSON：优先按当前 v2 结构，失败再按 v1 旧结构迁移。
/// 两者都失败时返回 v2 的解析错误（更贴近当前格式的诊断）。
pub(crate) fn parse_config_json(contents: &str) -> Result<AppConfig, serde_json::Error> {
    match serde_json::from_str::<AppConfig>(contents) {
        Ok(config) => Ok(config),
        Err(v2_error) => serde_json::from_str::<LegacyAppConfigV1>(contents)
            .map(AppConfig::from)
            .map_err(|_| v2_error),
    }
}

pub fn apply_display_preset(preset: DisplayPreset) -> Vec<MetricConfig> {
    match preset {
        DisplayPreset::Price => {
            vec![MetricConfig::new(DisplayMetric::LastPrice).with_precision(2)]
        }
        DisplayPreset::PriceChange => vec![
            MetricConfig::new(DisplayMetric::LastPrice).with_precision(2),
            MetricConfig::new(DisplayMetric::DailyChangePercent)
                .with_precision(2)
                .with_direction_arrow(),
        ],
        DisplayPreset::Position => vec![
            MetricConfig::new(DisplayMetric::PositionProfit)
                .with_precision(0)
                .with_sign()
                .with_compact_style(CompactStyle::None),
            MetricConfig::new(DisplayMetric::PositionReturnPercent)
                .with_precision(2)
                .with_direction_arrow(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_a_real_quote_provider_with_one_stock() {
        let config = AppConfig::default();
        assert_eq!(config.provider, ProviderKind::Tencent);
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.stocks.len(), 1);
        assert_eq!(
            config.active().map(|stock| stock.symbol.as_str()),
            Some("01810.HK")
        );
    }

    #[test]
    fn rejects_invalid_stock_lists() {
        let template = AppConfig::default().stocks[0].clone();

        let empty = AppConfig {
            stocks: Vec::new(),
            ..AppConfig::default()
        };
        assert_eq!(empty.validate(), Err(DomainError::EmptyStocks));

        let out_of_range = AppConfig {
            active_stock: 1,
            ..AppConfig::default()
        };
        assert_eq!(
            out_of_range.validate(),
            Err(DomainError::ActiveStockOutOfRange)
        );

        let duplicated = AppConfig {
            stocks: vec![template.clone(), template.clone()],
            ..AppConfig::default()
        };
        assert!(matches!(
            duplicated.validate(),
            Err(DomainError::DuplicateSymbol(_))
        ));

        let too_many = AppConfig {
            stocks: (0..=MAX_STOCKS)
                .map(|index| StockConfig {
                    symbol: format!("{index:06}.SH"),
                    ..template.clone()
                })
                .collect(),
            ..AppConfig::default()
        };
        assert!(matches!(
            too_many.validate(),
            Err(DomainError::TooManyStocks(_))
        ));
    }

    #[test]
    fn rejects_invalid_alert_lists() {
        use rust_decimal::Decimal;

        let alert = |id: &str, metric: AlertMetric| AlertRule {
            id: id.into(),
            symbol: "01810.HK".into(),
            metric,
            comparator: crate::AlertComparator::Above,
            threshold: Decimal::ONE,
            repeat: crate::AlertRepeat::DailyOnce,
            enabled: true,
            silent: false,
            custom_title: None,
            custom_body: None,
            last_triggered_day: None,
        };

        let duplicated = AppConfig {
            alerts: vec![
                alert("a", AlertMetric::Price),
                alert("a", AlertMetric::Price),
            ],
            ..AppConfig::default()
        };
        assert!(matches!(
            duplicated.validate(),
            Err(DomainError::DuplicateAlertId(_))
        ));

        let missing_id = AppConfig {
            alerts: vec![alert("  ", AlertMetric::Price)],
            ..AppConfig::default()
        };
        assert_eq!(missing_id.validate(), Err(DomainError::AlertIdRequired));

        let too_many = AppConfig {
            alerts: (0..=MAX_ALERTS)
                .map(|index| alert(&format!("id-{index}"), AlertMetric::Price))
                .collect(),
            ..AppConfig::default()
        };
        assert!(matches!(
            too_many.validate(),
            Err(DomainError::TooManyAlerts(_))
        ));

        // 持仓类指标要求该股票已配置持仓，否则是永远不会触发的死规则。
        let orphan = AppConfig {
            alerts: vec![alert("p", AlertMetric::PositionProfit)],
            ..AppConfig::default()
        };
        assert!(matches!(
            orphan.validate(),
            Err(DomainError::AlertPositionRequired(_))
        ));

        let mut with_position = AppConfig {
            alerts: vec![alert("p", AlertMetric::PositionProfit)],
            ..AppConfig::default()
        };
        with_position.stocks[0].position =
            Some(Position::try_new("100", "20").expect("fixture position"));
        with_position
            .validate()
            .expect("alert with position should be valid");
    }

    #[test]
    fn migrates_a_v1_config_file_into_the_stock_list() {
        let legacy = r#"{
            "schemaVersion": 1,
            "provider": "tencent",
            "symbol": "600756.SH",
            "shortName": "浪潮",
            "currency": "CNY",
            "position": { "quantity": "100", "averageCost": "20.5" },
            "display": {
                "items": [{
                    "metric": "lastPrice",
                    "precision": 2,
                    "showSign": false,
                    "directionArrow": false,
                    "compactStyle": "none",
                    "label": null
                }],
                "separator": " ",
                "appendClosedStatus": true,
                "appendDelayedStatus": true
            },
            "extendedHours": false,
            "launchAtLogin": true,
            "trayThrottleMs": 5000
        }"#;
        // extendedHours 已随「扩展时段」开关移除，旧文件里的残留字段应被忽略而非报错

        let config = parse_config_json(legacy).expect("legacy config should migrate");

        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.stocks.len(), 1);
        assert_eq!(config.stocks[0].symbol, "600756.SH");
        assert_eq!(config.stocks[0].short_name, "浪潮");
        assert!(config.stocks[0].position.is_some());
        assert_eq!(config.active_stock, 0);
        assert!(config.launch_at_login);
        assert_eq!(config.tray_throttle_ms, 5_000);
        config.validate().expect("migrated config should be valid");
    }

    #[test]
    fn ignores_the_retired_extended_hours_field_in_existing_config_files() {
        // 现役用户的 config.json 里都还带着 extendedHours，
        // 移除该字段后必须仍能正常读取，不能让人一升级就配置全丢。
        let stored = serde_json::to_value(AppConfig::default()).expect("config serializes");
        let mut with_retired_field = stored.clone();
        with_retired_field["extendedHours"] = serde_json::Value::Bool(true);

        let parsed = parse_config_json(&with_retired_field.to_string())
            .expect("retired fields must be ignored, not rejected");

        assert_eq!(parsed, AppConfig::default());
        // 重新写回时不再包含该字段
        assert!(stored.get("extendedHours").is_none());
    }

    #[test]
    fn keeps_reporting_errors_for_unparseable_config_files() {
        assert!(parse_config_json("{not-json").is_err());
        assert!(parse_config_json(r#"{"schemaVersion": 2}"#).is_err());
    }
}
