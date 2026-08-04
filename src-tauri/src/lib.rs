mod app;
mod commands;
mod error;
mod events;
mod models;
mod services;
mod storage;

use std::sync::Arc;

use tauri::Manager;

use services::Services;
use storage::Storage;

/// Shared application state: real storage layer + mock domain services.
pub struct AppState {
    pub storage: Arc<Storage>,
    pub services: Services,
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

    tauri::Builder::default()
        .setup(|tauri_app| {
            app::setup(tauri_app)?;
            let state = tauri_app.state::<AppState>();
            eprintln!(
                "[MayDolist] setup ok, data dir: {}",
                state.storage.data_dir().display()
            );
            Ok(())
        })
        .manage(AppState {
            storage,
            services: Services::mock(),
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::get_data_dir,
            commands::config::set_config,
            commands::todo::todo_list,
            commands::todo::todo_create_list,
            commands::todo::todo_create_item,
            commands::todo::todo_update_item,
            commands::todo::todo_soft_delete,
            commands::note::note_list,
            commands::note::note_create,
            commands::note::note_update,
            commands::snippet::snippet_list,
            commands::snippet::snippet_create,
            commands::snippet::snippet_update,
            commands::snippet::snippet_delete,
            commands::github::github_auth_status,
            commands::github::github_watchlist,
            commands::github::github_watch_add,
            commands::github::github_watch_remove,
            commands::github::github_refresh,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
