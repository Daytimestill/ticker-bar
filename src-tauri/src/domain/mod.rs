use std::{error::Error, fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteSnapshot {
    pub symbol: Option<String>,
    pub short_name: Option<String>,
    pub last_price: Decimal,
    pub previous_close: Decimal,
    pub day_high: Option<Decimal>,
    pub day_low: Option<Decimal>,
    pub currency: String,
    pub timestamp: i64,
    pub updated_time: Option<String>,
}

impl QuoteSnapshot {
    pub fn try_new(
        last_price: &str,
        previous_close: &str,
        currency: impl Into<String>,
        timestamp: i64,
    ) -> Result<Self, DomainError> {
        let last_price = parse_non_negative_decimal("last price", last_price)?;
        let previous_close = parse_non_negative_decimal("previous close", previous_close)?;

        Ok(Self {
            symbol: None,
            short_name: None,
            last_price,
            previous_close,
            day_high: None,
            day_low: None,
            currency: currency.into(),
            timestamp,
            updated_time: None,
        })
    }

    pub fn with_identity(
        mut self,
        symbol: impl Into<String>,
        short_name: impl Into<String>,
    ) -> Self {
        self.symbol = Some(symbol.into());
        self.short_name = Some(short_name.into());
        self
    }

    pub fn with_day_range(mut self, high: &str, low: &str) -> Result<Self, DomainError> {
        self.day_high = Some(parse_non_negative_decimal("day high", high)?);
        self.day_low = Some(parse_non_negative_decimal("day low", low)?);
        Ok(self)
    }

    pub fn with_updated_time(mut self, updated_time: impl Into<String>) -> Self {
        self.updated_time = Some(updated_time.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub quantity: Decimal,
    pub average_cost: Decimal,
}

impl Position {
    pub fn try_new(quantity: &str, average_cost: &str) -> Result<Self, DomainError> {
        let quantity = parse_non_negative_decimal("quantity", quantity)?;
        let average_cost = parse_non_negative_decimal("average cost", average_cost)?;

        Ok(Self {
            quantity,
            average_cost,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionPnl {
    pub market_value: Decimal,
    pub cost_basis: Decimal,
    pub unrealized_profit: Decimal,
    pub return_percent: Decimal,
}

/// 上限约束：所有外部输入的数值不超过 1 万亿，保证后续任何
/// 乘法/百分比运算都远离 Decimal 溢出 panic 区间（约 7.9e28）。
fn max_supported_value() -> Decimal {
    Decimal::from(1_000_000_000_000_u64)
}

pub(crate) fn ensure_supported_magnitude(
    name: &'static str,
    value: Decimal,
) -> Result<(), DomainError> {
    if value.is_sign_negative() {
        return Err(DomainError::NegativeDecimal {
            name,
            value: value.to_string(),
        });
    }
    ensure_bounded_magnitude(name, value)
}

/// 只约束数量级、允许负值（亏损阈值、跌幅阈值等场景）。
pub(crate) fn ensure_bounded_magnitude(
    name: &'static str,
    value: Decimal,
) -> Result<(), DomainError> {
    if value.abs() > max_supported_value() {
        return Err(DomainError::ValueTooLarge {
            name,
            value: value.to_string(),
        });
    }
    Ok(())
}

pub fn calculate_position(
    quote: &QuoteSnapshot,
    position: &Position,
) -> Result<PositionPnl, DomainError> {
    let cost_basis = position.average_cost * position.quantity;
    if cost_basis.is_zero() {
        return Err(DomainError::ZeroCostBasis);
    }

    let market_value = quote.last_price * position.quantity;
    let unrealized_profit = market_value - cost_basis;
    let return_percent = unrealized_profit / cost_basis * Decimal::ONE_HUNDRED;

    Ok(PositionPnl {
        market_value,
        cost_basis,
        unrealized_profit,
        return_percent,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayMetric {
    Symbol,
    ShortName,
    LastPrice,
    DailyChange,
    DailyChangePercent,
    PreviousClose,
    DayHigh,
    DayLow,
    PositionProfit,
    PositionReturnPercent,
    MarketValue,
    AverageCost,
    Quantity,
    ProfitPerShare,
    MarketStatus,
    UpdatedTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompactStyle {
    None,
    Western,
    Chinese,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricConfig {
    pub metric: DisplayMetric,
    pub precision: u32,
    pub show_sign: bool,
    pub direction_arrow: bool,
    pub compact_style: CompactStyle,
    pub label: Option<String>,
}

impl MetricConfig {
    pub fn new(metric: DisplayMetric) -> Self {
        Self {
            metric,
            precision: 2,
            show_sign: false,
            direction_arrow: false,
            compact_style: CompactStyle::None,
            label: None,
        }
    }

    pub fn with_precision(mut self, precision: u32) -> Self {
        self.precision = precision;
        self
    }

    pub fn with_sign(mut self) -> Self {
        self.show_sign = true;
        self
    }

    pub fn with_direction_arrow(mut self) -> Self {
        self.direction_arrow = true;
        self
    }

    pub fn with_compact_style(mut self, style: CompactStyle) -> Self {
        self.compact_style = style;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayConfig {
    pub items: Vec<MetricConfig>,
    pub separator: String,
    pub append_closed_status: bool,
    pub append_delayed_status: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    Connecting,
    Live,
    Delayed,
    Closed,
    Reconnecting,
    Disconnected,
    AuthenticationFailed,
}

pub fn render_tray_title(
    quote: &QuoteSnapshot,
    position: Option<&Position>,
    display: &DisplayConfig,
    connection: &ConnectionState,
) -> Result<String, DomainError> {
    if display.items.is_empty() {
        return Err(DomainError::EmptyDisplayMetrics);
    }

    if matches!(connection, ConnectionState::AuthenticationFailed) {
        return Ok("行情异常".into());
    }

    // 持仓成本为零等个别指标不可算时只跳过该指标，不让整条标题渲染失败。
    let pnl =
        position.and_then(|current_position| calculate_position(quote, current_position).ok());
    let mut parts = Vec::with_capacity(display.items.len());

    for item in &display.items {
        let value = match item.metric {
            DisplayMetric::Symbol => quote
                .symbol
                .as_deref()
                .map(|value| apply_label(value.to_owned(), item)),
            DisplayMetric::ShortName => quote
                .short_name
                .as_deref()
                .map(|value| apply_label(value.to_owned(), item)),
            DisplayMetric::LastPrice => {
                Some(format_value(quote.last_price, item, ValueKind::Number))
            }
            DisplayMetric::DailyChange => {
                let change = quote.last_price - quote.previous_close;
                Some(format_value(change, item, ValueKind::Number))
            }
            DisplayMetric::DailyChangePercent => {
                // 新股/停牌可能出现昨收为零，此时跳过百分比而非整体失败。
                if quote.previous_close.is_zero() {
                    None
                } else {
                    let change_percent = (quote.last_price - quote.previous_close)
                        / quote.previous_close
                        * Decimal::ONE_HUNDRED;
                    Some(format_value(change_percent, item, ValueKind::Percent))
                }
            }
            DisplayMetric::PreviousClose => {
                Some(format_value(quote.previous_close, item, ValueKind::Number))
            }
            DisplayMetric::DayHigh => quote
                .day_high
                .map(|value| format_value(value, item, ValueKind::Number)),
            DisplayMetric::DayLow => quote
                .day_low
                .map(|value| format_value(value, item, ValueKind::Number)),
            DisplayMetric::PositionProfit => pnl
                .as_ref()
                .map(|value| format_value(value.unrealized_profit, item, ValueKind::Number)),
            DisplayMetric::PositionReturnPercent => pnl
                .as_ref()
                .map(|value| format_value(value.return_percent, item, ValueKind::Percent)),
            DisplayMetric::MarketValue => pnl
                .as_ref()
                .map(|value| format_value(value.market_value, item, ValueKind::Number)),
            DisplayMetric::AverageCost => {
                position.map(|value| format_value(value.average_cost, item, ValueKind::Number))
            }
            DisplayMetric::Quantity => {
                position.map(|value| format_value(value.quantity, item, ValueKind::Number))
            }
            DisplayMetric::ProfitPerShare => position.map(|value| {
                format_value(
                    quote.last_price - value.average_cost,
                    item,
                    ValueKind::Number,
                )
            }),
            DisplayMetric::MarketStatus => {
                Some(apply_label(market_status_text(connection).into(), item))
            }
            DisplayMetric::UpdatedTime => quote
                .updated_time
                .as_deref()
                .map(|value| apply_label(value.to_owned(), item)),
        };

        if let Some(value) = value {
            parts.push(value);
        }
    }

    if parts.is_empty() {
        return Err(DomainError::NoRenderableMetrics);
    }

    let mut title = parts.join(&display.separator);
    let contains_market_status = display
        .items
        .iter()
        .any(|item| item.metric == DisplayMetric::MarketStatus);
    let suffix = match connection {
        ConnectionState::Connecting | ConnectionState::Reconnecting => Some("↻"),
        ConnectionState::Disconnected => Some("!"),
        ConnectionState::Delayed if display.append_delayed_status && !contains_market_status => {
            Some("·延")
        }
        ConnectionState::Closed if display.append_closed_status && !contains_market_status => {
            Some("·收")
        }
        ConnectionState::Live
        | ConnectionState::Delayed
        | ConnectionState::Closed
        | ConnectionState::AuthenticationFailed => None,
    };

    if let Some(suffix) = suffix {
        title.push(' ');
        title.push_str(suffix);
    }

    Ok(title)
}

#[derive(Debug, Clone, Copy)]
enum ValueKind {
    Number,
    Percent,
}

fn format_value(value: Decimal, config: &MetricConfig, kind: ValueKind) -> String {
    let (scaled, compact_suffix) = compact_value(value, config.compact_style);
    let rounded = normalize_zero(scaled.round_dp(config.precision));
    let number = fixed_decimal(rounded.abs(), config.precision);

    let prefix = if config.direction_arrow {
        if rounded.is_sign_positive() && !rounded.is_zero() {
            "↑"
        } else if rounded.is_sign_negative() {
            "↓"
        } else {
            ""
        }
    } else if rounded.is_sign_negative() {
        "-"
    } else if config.show_sign && !rounded.is_zero() {
        "+"
    } else {
        ""
    };

    let suffix = match kind {
        ValueKind::Number => compact_suffix,
        ValueKind::Percent => "%",
    };

    apply_label(format!("{prefix}{number}{suffix}"), config)
}

fn compact_value(value: Decimal, style: CompactStyle) -> (Decimal, &'static str) {
    let absolute = value.abs();
    match style {
        CompactStyle::None => (value, ""),
        CompactStyle::Chinese if absolute >= Decimal::from(100_000_000_u64) => {
            (value / Decimal::from(100_000_000_u64), "亿")
        }
        CompactStyle::Chinese if absolute >= Decimal::from(10_000_u64) => {
            (value / Decimal::from(10_000_u64), "万")
        }
        CompactStyle::Western if absolute >= Decimal::from(1_000_000_u64) => {
            (value / Decimal::from(1_000_000_u64), "M")
        }
        CompactStyle::Western if absolute >= Decimal::from(1_000_u64) => {
            (value / Decimal::from(1_000_u64), "K")
        }
        CompactStyle::Chinese | CompactStyle::Western => (value, ""),
    }
}

fn apply_label(value: String, config: &MetricConfig) -> String {
    match &config.label {
        Some(label) => format!("{label}{value}"),
        None => value,
    }
}

fn market_status_text(connection: &ConnectionState) -> &'static str {
    match connection {
        ConnectionState::Connecting | ConnectionState::Reconnecting => "连",
        ConnectionState::Live => "交易中",
        ConnectionState::Delayed => "延",
        ConnectionState::Closed => "收",
        ConnectionState::Disconnected => "断",
        ConnectionState::AuthenticationFailed => "异常",
    }
}

fn fixed_decimal(value: Decimal, precision: u32) -> String {
    let mut text = value.to_string();
    if precision == 0 {
        return text
            .split_once('.')
            .map_or(text.clone(), |(whole, _)| whole.to_owned());
    }

    if !text.contains('.') {
        text.push('.');
    }

    let fractional_digits = text
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    for _ in fractional_digits..precision as usize {
        text.push('0');
    }
    text
}

fn normalize_zero(value: Decimal) -> Decimal {
    if value.is_zero() {
        Decimal::ZERO
    } else {
        value
    }
}

fn parse_non_negative_decimal(name: &'static str, value: &str) -> Result<Decimal, DomainError> {
    let parsed = Decimal::from_str(value).map_err(|_| DomainError::InvalidDecimal {
        name,
        value: value.into(),
    })?;
    ensure_supported_magnitude(name, parsed)?;
    Ok(parsed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidDecimal { name: &'static str, value: String },
    NegativeDecimal { name: &'static str, value: String },
    ValueTooLarge { name: &'static str, value: String },
    ZeroCostBasis,
    ZeroPreviousClose,
    SymbolRequired,
    EmptyStocks,
    TooManyStocks(usize),
    DuplicateSymbol(String),
    ActiveStockOutOfRange,
    AlertSymbolUnknown(String),
    AlertIdRequired,
    DuplicateAlertId(String),
    TooManyAlerts(usize),
    AlertPositionRequired(String),
    EmptyDisplayMetrics,
    NoRenderableMetrics,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDecimal { name, value } => {
                write!(formatter, "{name} is not a valid decimal: {value}")
            }
            Self::NegativeDecimal { name, value } => {
                write!(formatter, "{name} cannot be negative: {value}")
            }
            Self::ValueTooLarge { name, value } => {
                write!(formatter, "{name} is unreasonably large: {value}")
            }
            Self::ZeroCostBasis => write!(formatter, "cost basis must be greater than zero"),
            Self::ZeroPreviousClose => {
                write!(formatter, "previous close must be greater than zero")
            }
            Self::SymbolRequired => write!(formatter, "symbol is required"),
            Self::EmptyStocks => write!(formatter, "at least one stock is required"),
            Self::TooManyStocks(count) => {
                write!(formatter, "at most 8 stocks are supported, got {count}")
            }
            Self::DuplicateSymbol(symbol) => {
                write!(formatter, "duplicate stock symbol: {symbol}")
            }
            Self::ActiveStockOutOfRange => {
                write!(formatter, "active stock index is out of range")
            }
            Self::AlertSymbolUnknown(symbol) => {
                write!(formatter, "alert references an unknown stock: {symbol}")
            }
            Self::AlertIdRequired => write!(formatter, "alert id is required"),
            Self::DuplicateAlertId(id) => {
                write!(formatter, "duplicate alert id: {id}")
            }
            Self::TooManyAlerts(count) => {
                write!(formatter, "at most 20 alerts are supported, got {count}")
            }
            Self::AlertPositionRequired(symbol) => {
                write!(
                    formatter,
                    "alert metric requires a position on stock: {symbol}"
                )
            }
            Self::EmptyDisplayMetrics => {
                write!(formatter, "at least one display metric is required")
            }
            Self::NoRenderableMetrics => write!(formatter, "no selected metric has a value"),
        }
    }
}

impl Error for DomainError {}
