use std::{collections::HashMap, sync::atomic::Ordering, time::UNIX_EPOCH};

use tauri::{AppHandle, State};
use time::{OffsetDateTime, UtcOffset};

use crate::{
    AlertRule, AppConfig, CONFIG_SCHEMA_VERSION, ConnectionState, PortfolioSummary, ProviderKind,
    StockSearchResult, TencentQuoteProvider,
    alerts::{build_notification, metric_value},
    render_tray_title, save_config, summarize_portfolio,
};

use super::{
    MAX_REFRESH_INTERVAL_MS, MIN_REFRESH_INTERVAL_MS, RefreshStatus, RuntimeState,
    alert_dispatch::send_notification,
    current_timestamp, display_connection,
    refresh::refresh_once,
    tray::{configure_launch_at_login, update_tray},
};

#[tauri::command]
pub(super) fn get_config(state: State<'_, RuntimeState>) -> Result<AppConfig, String> {
    state
        .config
        .read()
        .map(|config| config.clone())
        .map_err(|_| "无法读取设置：内部状态已损坏".to_owned())
}

#[tauri::command]
pub(super) fn preview_title(
    state: State<'_, RuntimeState>,
    config: AppConfig,
) -> Result<String, String> {
    config.validate().map_err(|error| error.to_string())?;
    let Some(active) = config.active() else {
        return Ok("行情加载中…".into());
    };
    let quotes = state
        .quotes
        .read()
        .map_err(|_| "无法预览：行情状态已损坏".to_owned())?;
    let Some(update) = quotes.get(active.symbol.trim()) else {
        return Ok(if quotes.is_empty() {
            "行情加载中…".into()
        } else {
            "保存后加载行情".into()
        });
    };
    let transport = *state
        .transport
        .read()
        .map_err(|_| "无法预览：连接状态已损坏".to_owned())?;
    let connection = display_connection(transport, Some(update.connection));

    render_tray_title(
        &update.quote,
        active.position.as_ref(),
        &config.display,
        &connection,
    )
    .map_err(|error| error.to_string())
}

/// 设置页持仓分区的实时收益与合计。
/// 和 preview_title 一样接收「编辑中」的配置，让还没保存的数量/成本立刻反映出来。
#[tauri::command]
pub(super) fn preview_portfolio(
    state: State<'_, RuntimeState>,
    config: AppConfig,
) -> Result<PortfolioSummary, String> {
    let quotes = state
        .quotes
        .read()
        .map_err(|_| "无法统计持仓：行情状态已损坏".to_owned())?;
    Ok(summarize_portfolio(&config.stocks, |symbol| {
        quotes.get(symbol).map(|update| &update.quote)
    }))
}

/// 试发一条提醒通知，用来验证整条通知链路（尤其是系统通知权限）。
///
/// 休市时提醒本就不会触发——那份不动的收盘价会天天满足条件、反复轰炸，
/// 所以判定只在交易中/延迟时进行。于是需要这个入口：它跳过穿越判定，
/// 但文案组装、静默开关与发送路径都与真实触发完全一致。
/// 指标值优先取当前缓存行情（休市时即最近收盘价），取不到就退回阈值本身。
#[tauri::command]
pub(super) fn send_test_alert(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    rule: AlertRule,
) -> Result<(), String> {
    let config = state
        .config
        .read()
        .map_err(|_| "无法试发：设置状态已损坏".to_owned())?;
    let stock = config
        .stocks
        .iter()
        .find(|stock| stock.symbol.trim() == rule.symbol.trim())
        .ok_or_else(|| format!("找不到股票 {}，请先保存设置", rule.symbol))?;

    let value = state
        .quotes
        .read()
        .ok()
        .and_then(|quotes| {
            quotes
                .get(stock.symbol.trim())
                .and_then(|update| metric_value(rule.metric, stock, update))
        })
        // 还没拿到行情时用阈值占位，至少能验证通知本身发得出去
        .unwrap_or(rule.threshold);

    let notification = build_notification(&rule, stock, value);
    send_notification(&app, &notification).map_err(|error| {
        format!("通知发送失败，请到 系统设置 → 通知 里允许 TickerBar 发送通知：{error}")
    })
}

/// 版本与构建时间，用于确认「装的到底是不是刚打的那个包」。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppInfo {
    version: String,
    built_at: Option<String>,
}

/// 取可执行文件的修改时间当作构建时间：不需要在 build.rs 里埋时间戳，
/// 覆盖安装后必然变化，正好回答「这个包是不是新的」。
fn executable_built_at() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let modified = std::fs::metadata(executable).ok()?.modified().ok()?;
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let offset = OffsetDateTime::now_local()
        .map(|now| now.offset())
        .unwrap_or(UtcOffset::UTC);
    let stamp = OffsetDateTime::from_unix_timestamp(seconds)
        .ok()?
        .to_offset(offset);
    Some(format!(
        "{:02}-{:02} {:02}:{:02}",
        stamp.month() as u8,
        stamp.day(),
        stamp.hour(),
        stamp.minute()
    ))
}

