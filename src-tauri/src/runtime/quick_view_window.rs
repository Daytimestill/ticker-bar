use super::QUICK_VIEW_WINDOW_LABEL;
use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const QUICK_VIEW_WIDTH: f64 = 476.0;
const QUICK_VIEW_HEIGHT: f64 = 560.0;
const QUICK_VIEW_TOP_MARGIN: f64 = 56.0;

fn position_near_top(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor_under_cursor = window.cursor_position().ok().and_then(|position| {
        window
            .monitor_from_point(position.x, position.y)
            .ok()
            .flatten()
    });
    let Some(monitor) = monitor_under_cursor
        .or(window.current_monitor()?)
        .or(window.primary_monitor()?)
    else {
        return window.center();
    };
    let scale = monitor.scale_factor();
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let window_size = window.outer_size()?;
    let x = monitor_position.x
        + (i64::from(monitor_size.width) - i64::from(window_size.width))
            .div_euclid(2)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let y = monitor_position.y + (QUICK_VIEW_TOP_MARGIN * scale).round() as i32;
    window.set_position(PhysicalPosition::new(x, y))
}

fn create_quick_view(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(
        app,
        QUICK_VIEW_WINDOW_LABEL,
        WebviewUrl::App("index.html?view=quick".into()),
    )
    .title("TickerBar 实时速览")
    .inner_size(QUICK_VIEW_WIDTH, QUICK_VIEW_HEIGHT)
    .min_inner_size(QUICK_VIEW_WIDTH, QUICK_VIEW_HEIGHT)
    .max_inner_size(QUICK_VIEW_WIDTH, QUICK_VIEW_HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    // 透明无边框窗口的原生阴影仍按矩形绘制，会和网页圆角形成双层边框。
    .shadow(false)
    .visible(false)
    .build()
}

pub(super) fn hide_quick_view(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(QUICK_VIEW_WINDOW_LABEL) {
        window.hide()?;
    }
    Ok(())
}

pub(super) fn toggle_quick_view(app: &AppHandle) -> tauri::Result<()> {
    let window = match app.get_webview_window(QUICK_VIEW_WINDOW_LABEL) {
        Some(window) => window,
        None => create_quick_view(app)?,
    };
    if window.is_visible()? {
        return window.hide();
    }
    position_near_top(&window)?;
    window.show()?;
    window.set_focus()
}
