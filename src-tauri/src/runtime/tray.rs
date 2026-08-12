use std::sync::Mutex;

use rust_decimal::Decimal;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    ConnectionState, PortfolioTotal, ProviderUpdate, StockConfig, render_tray_title, save_config,
    summarize_portfolio,
};

use super::{
    RefreshStatus, RuntimeState, SETTINGS_WINDOW_HEIGHT, SETTINGS_WINDOW_LABEL,
    SETTINGS_WINDOW_MIN_HEIGHT, SETTINGS_WINDOW_MIN_WIDTH, SETTINGS_WINDOW_WIDTH,
    STOCK_MENU_ID_PREFIX, TRAY_ID, display_connection, refresh::refresh_once,
};

/// 菜单栏下拉里的股票行句柄：符号未变时原地改文字/勾选，避免刷新时重建菜单
/// 把用户正打开的菜单顶掉。同时缓存上次渲染的文案与勾选位，
/// 让每 3 秒一轮的刷新只在内容真变化时才调用 AppKit setter。
struct TrayMenuItems {
    symbols: Vec<String>,
    items: Vec<CheckMenuItem<tauri::Wry>>,
    labels: Vec<String>,
    active_index: usize,
    /// 组合合计行（禁用态，仅作展示）；没有任何持仓时不建这一行。
    total: Option<MenuItem<tauri::Wry>>,
    total_label: String,
}

#[derive(Default)]
struct TrayRenderCache {
    menu: Option<TrayMenuItems>,
    title: Option<String>,
    tooltip: Option<String>,
}

#[derive(Default)]
pub struct TrayMenuState(Mutex<TrayRenderCache>);

