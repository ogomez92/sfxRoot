//! Directory management commands.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::db::{Directory, DirectoryRepository};
use crate::error::{Result, SfxError};
use crate::state::AppState;

/// Browse for a directory to add.
#[tauri::command]
pub async fn directories_browse(app: AppHandle) -> Result<Option<String>> {
    let folder = app.dialog().file().blocking_pick_folder();
    match folder {
        Some(file_path) => {
            let path = file_path
                .into_path()
                .map_err(|e| SfxError::InvalidPath(e.to_string()))?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// List all indexed directories.
#[tauri::command]
pub async fn directories_list(state: State<'_, AppState>) -> Result<Vec<Directory>> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = DirectoryRepository::new(db.conn());
    repo.list()
}

/// Add a new directory (without indexing - just register it).
#[tauri::command]
pub async fn directories_add(path: String, state: State<'_, AppState>) -> Result<Directory> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = DirectoryRepository::new(db.conn());
    repo.add(&path)
}

/// Remove a directory from the database.
#[tauri::command]
pub async fn directories_remove(id: i64, state: State<'_, AppState>) -> Result<()> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = DirectoryRepository::new(db.conn());
    repo.remove(id)
}

/// Get a directory by ID.
#[tauri::command]
pub async fn directories_get(id: i64, state: State<'_, AppState>) -> Result<Option<Directory>> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = DirectoryRepository::new(db.conn());
    repo.get_by_id(id)
}
