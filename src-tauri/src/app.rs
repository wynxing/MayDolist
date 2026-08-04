use crate::{
    error::{AppError, AppResult},
    models::Note,
    AppState,
};
use std::str::FromStr;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn setup(app: &mut tauri::App) -> AppResult<()> {
    let handle = app.handle().clone();
    let config = app.state::<AppState>().storage.load_config()?;
    register_hotkey(&handle, &config.hotkey)?;
    build_tray(&handle)?;
    if std::env::args().any(|v| v == "--autostart") {
        if let Some(window) = app.get_webview_window("main") {
            window.hide().ok();
        }
    }
    let notes = app.state::<AppState>().services.note.list(false)?;
    for note in notes.into_iter().filter(|v| v.floating) {
        show_note(&handle, &note)?;
    }
    spawn_hot_corner(handle);
    if let Some(main) = app.get_webview_window("main") {
        let blur_window = main.clone();
        main.on_window_event(move |event| match event {
            tauri::WindowEvent::Focused(false) => {
                blur_window.hide().ok();
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                blur_window.hide().ok();
            }
            _ => {}
        });
    }
    spawn_github_refresh(app.handle().clone());
    Ok(())
}

pub fn register_hotkey(app: &AppHandle, value: &str) -> AppResult<()> {
    let shortcut = Shortcut::from_str(value)
        .map_err(|e| AppError::InvalidInput(format!("invalid hotkey: {e}")))?;
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_main(&handle).ok();
            }
        })
        .map_err(|e| AppError::InvalidInput(format!("hotkey unavailable: {e}")))?;
    Ok(())
}

fn build_tray(app: &AppHandle) -> AppResult<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示/隐藏 MayDolist", true, None::<&str>)
        .map_err(internal)?;
    let new_note =
        MenuItem::with_id(app, "new-note", "新建便签", true, None::<&str>).map_err(internal)?;
    let refresh =
        MenuItem::with_id(app, "refresh", "刷新 GitHub", true, None::<&str>).map_err(internal)?;
    let settings =
        MenuItem::with_id(app, "settings", "设置", true, None::<&str>).map_err(internal)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>).map_err(internal)?;
    let menu = Menu::with_items(app, &[&toggle, &new_note, &refresh, &settings, &quit])
        .map_err(internal)?;
    let handle = app.clone();
    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("MayDolist")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "toggle" => {
                toggle_main(app).ok();
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
        w.show().map_err(internal)?;
        w.set_focus().map_err(internal)
    }
}

pub fn show_note(app: &AppHandle, note: &Note) -> AppResult<()> {
    let label = format!("note-{}", note.id);
    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(internal)?;
        return w.set_focus().map_err(internal);
    }
    let url = WebviewUrl::App(format!("index.html?note={}", note.id).into());
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
        .resizable(true);
    if let Some(v) = &note.window_bounds {
        builder = builder.position(v.x, v.y);
    }
    let window = builder.build().map_err(internal)?;
    let note_id = note.id.clone();
    let handle = app.clone();
    window.on_window_event(move |event| {
        let patch = match event {
            tauri::WindowEvent::Moved(position) => {
                let current = handle.state::<AppState>().services.note.get(&note_id).ok();
                current.map(|note| crate::services::note::NotePatch {
                    window_bounds: Some(crate::models::WindowBounds {
                        x: position.x as f64,
                        y: position.y as f64,
                        width: note
                            .window_bounds
                            .as_ref()
                            .map(|v| v.width)
                            .unwrap_or(360.0),
                        height: note
                            .window_bounds
                            .as_ref()
                            .map(|v| v.height)
                            .unwrap_or(280.0),
                    }),
                    ..Default::default()
                })
            }
            tauri::WindowEvent::Resized(size) => {
                let current = handle.state::<AppState>().services.note.get(&note_id).ok();
                current.map(|note| crate::services::note::NotePatch {
                    window_bounds: Some(crate::models::WindowBounds {
                        x: note.window_bounds.as_ref().map(|v| v.x).unwrap_or(0.0),
                        y: note.window_bounds.as_ref().map(|v| v.y).unwrap_or(0.0),
                        width: size.width as f64,
                        height: size.height as f64,
                    }),
                    ..Default::default()
                })
            }
            _ => None,
        };
        if let Some(patch) = patch {
            handle
                .state::<AppState>()
                .services
                .note
                .update(&note_id, patch)
                .ok();
        }
    });
    Ok(())
}

fn spawn_github_refresh(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let state = app.state::<AppState>();
        let config = match state.storage.load_config() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if config.github_refresh_interval_minutes == 0 {
            continue;
        }
        let minute = chrono::Utc::now().timestamp() / 60;
        if minute % i64::from(config.github_refresh_interval_minutes) == 0
            && state.services.github.refresh_all().is_ok()
        {
            crate::events::emit_entity_changed(&app, "github", "*", "background-refreshed").ok();
        }
    });
}

#[tauri::command]
pub fn app_get_bootstrap(state: tauri::State<'_, AppState>) -> AppResult<serde_json::Value> {
    let config = state.storage.load_config()?;
    Ok(
        serde_json::json!({"config":config,"dataDir":state.storage.data_dir(),"version":env!("CARGO_PKG_VERSION"),"logDir":state.storage.data_dir().join("logs")}),
    )
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
        loop {
            std::thread::sleep(std::time::Duration::from_millis(80));
            let config = match app.state::<AppState>().storage.load_config() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if config.hot_corner == "off" {
                continue;
            }
            let hit = hot_corner_hit(&config.hot_corner);
            if hit && armed {
                let since = entered.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= std::time::Duration::from_millis(350) {
                    show_main(&app).ok();
                    armed = false;
                }
            } else if !hit {
                entered = None;
                armed = true;
            }
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
