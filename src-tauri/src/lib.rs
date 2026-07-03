//! SFX Root - Sound Effects Library Manager
//!
//! Tauri backend for the SFX Root application.

pub mod commands;
pub mod db;
pub mod error;
pub mod indexing;
pub mod mcp_server;
pub mod state;

use tauri::Manager;

use commands::mcp::McpState;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Plugins
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // Application state
        .manage(AppState::new())
        .manage(McpState::new())
        // Command handlers
        .invoke_handler(tauri::generate_handler![
            // Database commands
            commands::db_browse,
            commands::db_create,
            commands::db_open,
            commands::db_close,
            commands::db_is_open,
            // Directory commands
            commands::directories_browse,
            commands::directories_list,
            commands::directories_add,
            commands::directories_remove,
            commands::directories_get,
            commands::directories_incomplete,
            // Indexing commands
            commands::indexing_start,
            commands::indexing_resync,
            commands::indexing_resume,
            commands::indexing_cancel,
            // Search commands
            commands::viewer_query,
            commands::viewer_count,
            commands::viewer_find_prefix_index,
            commands::viewer_get_paths,
            // Folder scan commands
            commands::folder_browse,
            commands::folder_scan,
            // MCP server commands
            commands::mcp_start,
            commands::mcp_stop,
            commands::mcp_status,
            commands::mcp_get_db_path,
            commands::mcp_save_skill,
            // System commands
            commands::open_in_explorer,
            commands::copy_to_clipboard,
        ])
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Stop the in-process MCP server when the window closes
                if let Some(mcp_state) = _window.try_state::<McpState>() {
                    mcp_state.shutdown();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
