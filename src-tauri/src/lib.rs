mod app;
mod commands;
mod error;
mod events;
mod logging;
mod models;
mod services;
mod storage;

use std::sync::Arc;

use tauri::Manager;

use services::Services;
use storage::Storage;

/// Shared application state: storage and persistent domain services.
pub struct AppState {
    pub storage: Arc<Storage>,
    pub services: Services,
    pub log: logging::Logger,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let storage = match Storage::new() {
        Ok(storage) => Arc::new(storage),
        Err(err) => {
            eprintln!("[MayDolist] failed to initialize storage: {err}");
            std::process::exit(1);
        }
    };
    let log = logging::Logger::new(storage.data_dir().join("logs"));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app::show_main(app).ok();
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|tauri_app| {
            app::setup(tauri_app)?;
            let state = tauri_app.state::<AppState>();
            let message = format!(
                "[MayDolist] setup ok, data dir: {}",
                state.storage.data_dir().display()
            );
            state.log.log("info", &message);
            eprintln!("{message}");
            Ok(())
        })
        .manage(AppState {
            storage: storage.clone(),
            services: Services::new(storage),
            log,
        })
        .invoke_handler(tauri::generate_handler![
            commands::backup::backup_export,
            commands::backup::backup_inspect,
            commands::backup::backup_import,
            commands::backup::backup_create,
            commands::backup::backup_list,
            commands::backup::backup_open_data_dir,
            commands::config::get_config,
            commands::config::get_data_dir,
            commands::config::set_config,
            app::app_show_main,
            app::app_hide_main,
            app::app_quit,
            app::open_external,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::settings::settings_migrate_data_dir,
            commands::settings::settings_set_autostart,
            commands::todo::todo_list,
            commands::todo::todo_create_list,
            commands::todo::todo_update_list,
            commands::todo::todo_reorder_lists,
            commands::todo::todo_create_item,
            commands::todo::todo_create_from_github,
            commands::todo::todo_update_item,
            commands::todo::todo_move_item,
            commands::todo::todo_reorder_items,
            commands::todo::todo_soft_delete,
            commands::note::note_list,
            commands::note::note_get,
            commands::note::note_create,
            commands::note::note_update,
            commands::note::note_soft_delete,
            commands::note::note_show_floating,
            commands::note::note_dock,
            commands::note::note_update_window_state,
            commands::quick_capture::quick_capture_submit,
            commands::quick_capture::quick_capture_hide,
            commands::trash::trash_list,
            commands::trash::trash_restore,
            commands::trash::trash_delete_permanently,
            commands::trash::trash_clear,
            commands::github::github_status,
            commands::github::github_watchlist,
            commands::github::github_watch_add,
            commands::github::github_watch_remove,
            commands::github::github_watch_filters,
            commands::github::github_watch_signal_filters,
            commands::github::github_watch_collapsed,
            commands::github::github_ignore_item,
            commands::github::github_pin_item,
            commands::github::github_unpin_item,
            commands::github::github_refresh_repo,
            commands::github::github_refresh_all,
            commands::github::github_get_snapshot,
            commands::focus::focus_overview,
            commands::update::update_runtime_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
