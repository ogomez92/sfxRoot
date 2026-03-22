//! Database management commands.

use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::db::DatabaseConnection;
use crate::error::{Result, SfxError};
use crate::state::AppState;

/// Browse for a database file to open.
#[tauri::command]
pub async fn db_browse(app: AppHandle) -> Result<Option<String>> {
    let file = app
        .dialog()
        .file()
        .add_filter("SQLite Database", &["db", "sqlite", "sqlite3"])
        .blocking_pick_file();

    match file {
        Some(file_path) => {
            let path = file_path
                .into_path()
                .map_err(|e| SfxError::InvalidPath(e.to_string()))?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// Create a new database file.
#[tauri::command]
pub async fn db_create(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>> {
    let file = app
        .dialog()
        .file()
        .add_filter("SQLite Database", &["db"])
        .set_file_name("sounds.db")
        .blocking_save_file();

    match file {
        Some(file_path) => {
            let path = file_path
                .into_path()
                .map_err(|e| SfxError::InvalidPath(e.to_string()))?;
            let db = DatabaseConnection::create(&path)?;
            *state.db.lock().unwrap() = Some(db);
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// Open an existing database file.
#[tauri::command]
pub async fn db_open(path: String, state: State<'_, AppState>) -> Result<()> {
    let path_buf = PathBuf::from(&path);
    let db = DatabaseConnection::open(&path_buf)?;
    *state.db.lock().unwrap() = Some(db);
    Ok(())
}

/// Close the current database.
#[tauri::command]
pub async fn db_close(state: State<'_, AppState>) -> Result<()> {
    *state.db.lock().unwrap() = None;
    Ok(())
}

/// Check if a database is currently open.
#[tauri::command]
pub async fn db_is_open(state: State<'_, AppState>) -> Result<bool> {
    Ok(state.is_db_open())
}
