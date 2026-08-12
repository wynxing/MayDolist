use crate::{
    error::{AppError, AppResult},
    models::{AppConfig, Note, WindowBounds},
    AppState,
};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn setup(app: &mut tauri::App) -> AppResult<()> {
    let handle = app.handle().clone();
    let config = app.state::<AppState>().storage.load_config()?;
    apply_hotkeys(&handle, &config)?;
    build_tray(&handle)?;
    if std::env::args().any(|v| v == "--autostart") {
        if let Some(window) = app.get_webview_window("main") {
            window.hide().ok();
        }
    }
    let notes = app.state::<AppState>().services.note.list(false)?;
    for note in notes.into_iter().filter(|v| v.floating) {
        show_note(&handle, &note, false)?;
    }
    spawn_hot_corner(handle);
    if let Some(main) = app.get_webview_window("main") {
        apply_acrylic(&main);
        let blur_window = main.clone();
        main.on_window_event(move |event| match event {
            tauri::WindowEvent::Focused(false) => {
                // Delay the hide and only act when focus did not move to one
                // of our own windows (floating note, etc.). The two-stage
                // check covers the window where a new webview is still
                // initializing: focus may briefly be nowhere during creation.
                let handle = blur_window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(300));
                    let focus_is_elsewhere = !handle
                        .webview_windows()
                        .values()
                        .any(|w| w.is_focused().unwrap_or(false));
                    if focus_is_elsewhere {
                        std::thread::sleep(Duration::from_millis(300));
                        let still_elsewhere = !handle
                            .webview_windows()
                            .values()
                            .any(|w| w.is_focused().unwrap_or(false));
                        if still_elsewhere {
                            if let Some(window) = handle.get_webview_window("main") {
                                window.hide().ok();
                            }
                        }
                    }
                });
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                blur_window.hide().ok();
            }
            _ => {}
        });
    }
    if let Some(quick_capture) = app.get_webview_window(QUICK_CAPTURE_WINDOW) {
        apply_acrylic(&quick_capture);
        let window = quick_capture.clone();
        quick_capture.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
            }
        });
    }
    spawn_github_refresh(app.handle().clone());
    Ok(())
}

/// Register the main panel and quick capture global hotkeys from the config.
/// Both shortcuts are parsed before anything is unregistered, so an invalid
/// value never disables a working hotkey. The quick capture hotkey is only
/// registered when `quick_capture_enabled` is true.
pub fn apply_hotkeys(app: &AppHandle, config: &AppConfig) -> AppResult<()> {
    let main_shortcut = Shortcut::from_str(&config.hotkey)
        .map_err(|e| AppError::InvalidInput(format!("invalid hotkey: {e}")))?;
    let quick_shortcut = if config.quick_capture_enabled {
        if config.quick_capture_hotkey.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "quick capture hotkey must not be empty".into(),
            ));
        }
        if config.quick_capture_hotkey.trim() == config.hotkey.trim() {
            return Err(AppError::InvalidInput(
                "quick capture hotkey conflicts with the main panel hotkey".into(),
            ));
        }
        Some(
            Shortcut::from_str(&config.quick_capture_hotkey)
                .map_err(|e| AppError::InvalidInput(format!("invalid hotkey: {e}")))?,
        )
    } else {
        None
    };
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(main_shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_main(&handle).ok();
            }
        })
        .map_err(|e| AppError::InvalidInput(format!("hotkey unavailable: {e}")))?;
    if let Some(shortcut) = quick_shortcut {
        let handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    toggle_quick_capture(&handle).ok();
                }
            })
            .map_err(|e| AppError::InvalidInput(format!("hotkey unavailable: {e}")))?;
    }
    Ok(())
}

fn build_tray(app: &AppHandle) -> AppResult<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示/隐藏 MayDolist", true, None::<&str>)
        .map_err(internal)?;
    let quick_capture = MenuItem::with_id(app, "quick-capture", "快速收集", true, None::<&str>)
        .map_err(internal)?;
    let new_note =
        MenuItem::with_id(app, "new-note", "新建便签", true, None::<&str>).map_err(internal)?;
    let refresh =
        MenuItem::with_id(app, "refresh", "刷新 GitHub", true, None::<&str>).map_err(internal)?;
    let settings =
        MenuItem::with_id(app, "settings", "设置", true, None::<&str>).map_err(internal)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>).map_err(internal)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &quick_capture,
            &new_note,
            &refresh,
            &settings,
            &quit,
        ],
    )
    .map_err(internal)?;
    let handle = app.clone();
    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("MayDolist")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "toggle" => {
                toggle_main(app).ok();
            }
            "quick-capture" => {
                show_quick_capture(app).ok();
            }
            "new-note" => {
                show_main(app).ok();
                app.emit("tray-action", "new-note").ok();
            }
            "refresh" => {
                app.emit("tray-action", "refresh-github").ok();
            }
            "settings" => {
                show_main(app).ok();
                app.emit("tray-action", "settings").ok();
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(&handle).ok();
            }
        })
        .build(app)
        .map_err(internal)?;
    Ok(())
}

