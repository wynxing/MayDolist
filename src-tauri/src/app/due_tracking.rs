//! Background loops: periodic GitHub refresh and the due-date tracking
//! (reminders + tray badge).

use crate::AppState;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use super::badge::update_tray_badge;
use super::windows::show_main;

pub(super) fn spawn_github_refresh(app: AppHandle) {
    std::thread::spawn(move || {
        let mut first_run = true;
        loop {
            if !first_run {
                std::thread::sleep(Duration::from_secs(60));
            }
            first_run = false;
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
                        if config.github_sync_enabled {
                            let summary = state.services.github.sync_linked_todos(
                                &state.services.todo,
                                config.github_auto_complete_todos,
                            );
                            for id in &summary.changed_item_ids {
                                let operation = if summary.auto_completed_item_ids.contains(id) {
                                    "auto-completed"
                                } else {
                                    "source-state-changed"
                                };
                                crate::events::emit_entity_changed(&app, "todoItem", id, operation)
                                    .ok();
                            }
                            crate::events::emit_entity_changed(
                                &app,
                                "github",
                                "*",
                                if summary.failed > 0 {
                                    "sync-failed"
                                } else {
                                    "status-synced"
                                },
                            )
                            .ok();
                        }
                        crate::events::emit_entity_changed(
                            &app,
                            "github",
                            "*",
                            "background-refreshed",
                        )
                        .ok();
                    }
                    Err(err) => {
                        state
                            .log
                            .log("error", &format!("background github refresh failed: {err}"));
                    }
                }
            }
        }
    });
}

/// Due-date background loop: every tick it (1) fires due reminders as Windows
/// toasts (silently degrading to the tray badge when the system blocks
/// notifications), and (2) refreshes the tray badge with the overdue count
/// (hidden when 0). All writes stay on the Rust side; the frontend only
/// receives the `focus-todo` event when a toast is clicked.
pub(super) fn spawn_due_tracking(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_overdue: Option<usize> = None;
        tick_due_tracking(&app, &mut last_overdue);
        loop {
            std::thread::sleep(Duration::from_secs(15));
            tick_due_tracking(&app, &mut last_overdue);
        }
    });
}

fn tick_due_tracking(app: &AppHandle, last_overdue: &mut Option<usize>) {
    let state = app.state::<AppState>();
    let lists = match state.services.todo.list(false) {
        Ok(lists) => lists,
        Err(err) => {
            state
                .log
                .log("error", &format!("due tracking failed: {err}"));
            return;
        }
    };
    let now = chrono::Utc::now();
    let quiet_hours = state
        .storage
        .load_config()
        .ok()
        .and_then(|config| config.quiet_hours);
    let local_now = chrono::Local::now();
    for due in crate::services::reminder::due_reminders(&lists, &now) {
        let quiet = quiet_hours
            .as_ref()
            .is_some_and(|window| window.contains(local_now.time()));
        if !quiet {
            show_reminder(app, &due);
            state
                .log
                .log("info", &format!("due reminder fired for {}", due.id));
        }
        if let Err(err) = state.services.todo.mark_reminded(&due.id, &due.remind_at) {
            state
                .log
                .log("error", &format!("failed to persist reminder state: {err}"));
        }
    }

    let overdue = crate::services::reminder::overdue_count(&lists, local_now.date_naive());
    if *last_overdue != Some(overdue) {
        update_tray_badge(app, overdue);
        *last_overdue = Some(overdue);
    }
}

/// Show a Windows toast for a due reminder. Errors (missing AUMID in dev,
/// notifications disabled, system interception) are logged and swallowed —
/// the tray badge remains the passive signal.
#[cfg(windows)]
fn show_reminder(app: &AppHandle, due: &crate::services::reminder::DueReminder) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tauri_winrt_notification::{Sound, Toast};

    let handle = app.clone();
    let click_handle = app.clone();
    let app_id = app.config().identifier.clone();
    let todo_id = due.id.clone();
    let title = due.title.clone();
    let list_title = due.list_title.clone();
    let due_label = due.due_date.clone().unwrap_or_default();
    std::thread::spawn(move || {
        // Keep the thread (and its COM apartment) alive until the toast is
        // activated or dismissed, otherwise the click callback never fires.
        let done = Arc::new(AtomicBool::new(false));
        let done_click = done.clone();
        let done_dismiss = done.clone();
        init_com_mta();
        let result = Toast::new(&app_id)
            .title("MayDolist 到期提醒")
            .text1(&title)
            .text2(&format!("来自「{list_title}」 · 截止 {due_label}"))
            .sound(Some(Sound::Reminder))
            .add_button("查看待办", &todo_id)
            .on_activated(move |action| {
                show_main(&click_handle).ok();
                if let Some(id) = action {
                    click_handle.emit("focus-todo", id).ok();
                }
                done_click.store(true, Ordering::Relaxed);
                Ok(())
            })
            .on_dismissed(move |_| {
                done_dismiss.store(true, Ordering::Relaxed);
                Ok(())
            })
            .show();
        if let Err(err) = result {
            handle
                .state::<AppState>()
                .log
                .log("info", &format!("toast suppressed: {err}"));
            return;
        }
        // Bounded wait (max 30s) so a dismissed/ignored toast never leaks a
        // thread forever.
        let mut waited = 0u32;
        while !done.load(Ordering::Relaxed) && waited < 300 {
            std::thread::sleep(Duration::from_millis(100));
            waited += 1;
        }
    });
}

#[cfg(windows)]
fn init_com_mta() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

#[cfg(not(windows))]
fn show_reminder(app: &AppHandle, due: &crate::services::reminder::DueReminder) {
    app.state::<AppState>().log.log(
        "info",
        &format!("reminder due (toast unavailable): {}", due.id),
    );
}
