use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::Serialize;

use crate::{
    AppConfig, ConnectionState, PortfolioTotal, ProviderUpdate, QuickViewSort, StockConfig,
    calculate_position, summarize_portfolio,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickViewRow {
    pub symbol: String,
    pub short_name: String,
    pub currency: String,
    pub last_price: Option<Decimal>,
    pub daily_change_percent: Option<Decimal>,
    pub position_profit: Option<Decimal>,
    pub position_return_percent: Option<Decimal>,
    pub connection: ConnectionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickViewSnapshot {
    pub shortcut: String,
    pub connection: ConnectionState,
    pub rows: Vec<QuickViewRow>,
    pub totals: Vec<PortfolioTotal>,
    pub missing_quotes: usize,
    pub updated_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

fn change_percent(update: &ProviderUpdate) -> Option<Decimal> {
    let previous = update.quote.previous_close;
    if previous.is_zero() {
        None
    } else {
        Some((update.quote.last_price - previous) / previous * Decimal::ONE_HUNDRED)
    }
}

/// 把已经缓存的行情整理成速览面板数据。这里绝不触发网络请求：快捷键应该立即响应，
/// 行情的新鲜度仍由现有刷新循环负责。
pub fn build_quick_view_snapshot(
    config: &AppConfig,
    quotes: &HashMap<String, ProviderUpdate>,
    transport: ConnectionState,
    last_success_at: Option<String>,
    last_error: Option<String>,
) -> QuickViewSnapshot {
    let portfolio = summarize_portfolio(&config.stocks, |symbol| {
        quotes.get(symbol).map(|update| &update.quote)
    });

    let mut ordered_stocks: Vec<&StockConfig> = config.stocks.iter().collect();
    if config.quick_view_sort == QuickViewSort::PositionFirst {
        // 稳定排序：只把有持仓的整体提到前面，组内仍严格保留股票页的自定义顺序。
        ordered_stocks.sort_by_key(|stock| stock.position.is_none());
    }

    let rows = ordered_stocks
        .into_iter()
        .map(|stock| {
            let update = quotes.get(stock.symbol.trim());
            let position = update.and_then(|update| {
                stock
                    .position
                    .as_ref()
                    .and_then(|position| calculate_position(&update.quote, position).ok())
            });
            QuickViewRow {
                symbol: stock.symbol.trim().to_owned(),
                short_name: if stock.short_name.trim().is_empty() {
                    stock.symbol.trim().to_owned()
                } else {
                    stock.short_name.trim().to_owned()
                },
                currency: stock.currency.trim().to_ascii_uppercase(),
                last_price: update.map(|value| value.quote.last_price),
                daily_change_percent: update.and_then(change_percent),
                position_profit: position.as_ref().map(|value| value.unrealized_profit),
                position_return_percent: position.map(|value| value.return_percent),
                connection: match transport {
                    ConnectionState::Live | ConnectionState::Delayed | ConnectionState::Closed => {
                        update.map_or(transport, |value| value.connection)
                    }
                    _ => transport,
                },
            }
        })
        .collect();

    let updated_at = config
        .stocks
        .iter()
        .filter_map(|stock| quotes.get(stock.symbol.trim()))
        .filter_map(|update| update.quote.updated_time.as_deref())
        .max()
        .map(str::to_owned);

    QuickViewSnapshot {
        shortcut: config.quick_view_shortcut.clone(),
        connection: transport,
        rows,
        totals: portfolio.totals,
        missing_quotes: portfolio.missing_quotes,
        updated_at,
        last_success_at,
        last_error,
    }
}
