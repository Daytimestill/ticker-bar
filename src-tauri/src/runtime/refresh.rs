use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use tauri::{AppHandle, Manager};

use crate::{ConnectionState, ProviderError, ProviderKind, ProviderUpdate, QuoteProvider};

use super::{
    MAX_BACKOFF_EXPONENT, MAX_REFRESH_INTERVAL_MS, MIN_REFRESH_INTERVAL_MS, RuntimeState,
    alert_dispatch::process_alerts, current_clock_text, current_timestamp, tray::update_tray,
};

/// 一轮刷新中逐股的抓取结果（symbol → 结果）。
type StockFetchResults = Vec<(String, Result<ProviderUpdate, ProviderError>)>;

pub(super) fn start_refresh_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut consecutive_failures = 0_u32;
        loop {
            // 每次刷新放进独立任务：即使刷新链路 panic，也只损失本轮，
            // 循环本身继续存活，避免菜单栏从此停在旧数据上。
            let refresh_app = app.clone();
            let outcome =
                tauri::async_runtime::spawn(async move { refresh_once(&refresh_app).await }).await;
            match outcome {
                Ok(Ok(())) => consecutive_failures = 0,
                Ok(Err(_)) => consecutive_failures = consecutive_failures.saturating_add(1),
                Err(join_error) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    app.state::<RuntimeState>()
                        .record_refresh_error(&format!("刷新任务异常终止：{join_error}"));
                }
            }

            let configured_interval = app
                .state::<RuntimeState>()
                .config
                .read()
                .map(|config| config.tray_throttle_ms)
                .unwrap_or(MIN_REFRESH_INTERVAL_MS);
            let delay = refresh_delay_ms(configured_interval, consecutive_failures);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    });
}

fn refresh_delay_ms(configured_interval: u64, consecutive_failures: u32) -> u64 {
    let base_interval = configured_interval.clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS);
    let multiplier = 1_u64 << consecutive_failures.min(MAX_BACKOFF_EXPONENT);

    base_interval
        .saturating_mul(multiplier)
        .min(MAX_REFRESH_INTERVAL_MS)
}

/// 刷新入口：定时循环、菜单「立即刷新」、保存设置三处都会调用。
/// 并发刷新会重复请求接口、并让同一条提醒规则被评估两次，
/// 这里用原子标记互斥：已有一轮在跑时直接让位，结果由那一轮负责。
pub(super) async fn refresh_once(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<RuntimeState>();
    if state
        .refreshing
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return Ok(());
    }
    // Drop 兜底复位：即使刷新链路 panic 也不会把标记卡死在 true。
    let _guard = RefreshingGuard(&state.refreshing);
    refresh_once_inner(app).await
}

struct RefreshingGuard<'a>(&'a AtomicBool);

impl Drop for RefreshingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

async fn refresh_once_inner(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<RuntimeState>();
    let config = state
        .config
        .read()
        .map_err(|_| "无法刷新：设置状态已损坏".to_owned())?
        .clone();
    let timestamp = current_timestamp();

    let outcome: Result<StockFetchResults, ProviderError> = match config.provider {
        ProviderKind::Mock => {
            let clock = current_clock_text();
            let mut provider = state
                .mock_provider
                .lock()
                .map_err(|_| "无法刷新：Mock 行情状态已损坏".to_owned())?;
            Ok(config
                .stocks
                .iter()
                .map(|stock| {
                    (
                        stock.symbol.trim().to_owned(),
                        provider.next_quote(stock, timestamp, &clock),
                    )
                })
                .collect())
        }
        ProviderKind::Tencent => state
            .tencent_provider
            .next_quotes(&config.stocks, timestamp)
            .await
            .map(|results| {
                config
                    .stocks
                    .iter()
                    .map(|stock| stock.symbol.trim().to_owned())
                    .zip(results)
                    .collect()
            }),
        ProviderKind::Longbridge => Err(ProviderError::NotConfigured),
    };

    match outcome {
        Ok(results) => {
            let mut succeeded = 0_usize;
            let mut errors: Vec<String> = Vec::new();
            {
                let mut quotes = state
                    .quotes
                    .write()
                    .map_err(|_| "无法刷新：行情状态已损坏".to_owned())?;
                // 清掉已被移除股票的缓存，避免长期运行下的残留。
                quotes.retain(|symbol, _| {
                    config
                        .stocks
                        .iter()
                        .any(|stock| stock.symbol.trim() == symbol)
                });
                for (symbol, result) in results {
                    match result {
                        Ok(update) => {
                            quotes.insert(symbol, update);
                            succeeded += 1;
                        }
                        Err(error) => errors.push(format!("{symbol}: {error}")),
                    }
                }
            }
            if succeeded > 0 {
                *state
                    .transport
                    .write()
                    .map_err(|_| "无法刷新：连接状态已损坏".to_owned())? = ConnectionState::Live;
                state.record_refresh_success();
                if !errors.is_empty() {
                    state.record_refresh_error(&errors.join("；"));
                }
                process_alerts(app, &config, timestamp);
                update_tray(app)
            } else {
                *state
                    .transport
                    .write()
                    .map_err(|_| "无法刷新：连接状态已损坏".to_owned())? =
                    ConnectionState::Disconnected;
                let reason = errors.join("；");
                state.record_refresh_error(&reason);
                update_tray(app)?;
                Err(reason)
            }
        }
        Err(error) => {
            let transport = if matches!(error, ProviderError::NotConfigured) {
                ConnectionState::AuthenticationFailed
            } else {
                ConnectionState::Disconnected
            };
            *state
                .transport
                .write()
                .map_err(|_| "无法刷新：连接状态已损坏".to_owned())? = transport;
            state.record_refresh_error(&error.to_string());
            update_tray(app)?;
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_a_conservative_minimum_refresh_interval() {
        assert_eq!(refresh_delay_ms(500, 0), 3_000);
        assert_eq!(refresh_delay_ms(1_000, 0), 3_000);
        assert_eq!(refresh_delay_ms(5_000, 0), 5_000);
        assert_eq!(refresh_delay_ms(90_000, 0), 60_000);
    }

    #[test]
    fn backs_off_after_failures_and_recovers_to_the_configured_interval() {
        assert_eq!(refresh_delay_ms(3_000, 1), 6_000);
        assert_eq!(refresh_delay_ms(3_000, 2), 12_000);
        assert_eq!(refresh_delay_ms(3_000, 3), 24_000);
        assert_eq!(refresh_delay_ms(3_000, 4), 48_000);
        assert_eq!(refresh_delay_ms(3_000, 5), 60_000);
        assert_eq!(refresh_delay_ms(3_000, 0), 3_000);
    }
}