pub fn show_main(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::NotFound("main window".into()))?;
    apply_acrylic(&window);
    window.show().map_err(internal)?;
    window.set_focus().map_err(internal)
}
pub fn hide_main(app: &AppHandle) -> AppResult<()> {
    app.get_webview_window("main")
        .ok_or_else(|| AppError::NotFound("main window".into()))?
        .hide()
        .map_err(internal)
}
pub fn toggle_main(app: &AppHandle) -> AppResult<()> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::NotFound("main window".into()))?;
    if w.is_visible().map_err(internal)? {
        w.hide().map_err(internal)
    } else {
        apply_acrylic(&w);
        w.show().map_err(internal)?;
        w.set_focus().map_err(internal)
    }
}

pub const QUICK_CAPTURE_WINDOW: &str = "quick-capture";

/// Show (and focus) the quick capture window, then tell the view to focus the
/// input. The window itself is declared hidden in tauri.conf.json so this only
/// ever reuses an already-created webview.
pub fn show_quick_capture(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(QUICK_CAPTURE_WINDOW)
        .ok_or_else(|| AppError::NotFound("quick capture window".into()))?;
    apply_acrylic(&window);
    window.show().map_err(internal)?;
    window.set_focus().map_err(internal)?;
    app.emit_to(QUICK_CAPTURE_WINDOW, "quick-capture-open", ())
        .map_err(internal)
}

pub fn hide_quick_capture(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(QUICK_CAPTURE_WINDOW) {
        window.hide().map_err(internal)?;
    }
    Ok(())
}

pub fn toggle_quick_capture(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(QUICK_CAPTURE_WINDOW)
        .ok_or_else(|| AppError::NotFound("quick capture window".into()))?;
    if window.is_visible().map_err(internal)? {
        window.hide().map_err(internal)
    } else {
        show_quick_capture(app)
    }
}

pub fn show_note(app: &AppHandle, note: &Note, focus_body: bool) -> AppResult<()> {
    let label = format!("note-{}", note.id);
    if let Some(w) = app.get_webview_window(&label) {
        apply_acrylic(&w);
        w.show().map_err(internal)?;
        return w.set_focus().map_err(internal);
    }
    let focus = if focus_body { "&focus=body" } else { "" };
    let url = WebviewUrl::App(format!("index.html?note={}{}", note.id, focus).into());
    let mut builder = WebviewWindowBuilder::new(app, &label, url)
        .title(&note.title)
        .inner_size(
            note.window_bounds
                .as_ref()
                .map(|v| v.width)
                .unwrap_or(360.0),
            note.window_bounds
                .as_ref()
                .map(|v| v.height)
                .unwrap_or(280.0),
        )
        .decorations(false)
        .always_on_top(note.always_on_top)
        .skip_taskbar(true)
        .transparent(true)
        .effects(acrylic_effects())
        .resizable(true);
    if let Some(v) = &note.window_bounds {
        builder = builder.position(v.x, v.y);
    }
    let window = builder.build().map_err(internal)?;
    apply_acrylic(&window);
    let note_id = note.id.clone();
    let initial_bounds = note.window_bounds.clone().unwrap_or(WindowBounds {
        x: 0.0,
        y: 0.0,
        width: 360.0,
        height: 280.0,
    });
    let tracker = Arc::new(Mutex::new(initial_bounds));
    let (bounds_tx, bounds_rx) = std::sync::mpsc::channel::<WindowBounds>();
    let writer = app.clone();
    let writer_note_id = note_id.clone();
    // Coalescing writer: a Moved/Resized event can fire every frame while the
    // user drags/resizes; queue bounds here and persist at most one write per
    // 250ms of activity, always flushing the final position/size.
    std::thread::spawn(move || {
        while let Ok(mut bounds) = bounds_rx.recv() {
            while let Ok(newer) = bounds_rx.try_recv() {
                bounds = newer;
            }
            std::thread::sleep(Duration::from_millis(250));
            while let Ok(newer) = bounds_rx.try_recv() {
                bounds = newer;
            }
            let patch = crate::services::note::NotePatch {
                window_bounds: Some(bounds),
                ..Default::default()
            };
            writer
                .state::<AppState>()
                .services
                .note
                .update(&writer_note_id, patch)
                .ok();
        }
    });
    let event_tracker = tracker.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(position) => {
            let mut bounds = event_tracker.lock().unwrap();
            if (bounds.x - position.x as f64).abs() < 1.0
                && (bounds.y - position.y as f64).abs() < 1.0
            {
                return;
            }
            bounds.x = position.x as f64;
            bounds.y = position.y as f64;
            let _ = bounds_tx.send(bounds.clone());
        }
        tauri::WindowEvent::Resized(size) => {
            let mut bounds = event_tracker.lock().unwrap();
            if (bounds.width - size.width as f64).abs() < 1.0
                && (bounds.height - size.height as f64).abs() < 1.0
            {
                return;
            }
            bounds.width = size.width as f64;
            bounds.height = size.height as f64;
            let _ = bounds_tx.send(bounds.clone());
        }
        _ => {}
    });
    Ok(())
}

