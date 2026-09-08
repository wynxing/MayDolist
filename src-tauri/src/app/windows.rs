//! Window lifecycle: main panel, quick capture and the dynamically-created
//! floating note windows, plus the shared acrylic effect.

use crate::{
    error::{AppError, AppResult},
    models::{Note, WindowBounds},
    AppState,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
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

const DEFAULT_NOTE_WIDTH: f64 = 360.0;
const DEFAULT_NOTE_HEIGHT: f64 = 280.0;

/// Physical pixel rectangle used for monitor intersection checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhysicalRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// True when `bounds` (physical pixels) intersects any monitor rectangle.
/// Invalid / non-finite sizes are treated as not visible so we recenter.
fn bounds_visible_on_any_monitor(bounds: &WindowBounds, monitors: &[PhysicalRect]) -> bool {
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || bounds.width < 1.0
        || bounds.height < 1.0
    {
        return false;
    }
    let win = PhysicalRect {
        x: bounds.x.round() as i32,
        y: bounds.y.round() as i32,
        width: bounds.width.round() as u32,
        height: bounds.height.round() as u32,
    };
    monitors
        .iter()
        .any(|monitor| rects_intersect(&win, monitor))
}

fn rects_intersect(a: &PhysicalRect, b: &PhysicalRect) -> bool {
    let a_x2 = a.x as i64 + a.width as i64;
    let a_y2 = a.y as i64 + a.height as i64;
    let b_x2 = b.x as i64 + b.width as i64;
    let b_y2 = b.y as i64 + b.height as i64;
    (a.x as i64) < b_x2 && a_x2 > b.x as i64 && (a.y as i64) < b_y2 && a_y2 > b.y as i64
}

fn monitor_rects(window: &WebviewWindow) -> Vec<PhysicalRect> {
    window
        .available_monitors()
        .ok()
        .into_iter()
        .flatten()
        .map(|monitor| {
            let pos = monitor.position();
            let size = monitor.size();
            PhysicalRect {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

fn physical_size_u32(value: f64, fallback: u32) -> u32 {
    if !value.is_finite() || value < 1.0 {
        return fallback;
    }
    value.min(u32::MAX as f64).round() as u32
}

/// Restore a note window from saved physical bounds, or center on the cursor
/// monitor when bounds are missing or off every current display.
fn restore_or_center_note(window: &WebviewWindow, bounds: Option<&WindowBounds>) -> AppResult<()> {
    if let Some(bounds) = bounds {
        let monitors = monitor_rects(window);
        if bounds_visible_on_any_monitor(bounds, &monitors) {
            window
                .set_size(Size::Physical(PhysicalSize::new(
                    physical_size_u32(bounds.width, DEFAULT_NOTE_WIDTH as u32),
                    physical_size_u32(bounds.height, DEFAULT_NOTE_HEIGHT as u32),
                )))
                .map_err(internal)?;
            window
                .set_position(Position::Physical(PhysicalPosition::new(
                    bounds.x.round() as i32,
                    bounds.y.round() as i32,
                )))
                .map_err(internal)?;
            return Ok(());
        }
    }
    center_on_cursor_or_primary(window)
}

fn bring_note_forward(window: &WebviewWindow) -> AppResult<()> {
    apply_acrylic(window);
    window.show().map_err(internal)?;
    window.set_focus().map_err(internal)
}

/// Center a window on the monitor containing the cursor, falling back to the
/// primary monitor when the cursor is not on any known monitor (e.g. a
/// monitor was unplugged). Cursor, monitor and window sizes are all reported
/// in physical pixels, so the math is done without DPI conversion.
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
    let x = origin.x + (size.width as i32 - outer.width as i32) / 2;
    let y = origin.y + (size.height as i32 - outer.height as i32) / 2;
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(internal)
}

pub fn show_note(app: &AppHandle, note: &Note, focus_body: bool) -> AppResult<()> {
    let label = format!("note-{}", note.id);
    if let Some(window) = app.get_webview_window(&label) {
        if let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) {
            let current = WindowBounds {
                x: pos.x as f64,
                y: pos.y as f64,
                width: size.width as f64,
                height: size.height as f64,
            };
            if !bounds_visible_on_any_monitor(&current, &monitor_rects(&window)) {
                center_on_cursor_or_primary(&window)?;
            }
        }
        return bring_note_forward(&window);
    }
    let focus = if focus_body { "&focus=body" } else { "" };
    let url = WebviewUrl::App(format!("index.html?note={}{}", note.id, focus).into());
    // Builder size APIs take logical pixels. Saved note bounds are physical
    // (from Moved/Resized), so always start from the default logical size and
    // apply physical geometry after the window exists.
    let window = WebviewWindowBuilder::new(app, &label, url)
        .title(&note.title)
        .inner_size(DEFAULT_NOTE_WIDTH, DEFAULT_NOTE_HEIGHT)
        .decorations(false)
        .always_on_top(note.always_on_top)
        .skip_taskbar(true)
        .transparent(true)
        .effects(acrylic_effects())
        .resizable(true)
        .build()
        .map_err(internal)?;
    restore_or_center_note(&window, note.window_bounds.as_ref())?;
    bring_note_forward(&window)?;
    let initial_bounds = match (window.outer_position(), window.inner_size()) {
        (Ok(pos), Ok(size)) => WindowBounds {
            x: pos.x as f64,
            y: pos.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        },
        _ => note.window_bounds.clone().unwrap_or(WindowBounds {
            x: 0.0,
            y: 0.0,
            width: DEFAULT_NOTE_WIDTH,
            height: DEFAULT_NOTE_HEIGHT,
        }),
    };
    let tracker = Arc::new(Mutex::new(initial_bounds));
    let (bounds_tx, bounds_rx) = std::sync::mpsc::channel::<WindowBounds>();
    let writer = app.clone();
    let writer_note_id = note.id.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> PhysicalRect {
        PhysicalRect {
            x,
            y,
            width,
            height,
        }
    }

    fn bounds(x: f64, y: f64, width: f64, height: f64) -> WindowBounds {
        WindowBounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn visible_when_fully_inside_primary() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert!(bounds_visible_on_any_monitor(
            &bounds(100.0, 80.0, 360.0, 280.0),
            &monitors
        ));
    }

    #[test]
    fn hidden_when_negative_and_no_left_monitor() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert!(!bounds_visible_on_any_monitor(
            &bounds(-800.0, 100.0, 360.0, 280.0),
            &monitors
        ));
    }

    #[test]
    fn visible_on_left_monitor_with_negative_origin() {
        let monitors = [monitor(-1920, 0, 1920, 1080), monitor(0, 0, 1920, 1080)];
        assert!(bounds_visible_on_any_monitor(
            &bounds(-1600.0, 200.0, 360.0, 280.0),
            &monitors
        ));
    }

    #[test]
    fn hidden_when_on_unplugged_display() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert!(!bounds_visible_on_any_monitor(
            &bounds(2800.0, 120.0, 360.0, 280.0),
            &monitors
        ));
    }

    #[test]
    fn hidden_when_size_invalid() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert!(!bounds_visible_on_any_monitor(
            &bounds(100.0, 100.0, 0.0, 280.0),
            &monitors
        ));
    }

    #[test]
    fn visible_when_partially_overlapping_edge() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert!(bounds_visible_on_any_monitor(
            &bounds(1800.0, 100.0, 360.0, 280.0),
            &monitors
        ));
    }
}
