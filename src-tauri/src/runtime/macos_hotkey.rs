//! macOS 原生全局快捷键。
//!
//! 使用系统自带的 Carbon Hot Key API，不监听普通键盘输入，因此无需辅助功能或
//! 输入监控权限。这里只开放设置页提供的三组组合键，避免自由文本映射产生歧义。

use std::{ffi::c_void, mem::size_of, ptr, sync::Mutex};

use tauri::{AppHandle, Manager};

use crate::AppConfig;

use super::quick_view_window::toggle_quick_view;

type OSStatus = i32;
type EventTargetRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventRef = *mut c_void;
type EventHandlerRef = *mut c_void;
type EventHotKeyRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

type EventHandlerProc =
    unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

const NO_ERR: OSStatus = 0;
const EVENT_NOT_HANDLED_ERR: OSStatus = -9874;
const EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const EVENT_HOT_KEY_PRESSED: u32 = 5;
const EVENT_PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const HOT_KEY_SIGNATURE: u32 = u32::from_be_bytes(*b"TKBR");
const HOT_KEY_ID: u32 = 1;
const HOT_KEY_EXCLUSIVE: u32 = 1;

const COMMAND_KEY: u32 = 1 << 8;
const SHIFT_KEY: u32 = 1 << 9;
const CONTROL_KEY: u32 = 1 << 12;
const KEY_T: u32 = 0x11;
const KEY_Y: u32 = 0x10;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerProc,
        type_count: usize,
        type_list: *const EventTypeSpec,
        user_data: *mut c_void,
        out_handler: *mut EventHandlerRef,
    ) -> OSStatus;
    fn RemoveEventHandler(handler: EventHandlerRef) -> OSStatus;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        hot_key_id: EventHotKeyId,
        target: EventTargetRef,
        options: u32,
        out_ref: *mut EventHotKeyRef,
    ) -> OSStatus;
    fn UnregisterEventHotKey(hot_key: EventHotKeyRef) -> OSStatus;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        out_data: *mut c_void,
    ) -> OSStatus;
}

fn shortcut_code(value: &str) -> Result<(u32, u32), String> {
    match value {
        "Control+Command+T" => Ok((KEY_T, CONTROL_KEY | COMMAND_KEY)),
        "Control+Shift+T" => Ok((KEY_T, CONTROL_KEY | SHIFT_KEY)),
        "Command+Shift+Y" => Ok((KEY_Y, COMMAND_KEY | SHIFT_KEY)),
        _ => Err(format!("不支持的全局快捷键：{value}")),
    }
}

unsafe extern "C" fn hot_key_handler(
    _call: EventHandlerCallRef,
    event: EventRef,
    user_data: *mut c_void,
) -> OSStatus {
    if event.is_null() || user_data.is_null() {
        return EVENT_NOT_HANDLED_ERR;
    }
    let mut id = EventHotKeyId::default();
    // SAFETY: Carbon 为该回调提供有效 EventRef；输出缓冲区与声明的结构体大小一致。
    let status = unsafe {
        GetEventParameter(
            event,
            EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            ptr::null_mut(),
            size_of::<EventHotKeyId>(),
            ptr::null_mut(),
            (&raw mut id).cast(),
        )
    };
    if status != NO_ERR || id.signature != HOT_KEY_SIGNATURE || id.id != HOT_KEY_ID {
        return EVENT_NOT_HANDLED_ERR;
    }
    // SAFETY: user_data 指向 MacHotKeyState 持有的 Box<AppHandle>，其生命周期覆盖应用。
    let app = unsafe { &*(user_data.cast::<AppHandle>()) };
    let _ = toggle_quick_view(app);
    NO_ERR
}

pub(super) struct MacHotKeyState {
    app: Box<AppHandle>,
    handler_ref: usize,
    hot_key_ref: Mutex<Option<usize>>,
}

impl MacHotKeyState {
    pub(super) fn install(app: AppHandle) -> Result<Self, String> {
        let app = Box::new(app);
        let user_data = (&raw const *app).cast_mut().cast::<c_void>();
        let event_type = EventTypeSpec {
            event_class: EVENT_CLASS_KEYBOARD,
            event_kind: EVENT_HOT_KEY_PRESSED,
        };
        let mut handler_ref: EventHandlerRef = ptr::null_mut();
        // SAFETY: 在 Tauri setup 主线程安装一次处理器；函数指针和 user_data 均保持有效。
        let status = unsafe {
            InstallEventHandler(
                GetApplicationEventTarget(),
                hot_key_handler,
                1,
                &raw const event_type,
                user_data,
                &raw mut handler_ref,
            )
        };
        if status != NO_ERR {
            return Err(format!("无法初始化全局快捷键（系统错误 {status}）"));
        }
        Ok(Self {
            app,
            handler_ref: handler_ref as usize,
            hot_key_ref: Mutex::new(None),
        })
    }