fn acrylic_effects() -> tauri::utils::config::WindowEffectsConfig {
    EffectsBuilder::new().effect(Effect::Acrylic).build()
}

fn apply_acrylic(window: &WebviewWindow) {
    // Reapply at runtime as well as through tauri.conf.json. This covers
    // dynamically-created note windows and Windows sessions where the effect
    // is cleared while a transparent window is hidden and shown again.
    window.set_effects(acrylic_effects()).ok();
}

fn spawn_github_refresh(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let state = app.state::<AppState>();
        let config = match state.storage.load_config() {
            Ok(value) => value,
            Err(err) => {
                state
                    .log
                    .log("error", &format!("failed to load config: {err}"));
                continue;
            }
        };
        if config.github_refresh_interval_minutes == 0 {
            continue;
        }
        let minute = chrono::Utc::now().timestamp() / 60;
        if minute % i64::from(config.github_refresh_interval_minutes) == 0 {
            match state.services.github.refresh_all() {
                Ok(_) => {
                    crate::events::emit_entity_changed(&app, "github", "*", "background-refreshed")
                        .ok();
                }
                Err(err) => {
                    state
                        .log
                        .log("error", &format!("background github refresh failed: {err}"));
                }
            }
        }
    });
}

#[tauri::command]
pub fn app_show_main(app: AppHandle) -> AppResult<()> {
    show_main(&app)
}
#[tauri::command]
pub fn app_hide_main(app: AppHandle) -> AppResult<()> {
    hide_main(&app)
}
#[tauri::command]
pub fn app_quit(app: AppHandle) {
    app.exit(0)
}
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    let parsed = url::Url::parse(&url).map_err(|_| AppError::InvalidInput("invalid url".into()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput("unsupported url scheme".into()));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(internal)
}

fn spawn_hot_corner(app: AppHandle) {
    std::thread::spawn(move || {
        let mut entered = None;
        let mut armed = true;
        let mut config: Option<crate::models::AppConfig> = None;
        let mut last_config_load = std::time::Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(std::time::Instant::now);
        loop {
            let now = std::time::Instant::now();
            if now.duration_since(last_config_load) >= Duration::from_secs(1) {
                last_config_load = now;
                config = app.state::<AppState>().storage.load_config().ok();
            }
            let Some(cfg) = config.as_ref() else {
                std::thread::sleep(Duration::from_millis(250));
                continue;
            };
            if cfg.hot_corner == "off" {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let hit = hot_corner_hit(&cfg.hot_corner);
            if hit && armed {
                let since = entered.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= Duration::from_millis(350) {
                    show_main(&app).ok();
                    armed = false;
                }
            } else if !hit {
                entered = None;
                armed = true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}
#[cfg(windows)]
fn hot_corner_hit(corner: &str) -> bool {
    use windows::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST},
        UI::WindowsAndMessaging::GetCursorPos,
    };
    unsafe {
        let mut p = POINT::default();
        if GetCursorPos(&mut p).is_err() {
            return false;
        }
        let monitor = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let r = info.rcWork;
            match corner {
                "top-left" => p.x <= r.left + 8 && p.y <= r.top + 8,
                "top-right" => p.x >= r.right - 8 && p.y <= r.top + 8,
                "bottom-left" => p.x <= r.left + 8 && p.y >= r.bottom - 8,
                "bottom-right" => p.x >= r.right - 8 && p.y >= r.bottom - 8,
                _ => false,
            }
        } else {
            false
        }
    }
}
#[cfg(not(windows))]
fn hot_corner_hit(_: &str) -> bool {
    false
}
fn internal<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Internal(e.to_string())
}
