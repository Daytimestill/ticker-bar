mod alert_dispatch;
mod commands;
mod refresh;
mod tray;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, RwLock, atomic::AtomicBool},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::Manager;
use time::OffsetDateTime;

use crate::{
    AppConfig, CONFIG_SCHEMA_VERSION, ConnectionState, MockQuoteProvider, ProviderError,
    ProviderKind, ProviderUpdate, TencentQuoteProvider, alerts::AlertMemoryEntry, load_config,
    save_config,
};

use refresh::start_refresh_loop;
use tray::{TrayMenuState, build_tray, show_settings, update_tray};

const TRAY_ID: &str = "tickerbar";
const SETTINGS_WINDOW_LABEL: &str = "settings";
const SETTINGS_WINDOW_WIDTH: f64 = 1_040.0;
const SETTINGS_WINDOW_HEIGHT: f64 = 780.0;
const SETTINGS_WINDOW_MIN_WIDTH: f64 = 900.0;
const SETTINGS_WINDOW_MIN_HEIGHT: f64 = 680.0;
const MIN_REFRESH_INTERVAL_MS: u64 = 3_000;
const MAX_REFRESH_INTERVAL_MS: u64 = 60_000;
const MAX_BACKOFF_EXPONENT: u32 = 5;
const STOCK_MENU_ID_PREFIX: &str = "stock:";

/// 最近一次刷新状态，用于排查偶发问题（挂在菜单栏 tooltip 与设置页上）。
#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshStatus {
    last_success_at: Option<String>,
    last_error: Option<String>,
}

pub struct RuntimeState {
    config: RwLock<AppConfig>,
    /// 按股票代码缓存的最近行情（含逐股市场状态）。
    quotes: RwLock<HashMap<String, ProviderUpdate>>,
    /// 数据链路整体状态：Connecting/Live/Disconnected/AuthenticationFailed。
    transport: RwLock<ConnectionState>,
    /// 提醒规则的穿越记忆：rule_id → (规则指纹, 上一轮指标值)。
    alert_memory: RwLock<HashMap<String, AlertMemoryEntry>>,
    refresh_status: RwLock<RefreshStatus>,
    mock_provider: Mutex<MockQuoteProvider>,
    tencent_provider: TencentQuoteProvider,
    config_path: PathBuf,
    /// 本次启动前 config.json 是否不存在（首次启动引导用，引导关闭后翻转）。
    first_run: AtomicBool,
    /// 刷新重入标记：定时循环、「立即刷新」、保存设置三个触发源互斥。
    refreshing: AtomicBool,
}

impl RuntimeState {
    fn new(
        config: AppConfig,
        config_path: PathBuf,
        first_run: bool,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            config: RwLock::new(config),
            quotes: RwLock::new(HashMap::new()),
            transport: RwLock::new(ConnectionState::Connecting),
            alert_memory: RwLock::new(HashMap::new()),
            refresh_status: RwLock::new(RefreshStatus::default()),
            mock_provider: Mutex::new(MockQuoteProvider::default()),
            tencent_provider: TencentQuoteProvider::new()?,
            config_path,
            first_run: AtomicBool::new(first_run),
            refreshing: AtomicBool::new(false),
        })
    }

    /// 状态记录是旁路信息，锁异常时静默跳过，不让它影响主流程。
    fn record_refresh_success(&self) {
        if let Ok(mut status) = self.refresh_status.write() {
            status.last_success_at = Some(current_clock_text());
            status.last_error = None;
        }
    }

    fn record_refresh_error(&self, error: &str) {
        if let Ok(mut status) = self.refresh_status.write() {
            status.last_error = Some(format!("{} {error}", current_clock_text()));
        }
    }
}

