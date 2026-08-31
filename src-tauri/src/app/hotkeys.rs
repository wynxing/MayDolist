//! Global hotkeys (main panel / quick capture / command palette) and the
//! screen-corner hover trigger.

use crate::{
    error::{AppError, AppResult},
    models::AppConfig,
    AppState,
};
use std::str::FromStr;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::windows::{show_main, toggle_command_palette, toggle_main, toggle_quick_capture};

/// Register the main panel, quick capture and command palette global hotkeys
/// from the config. All shortcuts are parsed and conflict-checked before
/// anything is unregistered, so an invalid value never disables a working
/// hotkey. The optional shortcuts are only registered when their enable flag
/// is true.
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
    let palette_shortcut = if config.command_palette_enabled {
        if config.command_palette_hotkey.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "command palette hotkey must not be empty".into(),
            ));
        }
        if config.command_palette_hotkey.trim() == config.hotkey.trim() {
            return Err(AppError::InvalidInput(
                "command palette hotkey conflicts with the main panel hotkey".into(),
            ));
        }
        if config.quick_capture_enabled
            && config.command_palette_hotkey.trim() == config.quick_capture_hotkey.trim()
        {
            return Err(AppError::InvalidInput(
                "command palette hotkey conflicts with the quick capture hotkey".into(),
            ));
        }
        Some(
            Shortcut::from_str(&config.command_palette_hotkey)
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
    if let Some(shortcut) = palette_shortcut {
        let handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    toggle_command_palette(&handle).ok();
                }
            })
            .map_err(|e| AppError::InvalidInput(format!("hotkey unavailable: {e}")))?;
    }
    Ok(())
}

pub(super) fn spawn_hot_corner(app: AppHandle) {
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
