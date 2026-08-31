//! Tray icon and its context menu.

use crate::error::AppResult;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};

use super::internal;
use super::windows::{show_main, show_quick_capture, toggle_main};

pub(super) fn build_tray(app: &AppHandle) -> AppResult<()> {
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
    TrayIconBuilder::with_id("main")
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
                show_main(app).ok();
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
