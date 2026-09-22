//! Global hotkeys (main panel / quick capture) and the screen-corner hover
//! trigger.

use crate::{
    error::{AppError, AppResult},
    models::AppConfig,
    AppState,
};
use std::str::FromStr;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::windows::{show_main, toggle_main, toggle_quick_capture};

/// Register the main panel and quick capture global hotkeys from the config.
/// All shortcuts are parsed and conflict-checked before anything is
/// unregistered, so an invalid value never disables a working hotkey. The
/// optional shortcuts are only registered when their enable flag is true.
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
        .map_err(hotkey_unavailable)?;
    if let Some(shortcut) = quick_shortcut {
        let handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    toggle_quick_capture(&handle).ok();
                }
            })
            .map_err(hotkey_unavailable)?;
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
            let hit = hot_corner_hit(&app, &cfg.hot_corner);
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

fn hotkey_unavailable(err: impl std::fmt::Display) -> AppError {
    #[cfg(target_os = "macos")]
    {
        AppError::InvalidInput(format!(
            "快捷键不可用（{err}）。请在「系统设置 → 隐私与安全性 → 辅助功能」中允许 MayDolist"
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        AppError::InvalidInput(format!("hotkey unavailable: {err}"))
    }
}

fn hot_corner_hit(app: &AppHandle, corner: &str) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    let Ok(pos) = window.cursor_position() else {
        return false;
    };
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    monitors.iter().any(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        let left = origin.x as f64;
        let top = origin.y as f64;
        let right = left + f64::from(size.width);
        let bottom = top + f64::from(size.height);
        point_hits_corner(pos.x, pos.y, left, top, right, bottom, corner)
    })
}

fn point_hits_corner(
    x: f64,
    y: f64,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    corner: &str,
) -> bool {
    const PAD: f64 = 8.0;
    match corner {
        "top-left" => x <= left + PAD && y <= top + PAD,
        "top-right" => x >= right - PAD && y <= top + PAD,
        "bottom-left" => x <= left + PAD && y >= bottom - PAD,
        "bottom-right" => x >= right - PAD && y >= bottom - PAD,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::point_hits_corner;

    #[test]
    fn hits_each_corner_inside_pad() {
        assert!(point_hits_corner(
            2.0, 2.0, 0.0, 0.0, 1920.0, 1080.0, "top-left"
        ));
        assert!(point_hits_corner(
            1918.0,
            1.0,
            0.0,
            0.0,
            1920.0,
            1080.0,
            "top-right"
        ));
        assert!(point_hits_corner(
            1.0,
            1078.0,
            0.0,
            0.0,
            1920.0,
            1080.0,
            "bottom-left"
        ));
        assert!(point_hits_corner(
            1919.0,
            1079.0,
            0.0,
            0.0,
            1920.0,
            1080.0,
            "bottom-right"
        ));
    }

    #[test]
    fn ignores_center_and_unknown_corner() {
        assert!(!point_hits_corner(
            960.0,
            540.0,
            0.0,
            0.0,
            1920.0,
            1080.0,
            "top-right"
        ));
        assert!(!point_hits_corner(
            0.0, 0.0, 0.0, 0.0, 1920.0, 1080.0, "off"
        ));
    }
}