pub(super) fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .title("···")
        .tooltip("TickerBar")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "settings" => {
                    let _ = show_settings(app);
                }
                "refresh" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = refresh_once(&app).await;
                    });
                }
                "quit" => app.exit(0),
                other => {
                    if let Some(index) = other
                        .strip_prefix(STOCK_MENU_ID_PREFIX)
                        .and_then(|raw| raw.parse::<usize>().ok())
                    {
                        let _ = set_active_stock(app, index);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// 点击菜单里的股票行：置顶该股票并持久化。
fn set_active_stock(app: &AppHandle, index: usize) -> Result<(), String> {
    let state = app.state::<RuntimeState>();
    {
        let mut config = state
            .config
            .write()
            .map_err(|_| "无法切换股票：内部状态已损坏".to_owned())?;
        if index >= config.stocks.len() {
            return Err("股票不存在".to_owned());
        }
        config.active_stock = index;
        save_config(&state.config_path, &config).map_err(|error| error.to_string())?;
    }
    update_tray(app)
}

pub(super) fn show_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        SETTINGS_WINDOW_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("TickerBar 设置")
    .inner_size(SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT)
    .min_inner_size(SETTINGS_WINDOW_MIN_WIDTH, SETTINGS_WINDOW_MIN_HEIGHT)
    .center()
    .build()?;
    window.set_focus()
}

/// 下拉菜单里的股票行文案：「简称  价格 ↑x.xx%」，无行情时显示「—」。
fn stock_menu_label(stock: &StockConfig, update: Option<&ProviderUpdate>) -> String {
    let name = if stock.short_name.trim().is_empty() {
        stock.symbol.trim()
    } else {
        stock.short_name.trim()
    };
    let Some(update) = update else {
        return format!("{name}  —");
    };

    let mut price = update.quote.last_price.round_dp(2);
    price.rescale(2);
    if update.quote.previous_close.is_zero() {
        return format!("{name}  {price}");
    }
    let percent = (update.quote.last_price - update.quote.previous_close)
        / update.quote.previous_close
        * Decimal::ONE_HUNDRED;
    let arrow = if percent.is_sign_negative() {
        "↓"
    } else if percent.is_zero() {
        ""
    } else {
        "↑"
    };
    let mut display_percent = percent.abs().round_dp(2);
    display_percent.rescale(2);
    format!("{name}  {price} {arrow}{display_percent}%")
}

/// 合计行文案：「合计  HKD +1234.00 ↑3.21%」，多币种各占一段、以中点分隔。
/// 没有可统计的持仓时返回 None，此时菜单里不出现这一行。
fn portfolio_menu_label(totals: &[PortfolioTotal], missing_quotes: usize) -> Option<String> {
    if totals.is_empty() {
        return None;
    }
    let segments: Vec<String> = totals
        .iter()
        .map(|total| {
            let mut profit = total.unrealized_profit.round_dp(2);
            profit.rescale(2);
            let mut percent = total.return_percent.abs().round_dp(2);
            percent.rescale(2);
            let arrow =
                if total.return_percent.is_sign_negative() && !total.return_percent.is_zero() {
                    "↓"
                } else if total.return_percent.is_zero() {
                    ""
                } else {
                    "↑"
                };
            let sign = if total.unrealized_profit.is_sign_negative() {
                ""
            } else {
                "+"
            };
            format!("{} {sign}{profit} {arrow}{percent}%", total.currency)
        })
        .collect();

    let mut label = format!("合计  {}", segments.join("  ·  "));
    if missing_quotes > 0 {
        // 缺行情就说清楚，不让用户把不完整的数字当成全部。
        label.push_str(&format!("（{missing_quotes} 只待更新）"));
    }
    Some(label)
}

pub(super) fn update_tray(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<RuntimeState>();
    // 每 3 秒都会走到这里：持读锁渲染而非整份 clone，避免每轮重复分配。
    let config = state
        .config
        .read()
        .map_err(|_| "无法更新菜单栏：设置状态已损坏".to_owned())?;
    let quotes = state
        .quotes
        .read()
        .map_err(|_| "无法更新菜单栏：行情状态已损坏".to_owned())?;
    let transport = *state
        .transport
        .read()
        .map_err(|_| "无法更新菜单栏：连接状态已损坏".to_owned())?;

    let active = config.active();
    let active_update = active.and_then(|stock| quotes.get(stock.symbol.trim()));
    let title = match (active, active_update) {
        (Some(stock), Some(update)) => {
            let connection = display_connection(transport, Some(update.connection));
            render_tray_title(
                &update.quote,
                stock.position.as_ref(),
                &config.display,
                &connection,
            )
            .map_err(|error| error.to_string())?
        }
        _ if matches!(
            transport,
            ConnectionState::Connecting | ConnectionState::Reconnecting
        ) =>
        {
            "行情加载中…".into()
        }
        _ => "行情异常".into(),
    };

    let tooltip = state
        .refresh_status
        .read()
        .map(|status| refresh_tooltip(&status))
        .unwrap_or_else(|_| "TickerBar".to_owned());

    let summary = summarize_portfolio(&config.stocks, |symbol| {
        quotes.get(symbol).map(|update| &update.quote)
    });
    let total_label = portfolio_menu_label(&summary.totals, summary.missing_quotes);

    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "菜单栏组件不存在".to_owned())?;

    let menu_state = app.state::<TrayMenuState>();
    let mut cache = menu_state
        .0
        .lock()
        .map_err(|_| "无法更新菜单：内部状态已损坏".to_owned())?;
    let symbols: Vec<String> = config
        .stocks
        .iter()
        .map(|stock| stock.symbol.trim().to_owned())
        .collect();
    // 合计行的有无会改变菜单结构，必须纳入复用判断；只是文案变化则原地改。
    let reusable = cache.menu.as_ref().is_some_and(|existing| {
        existing.symbols == symbols && existing.total.is_some() == total_label.is_some()
    });
    if let Some(existing) = cache.menu.as_mut().filter(|_| reusable) {
        // AppKit setter 有真实开销：只有文案/勾选真的变了才调用。
        for (index, (stock, item)) in config.stocks.iter().zip(&existing.items).enumerate() {
            let label = stock_menu_label(stock, quotes.get(stock.symbol.trim()));
            if existing.labels.get(index) != Some(&label) {
                let _ = item.set_text(&label);
                if let Some(slot) = existing.labels.get_mut(index) {
                    *slot = label;
                }
            }
            if existing.active_index != config.active_stock {
                let _ = item.set_checked(index == config.active_stock);
            }
        }
        existing.active_index = config.active_stock;
        if let (Some(total_item), Some(label)) = (existing.total.as_ref(), total_label.as_ref())
            && existing.total_label != *label
        {
            let _ = total_item.set_text(label);
            existing.total_label = label.clone();
        }
    } else {
        let mut items = Vec::with_capacity(config.stocks.len());
        let mut labels = Vec::with_capacity(config.stocks.len());
        for (index, stock) in config.stocks.iter().enumerate() {
            let label = stock_menu_label(stock, quotes.get(stock.symbol.trim()));
            items.push(
                CheckMenuItem::with_id(
                    app,
                    format!("{STOCK_MENU_ID_PREFIX}{index}"),
                    &label,
                    true,
                    index == config.active_stock,
                    None::<&str>,
                )
                .map_err(|error| error.to_string())?,
            );
            labels.push(label);
        }
        // 合计行禁用：它是信息展示而非可点动作，禁用态也符合系统菜单的惯例。
        let total_item = total_label
            .as_ref()
            .map(|label| {
                MenuItem::with_id(app, "portfolio-total", label, false, None::<&str>)
                    .map_err(|e| e.to_string())
            })
            .transpose()?;
        let stock_separator = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let settings = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let refresh_item = MenuItem::with_id(app, "refresh", "立即刷新", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let quit_separator = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let quit = MenuItem::with_id(app, "quit", "退出 TickerBar", true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let mut item_refs: Vec<&dyn IsMenuItem<tauri::Wry>> = items
            .iter()
            .map(|item| item as &dyn IsMenuItem<tauri::Wry>)
            .collect();
        if let Some(total_item) = total_item.as_ref() {
            item_refs.push(total_item);
        }
        item_refs.push(&stock_separator);
        item_refs.push(&settings);
        item_refs.push(&refresh_item);
        item_refs.push(&quit_separator);
        item_refs.push(&quit);
        let menu = Menu::with_items(app, &item_refs).map_err(|error| error.to_string())?;
        tray.set_menu(Some(menu))
            .map_err(|error| error.to_string())?;
        cache.menu = Some(TrayMenuItems {
            symbols,
            items,
            labels,
            active_index: config.active_stock,
            total_label: total_label.clone().unwrap_or_default(),
            total: total_item,
        });
    }

    if cache.tooltip.as_deref() != Some(tooltip.as_str()) {
        tray.set_tooltip(Some(&tooltip))
            .map_err(|error| error.to_string())?;
        cache.tooltip = Some(tooltip);
    }
    if cache.title.as_deref() != Some(title.as_str()) {
        tray.set_title(Some(&title))
            .map_err(|error| error.to_string())?;
        cache.title = Some(title);
    }
    Ok(())
}

fn refresh_tooltip(status: &RefreshStatus) -> String {
    let mut tooltip = String::from("TickerBar");
    if let Some(at) = &status.last_success_at {
        tooltip.push_str(&format!("\n最后更新 {at}"));
    }
    if let Some(error) = &status.last_error {
        tooltip.push_str(&format!("\n上次错误 {error}"));
    }
    tooltip
}

pub(super) fn configure_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppConfig, QuoteSnapshot};

    #[test]
    fn tooltip_reports_last_refresh_time_and_error_reason() {
        assert_eq!(refresh_tooltip(&RefreshStatus::default()), "TickerBar");
        assert_eq!(
            refresh_tooltip(&RefreshStatus {
                last_success_at: Some("10:00".into()),
                last_error: None,
            }),
            "TickerBar\n最后更新 10:00"
        );
        assert_eq!(
            refresh_tooltip(&RefreshStatus {
                last_success_at: Some("10:00".into()),
                last_error: Some("10:05 quote request failed: timeout".into()),
            }),
            "TickerBar\n最后更新 10:00\n上次错误 10:05 quote request failed: timeout"
        );
    }

    #[test]
    fn total_row_lists_each_currency_separately_and_flags_missing_quotes() {
        use crate::PortfolioTotal;

        assert_eq!(portfolio_menu_label(&[], 0), None);

        let profit = PortfolioTotal {
            currency: "HKD".into(),
            market_value: Decimal::from(31_000),
            cost_basis: Decimal::from(30_000),
            unrealized_profit: Decimal::from(1_000),
            return_percent: "3.3333".parse().expect("fixture percent"),
        };
        assert_eq!(
            portfolio_menu_label(std::slice::from_ref(&profit), 0),
            Some("合计  HKD +1000.00 ↑3.33%".to_owned())
        );

        let loss = PortfolioTotal {
            currency: "CNY".into(),
            market_value: Decimal::from(900),
            cost_basis: Decimal::from(1_000),
            unrealized_profit: Decimal::from(-100),
            return_percent: Decimal::from(-10),
        };
        assert_eq!(
            portfolio_menu_label(&[profit, loss], 2),
            Some("合计  HKD +1000.00 ↑3.33%  ·  CNY -100.00 ↓10.00%（2 只待更新）".to_owned())
        );
    }

    #[test]
    fn stock_rows_show_name_price_and_change_direction() {
        let stock = AppConfig::default().stocks[0].clone();
        assert_eq!(stock_menu_label(&stock, None), "小米  —");

        let update = ProviderUpdate {
            quote: QuoteSnapshot::try_new("27.380", "28.780", "HKD", 0)
                .expect("fixture quote should be valid"),
            connection: ConnectionState::Live,
        };
        assert_eq!(
            stock_menu_label(&stock, Some(&update)),
            "小米  27.38 ↓4.86%"
        );

        let flat = ProviderUpdate {
            quote: QuoteSnapshot::try_new("28.78", "28.78", "HKD", 0)
                .expect("fixture quote should be valid"),
            connection: ConnectionState::Live,
        };
        assert_eq!(stock_menu_label(&stock, Some(&flat)), "小米  28.78 0.00%");
    }
}