/// 链路异常（断网/未配置）优先于逐股市场状态展示。
fn display_connection(
    transport: ConnectionState,
    stock_connection: Option<ConnectionState>,
) -> ConnectionState {
    match transport {
        ConnectionState::Connecting
        | ConnectionState::Reconnecting
        | ConnectionState::Disconnected
        | ConnectionState::AuthenticationFailed => transport,
        ConnectionState::Live | ConnectionState::Delayed | ConnectionState::Closed => {
            stock_connection.unwrap_or(transport)
        }
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_settings(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::get_app_info,
            commands::get_first_run,
            commands::dismiss_first_run,
            commands::get_refresh_status,
            commands::preview_title,
            commands::preview_portfolio,
            commands::send_test_alert,
            commands::search_stocks,
            commands::save_user_config
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config_path = app.path().app_config_dir()?.join("config.json");
            // 配置损坏（手编 JSON 打错字等）不能让应用起不来：
            // 备份原文件、回退默认配置继续启动，并把原因挂到刷新状态上。
            let (loaded, recovery_note) = load_config_with_recovery(&config_path);
            let first_run = loaded.is_none();
            let mut config = loaded.unwrap_or_default();
            migrate_legacy_config(&mut config);
            // 无条件写回：v1→v2 的结构迁移发生在解析层，这里统一持久化。
            save_config(&config_path, &config)?;
            let state =
                RuntimeState::new(config, config_path, first_run).map_err(std::io::Error::other)?;
            app.manage(state);
            app.manage(TrayMenuState::default());
            if let Some(note) = &recovery_note {
                app.state::<RuntimeState>().record_refresh_error(note);
            }

            build_tray(app.handle())?;
            update_tray(app.handle()).map_err(std::io::Error::other)?;
            start_refresh_loop(app.handle().clone());
            // 首次启动纯菜单栏应用容易让人找不到入口：自动打开设置窗展示引导。
            if first_run {
                let _ = show_settings(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("TickerBar failed to start");

    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested { code, api, .. } = event
            && should_prevent_exit(code)
        {
            api.prevent_exit();
        }
    });
}

fn should_prevent_exit(code: Option<i32>) -> bool {
    code.is_none()
}

/// 读取持久化配置；读不出来（JSON 损坏、校验失败）时把原文件改名备份、
/// 回退到「文件不存在」语义，让启动流程永远能走通。
/// 返回 (加载结果, 恢复提示)；恢复提示非空即发生过自愈。
fn load_config_with_recovery(config_path: &Path) -> (Option<AppConfig>, Option<String>) {
    match load_config(config_path) {
        Ok(loaded) => (loaded, None),
        Err(error) => {
            let backup_path =
                config_path.with_extension(format!("json.corrupted-{}", current_timestamp()));
            let backup_note = match fs::rename(config_path, &backup_path) {
                Ok(()) => format!("原文件已备份为 {}", backup_path.display()),
                Err(rename_error) => format!("原文件备份失败：{rename_error}"),
            };
            (
                None,
                Some(format!(
                    "配置文件损坏，已重置为默认配置（{backup_note}）：{error}"
                )),
            )
        }
    }
}

fn migrate_legacy_config(config: &mut AppConfig) -> bool {
    let mut changed = false;

    if config.provider == ProviderKind::Mock {
        config.provider = ProviderKind::Tencent;
        if let Some(stock) = config.stocks.first_mut()
            && stock.short_name.trim() == "演示"
        {
            stock.short_name = "小米".into();
        }
        changed = true;
    }

    let normalized_interval = config
        .tray_throttle_ms
        .clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS);
    if normalized_interval != config.tray_throttle_ms {
        config.tray_throttle_ms = normalized_interval;
        changed = true;
    }

    if config.schema_version != CONFIG_SCHEMA_VERSION {
        config.schema_version = CONFIG_SCHEMA_VERSION;
        changed = true;
    }

    changed
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn current_clock_text() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!("{:02}:{:02}", now.hour(), now.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_window_is_wide_enough_for_the_editor_workbench() {
        assert_eq!(SETTINGS_WINDOW_WIDTH, 1_040.0);
        assert_eq!(SETTINGS_WINDOW_HEIGHT, 780.0);
        assert_eq!(SETTINGS_WINDOW_MIN_WIDTH, 900.0);
    }

    #[test]
    fn migrates_the_saved_mock_demo_to_the_real_provider() {
        let mut config = AppConfig {
            provider: ProviderKind::Mock,
            ..AppConfig::default()
        };
        config.stocks[0].short_name = "演示".into();

        assert!(migrate_legacy_config(&mut config));
        assert_eq!(config.provider, ProviderKind::Tencent);
        assert_eq!(config.stocks[0].short_name, "小米");
    }

    #[test]
    fn migrates_an_overly_fast_saved_refresh_interval() {
        let mut config = AppConfig {
            provider: ProviderKind::Tencent,
            tray_throttle_ms: 1_000,
            ..AppConfig::default()
        };

        assert!(migrate_legacy_config(&mut config));
        assert_eq!(config.tray_throttle_ms, 3_000);
    }

    #[test]
    fn refresh_status_serializes_with_camel_case_keys_for_the_frontend() {
        let status = RefreshStatus {
            last_success_at: Some("10:23".into()),
            last_error: Some("10:25 quote request failed".into()),
        };
        let json = serde_json::to_value(&status).expect("status should serialize");
        assert_eq!(json["lastSuccessAt"], "10:23");
        assert_eq!(json["lastError"], "10:25 quote request failed");
    }

    #[test]
    fn transport_problems_override_per_stock_market_state() {
        assert_eq!(
            display_connection(ConnectionState::Disconnected, Some(ConnectionState::Live)),
            ConnectionState::Disconnected
        );
        assert_eq!(
            display_connection(ConnectionState::Connecting, Some(ConnectionState::Closed)),
            ConnectionState::Connecting
        );
        assert_eq!(
            display_connection(ConnectionState::Live, Some(ConnectionState::Closed)),
            ConnectionState::Closed
        );
        assert_eq!(
            display_connection(ConnectionState::Live, None),
            ConnectionState::Live
        );
    }

    #[test]
    fn recovers_from_a_corrupted_config_file_instead_of_refusing_to_start() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tickerbar-recovery-{unique}"));
        fs::create_dir_all(&dir).expect("test directory should be created");
        let path = dir.join("config.json");
        fs::write(&path, "{not-json").expect("fixture should be written");

        let (loaded, note) = load_config_with_recovery(&path);

        assert!(loaded.is_none(), "corrupt config must fall back to default");
        let note = note.expect("recovery should explain what happened");
        assert!(note.contains("配置文件损坏"));
        assert!(!path.exists(), "corrupt file should be moved aside");
        let backups = fs::read_dir(&dir)
            .expect("test directory should be listable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("corrupted"))
            .count();
        assert_eq!(backups, 1, "original file must be kept as a backup");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_passes_through_missing_and_healthy_configs() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tickerbar-recovery-ok-{unique}"));
        let path = dir.join("config.json");

        let (loaded, note) = load_config_with_recovery(&path);
        assert!(loaded.is_none());
        assert!(note.is_none(), "missing file is first-run, not corruption");

        save_config(&path, &AppConfig::default()).expect("fixture config should save");
        let (loaded, note) = load_config_with_recovery(&path);
        assert!(loaded.is_some());
        assert!(note.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn closing_the_last_settings_window_keeps_the_tray_app_running() {
        assert!(should_prevent_exit(None));
    }

    #[test]
    fn choosing_quit_from_the_tray_still_exits_the_app() {
        assert!(!should_prevent_exit(Some(0)));
    }
}
