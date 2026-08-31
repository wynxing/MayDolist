//! Window lifecycle: main panel, quick capture, command palette and the
//! dynamically-created floating note windows, plus the shared acrylic effect.

use crate::{
    error::{AppError, AppResult},
    models::{Note, WindowBounds},
    AppState,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use super::internal;

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

pub const COMMAND_PALETTE_WINDOW: &str = "command-palette";

/// Show (and focus) the command palette window, centered on the monitor that
/// currently contains the cursor (falling back to the primary monitor), then
/// tell the view to focus and select its input. The window is declared hidden
/// in tauri.conf.json so this only ever reuses an already-created webview.
pub fn show_command_palette(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(COMMAND_PALETTE_WINDOW)
        .ok_or_else(|| AppError::NotFound("command palette window".into()))?;
    apply_acrylic(&window);
    center_on_cursor_or_primary(&window)?;
    window.show().map_err(internal)?;
    window.set_focus().map_err(internal)?;
    app.emit_to(COMMAND_PALETTE_WINDOW, "command-palette-open", ())
        .map_err(internal)
}

pub fn hide_command_palette(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(COMMAND_PALETTE_WINDOW) {
        window.hide().map_err(internal)?;
    }
    Ok(())
}

pub fn toggle_command_palette(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(COMMAND_PALETTE_WINDOW)
        .ok_or_else(|| AppError::NotFound("command palette window".into()))?;
    if window.is_visible().map_err(internal)? {
        window.hide().map_err(internal)
    } else {
        show_command_palette(app)
    }
}

/// Center the palette window on the monitor containing the cursor, falling
/// back to the primary monitor when the cursor is not on any known monitor
/// (e.g. a monitor was unplugged). Cursor, monitor and window sizes are all
/// reported in physical pixels, so the math is done without DPI conversion.
fn center_on_cursor_or_primary(window: &WebviewWindow) -> AppResult<()> {
    let cursor = window.cursor_position().ok();
    let target = cursor
        .and_then(|position| {
            window
                .monitor_from_point(position.x, position.y)
                .ok()
                .flatten()
        })
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| AppError::NotFound("no monitor available".into()))?;
    let origin = target.position();
    let size = target.size();
    let outer = window.outer_size().map_err(internal)?;
    let x = (origin.x + (size.width as i32 - outer.width as i32) / 2).max(0);
    let y = (origin.y + (size.height as i32 - outer.height as i32) / 2).max(0);
    window
        .set_position(tauri::Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(internal)
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

pub(super) fn apply_acrylic(window: &WebviewWindow) {
    // Reapply at runtime as well as through tauri.conf.json. This covers
    // dynamically-created note windows and Windows sessions where the effect
    // is cleared while a transparent window is hidden and shown again.
    window.set_effects(acrylic_effects()).ok();
}