#[tauri::command]
pub(super) fn get_app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        built_at: executable_built_at(),
    }
}

#[tauri::command]
pub(super) fn get_first_run(state: State<'_, RuntimeState>) -> bool {
    state.first_run.load(Ordering::Relaxed)
}

#[tauri::command]
pub(super) fn dismiss_first_run(state: State<'_, RuntimeState>) {
    state.first_run.store(false, Ordering::Relaxed);
}

#[tauri::command]
pub(super) fn get_refresh_status(state: State<'_, RuntimeState>) -> Result<RefreshStatus, String> {
    state
        .refresh_status
        .read()
        .map(|status| status.clone())
        .map_err(|_| "无法读取刷新状态：内部状态已损坏".to_owned())
}

#[tauri::command]
pub(super) async fn search_stocks(
    state: State<'_, RuntimeState>,
    query: String,
) -> Result<Vec<StockSearchResult>, String> {
    state
        .tencent_provider
        .search_stocks(&query)
        .await
        .map_err(|error| error.to_string())
}

fn normalized_symbols(config: &AppConfig) -> Vec<String> {
    let mut symbols: Vec<String> = config
        .stocks
        .iter()
        .map(|stock| stock.symbol.trim().to_ascii_uppercase())
        .collect();
    symbols.sort();
    symbols
}

#[tauri::command]
pub(super) async fn save_user_config(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    mut config: AppConfig,
) -> Result<AppConfig, String> {
    config.schema_version = CONFIG_SCHEMA_VERSION;
    config.tray_throttle_ms = config
        .tray_throttle_ms
        .clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS);
    if config.provider == ProviderKind::Tencent {
        for stock in &mut config.stocks {
            stock.symbol = TencentQuoteProvider::canonical_symbol(&stock.symbol).map_err(|_| {
                format!("无法识别股票代码 {}，请从搜索结果中选择股票", stock.symbol)
            })?;
        }
    }
    config.validate().map_err(|error| error.to_string())?;

    let quote_identity_changed = {
        let stored = state
            .config
            .read()
            .map_err(|_| "无法保存设置：内部状态已损坏".to_owned())?;
        stored.provider != config.provider
            || normalized_symbols(&stored) != normalized_symbols(&config)
    };

    // P0：股票列表变化时先批量验证全部行情，失败则不落盘、保留原配置。
    let prefetched = if quote_identity_changed && config.provider == ProviderKind::Tencent {
        let results = state
            .tencent_provider
            .next_quotes(&config.stocks, current_timestamp())
            .await
            .map_err(|error| format!("无法获取行情，已保留原设置：{error}"))?;
        let mut updates = HashMap::new();
        for (stock, result) in config.stocks.iter().zip(results) {
            let update = result.map_err(|error| {
                format!("无法获取 {} 的行情，已保留原设置：{error}", stock.symbol)
            })?;
            updates.insert(stock.symbol.trim().to_owned(), update);
        }
        Some(updates)
    } else {
        None
    };

    save_config(&state.config_path, &config).map_err(|error| error.to_string())?;
    *state
        .config
        .write()
        .map_err(|_| "无法保存设置：内部状态已损坏".to_owned())? = config.clone();

    match prefetched {
        Some(updates) => {
            *state
                .quotes
                .write()
                .map_err(|_| "无法保存设置：行情状态已损坏".to_owned())? = updates;
            *state
                .transport
                .write()
                .map_err(|_| "无法保存设置：连接状态已损坏".to_owned())? = ConnectionState::Live;
            state.record_refresh_success();
            update_tray(&app)?;
        }
        None if quote_identity_changed => {
            state
                .quotes
                .write()
                .map_err(|_| "无法保存设置：行情状态已损坏".to_owned())?
                .clear();
            *state
                .transport
                .write()
                .map_err(|_| "无法保存设置：连接状态已损坏".to_owned())? =
                ConnectionState::Connecting;
            update_tray(&app)?;
        }
        None => {}
    }

    let autostart_result = configure_launch_at_login(&app, config.launch_at_login);
    // 已预取过全部行情就不必再刷新；其余情况刷新一次让新配置立即生效。
    let refresh_result = if quote_identity_changed && config.provider == ProviderKind::Tencent {
        Ok(())
    } else {
        refresh_once(&app).await
    };

    if let Err(error) = autostart_result {
        return Err(format!("设置已保存，但登录启动更新失败：{error}"));
    }
    refresh_result.map_err(|error| format!("设置已保存，但行情刷新失败：{error}"))?;

    Ok(config)
}