    fn register_direct(&self, shortcut: &str) -> Result<(), String> {
        let (key_code, modifiers) = shortcut_code(shortcut)?;
        let mut hot_key_ref: EventHotKeyRef = ptr::null_mut();
        // SAFETY: 仅在主线程调用；Carbon 返回的句柄由本状态保存并负责注销。
        let status = unsafe {
            RegisterEventHotKey(
                key_code,
                modifiers,
                EventHotKeyId {
                    signature: HOT_KEY_SIGNATURE,
                    id: HOT_KEY_ID,
                },
                GetApplicationEventTarget(),
                HOT_KEY_EXCLUSIVE,
                &raw mut hot_key_ref,
            )
        };
        if status != NO_ERR {
            return Err(format!(
                "快捷键 {shortcut} 无法使用，可能已被其他应用占用（系统错误 {status}）"
            ));
        }
        *self
            .hot_key_ref
            .lock()
            .map_err(|_| "快捷键状态已损坏".to_owned())? = Some(hot_key_ref as usize);
        Ok(())
    }

    fn unregister_direct(&self) -> Result<(), String> {
        let mut slot = self
            .hot_key_ref
            .lock()
            .map_err(|_| "快捷键状态已损坏".to_owned())?;
        let Some(reference) = slot.take() else {
            return Ok(());
        };
        // SAFETY: reference 来自 RegisterEventHotKey，且从 slot 取出后只注销一次。
        let status = unsafe { UnregisterEventHotKey(reference as EventHotKeyRef) };
        if status == NO_ERR {
            Ok(())
        } else {
            *slot = Some(reference);
            Err(format!("旧快捷键释放失败（系统错误 {status}）"))
        }
    }

    fn apply_direct(&self, before: &AppConfig, after: &AppConfig) -> Result<(), String> {
        if before.quick_view_enabled == after.quick_view_enabled
            && before.quick_view_shortcut == after.quick_view_shortcut
        {
            return Ok(());
        }
        if before.quick_view_enabled {
            self.unregister_direct()?;
        }
        if after.quick_view_enabled
            && let Err(error) = self.register_direct(&after.quick_view_shortcut)
        {
            if before.quick_view_enabled {
                let _ = self.register_direct(&before.quick_view_shortcut);
            }
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn register_initial(&self, config: &AppConfig) -> Result<(), String> {
        if config.quick_view_enabled {
            self.register_direct(&config.quick_view_shortcut)
        } else {
            Ok(())
        }
    }
}

impl Drop for MacHotKeyState {
    fn drop(&mut self) {
        if let Ok(slot) = self.hot_key_ref.get_mut()
            && let Some(reference) = slot.take()
        {
            // SAFETY: 应用退出时释放仍登记的句柄。
            let _ = unsafe { UnregisterEventHotKey(reference as EventHotKeyRef) };
        }
        if self.handler_ref != 0 {
            // SAFETY: handler_ref 来自 InstallEventHandler，且只在 Drop 中移除一次。
            let _ = unsafe { RemoveEventHandler(self.handler_ref as EventHandlerRef) };
        }
        // 读取一次，明确保证 Box<AppHandle> 活到所有 Carbon 句柄移除之后。
        let _ = &self.app;
    }
}

pub(super) async fn rebind_quick_view_shortcut(
    app: &AppHandle,
    before: &AppConfig,
    after: &AppConfig,
) -> Result<(), String> {
    if before.quick_view_enabled == after.quick_view_enabled
        && before.quick_view_shortcut == after.quick_view_shortcut
    {
        return Ok(());
    }
    let before = before.clone();
    let after = after.clone();
    let app_for_main = app.clone();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let result = app_for_main
            .state::<MacHotKeyState>()
            .apply_direct(&before, &after);
        let _ = sender.send(result);
    })
    .map_err(|error| format!("无法更新全局快捷键：{error}"))?;
    receiver
        .await
        .map_err(|_| "全局快捷键更新被中断".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_the_shortcuts_exposed_by_settings() {
        assert_eq!(
            shortcut_code("Control+Command+T"),
            Ok((KEY_T, CONTROL_KEY | COMMAND_KEY))
        );
        assert_eq!(
            shortcut_code("Control+Shift+T"),
            Ok((KEY_T, CONTROL_KEY | SHIFT_KEY))
        );
        assert_eq!(
            shortcut_code("Command+Shift+Y"),
            Ok((KEY_Y, COMMAND_KEY | SHIFT_KEY))
        );
        assert!(shortcut_code("Command+Q").is_err());
    }
}
