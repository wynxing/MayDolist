//! App shell: window lifecycle, tray, hotkeys, hot corner, badge and
//! background loops. Split into focused submodules:
//!
//! - `windows`: main panel, quick capture, command palette and note windows
//! - `tray`: tray icon and its context menu
//! - `hotkeys`: global shortcuts and the screen-corner hover trigger
//! - `badge`: overdue tray badge rendering
//! - `due_tracking`: background loops (due reminders + GitHub refresh)

mod badge;
mod due_tracking;
mod hotkeys;
mod tray;
mod windows;

pub use hotkeys::apply_hotkeys;
pub use windows::{
    hide_command_palette, hide_main, hide_quick_capture, show_main, show_note, show_quick_capture,
    COMMAND_PALETTE_WINDOW, QUICK_CAPTURE_WINDOW,
};

use crate::{
    error::{AppError, AppResult},
    AppState,
};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use due_tracking::{spawn_due_tracking, spawn_github_refresh};
use hotkeys::spawn_hot_corner;
use tray::build_tray;
use windows::apply_acrylic;

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
    for note in notes.iter().filter(|v| v.floating) {
        show_note(&handle, note, false)?;
    }
    spawn_hot_corner(handle);
    spawn_due_tracking(app.handle().clone());
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
    if let Some(command_palette) = app.get_webview_window(COMMAND_PALETTE_WINDOW) {
        apply_acrylic(&command_palette);
        let window = command_palette.clone();
        command_palette.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
            }
        });
    }
    spawn_github_refresh(app.handle().clone());
    Ok(())
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

fn internal<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Internal(e.to_string())
}
