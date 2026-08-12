use tauri::{AppHandle, Emitter, Manager};

use crate::{
    AppConfig,
    alerts::{
        AlertMemoryEntry, AlertNotification, evaluate_rule, exchange_day, mark_triggered,
        rule_fingerprint,
    },
    save_config,
};

use super::RuntimeState;

/// 提醒判定：对每条规则做穿越检测，触发则发系统通知并持久化规则状态。
/// 旁路功能——任何一步失败都跳过并留痕，不影响行情刷新主流程。
/// config 由调用方（本轮刷新）传入快照，避免每轮重复整份 clone。
pub(super) fn process_alerts(app: &AppHandle, config: &AppConfig, timestamp: i64) {
    if config.alerts.is_empty() {
        return;
    }
    let state = app.state::<RuntimeState>();
    let today = exchange_day(timestamp);

    let mut notifications: Vec<AlertNotification> = Vec::new();
    {
        let Ok(quotes) = state.quotes.read() else {
            return;
        };
        let Ok(mut memory) = state.alert_memory.write() else {
            return;
        };
        for rule in &config.alerts {
            let fingerprint = rule_fingerprint(rule);
            let evaluated = config
                .stocks
                .iter()
                .find(|stock| stock.symbol.trim() == rule.symbol.trim())
                .and_then(|stock| {
                    quotes.get(stock.symbol.trim()).map(|update| {
                        // 规则语义变过（编辑/手改文件）时旧记忆作废：
                        // 按首次评估重新武装，不拿旧值与新阈值比出假穿越。
                        let previous = memory
                            .get(&rule.id)
                            .filter(|entry| entry.fingerprint == fingerprint)
                            .map(|entry| entry.value);
                        evaluate_rule(rule, stock, update, previous, &today)
                    })
                });
            if let Some((value, fired)) = evaluated {
                // 指标临时算不出时保留旧记忆，避免重臂后误触发。
                if let Some(value) = value {
                    memory.insert(rule.id.clone(), AlertMemoryEntry { fingerprint, value });
                }
                if let Some(notification) = fired {
                    notifications.push(notification);
                }
            }
        }
        // 清理已删除规则的记忆，防长期运行残留。
        memory.retain(|rule_id, _| config.alerts.iter().any(|rule| &rule.id == rule_id));
    }

    if notifications.is_empty() {
        return;
    }

    // 先持久化规则状态（每日去重/一次性停用），再发通知：
    // 即使通知失败也不会在下一轮重复触发。
    if let Ok(mut stored) = state.config.write() {
        for notification in &notifications {
            if let Some(rule) = stored
                .alerts
                .iter_mut()
                .find(|rule| rule.id == notification.rule_id)
            {
                mark_triggered(rule, &today);
            }
        }
        let _ = save_config(&state.config_path, &stored);
    }

    for notification in notifications {
        // 发送失败（最常见：系统通知权限未授权）必须留痕——
        // 规则状态已消耗，静默吞掉会让用户以为提醒从未生效过。
        if let Err(error) = send_notification(app, &notification) {
            state.record_refresh_error(&format!(
                "提醒通知发送失败（请检查系统设置中 TickerBar 的通知权限）：{error}"
            ));
        }
    }
}

/// 推给设置窗口的事件名，前端据此弹窗内 Toast。
pub(super) const ALERT_EVENT: &str = "alert-triggered";

pub(super) fn send_notification(
    app: &AppHandle,
    notification: &AlertNotification,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    // 设置窗口在最前台时 macOS 不给自家 App 弹横幅，只把通知折叠进通知中心。
    // 这个事件让开着的设置窗口自己补一条 Toast——窗口没开就没人收，正好。
    //
    // 放在发送之前：系统通知失败（最典型是权限被关）时这条反而是唯一的可见反馈。
    // 放在 send_notification 内部而非各调用点：真实触发与试发共用这个入口，
    // 结构上就不可能只有一条路径带 Toast。
    let _ = app.emit(ALERT_EVENT, notification);

    let mut builder = app
        .notification()
        .builder()
        .title(&notification.title)
        .body(&notification.body);
    if !notification.silent {
        builder = builder.sound("default");
    }
    builder.show().map_err(|error| error.to_string())
}
