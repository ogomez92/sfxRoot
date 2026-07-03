//! Directory management commands.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::db::{Directory, DirectoryRepository, SoundFileRepository};
use crate::error::{Result, SfxError};
use crate::state::AppState;

/// A directory whose indexing was interrupted and can be resumed, along with
/// the number of files already indexed so far.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncompleteDirectory {
    pub id: i64,
    pub path: String,
    pub indexed_count: i64,
}

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

/// List directories whose indexing was interrupted and can be resumed.
///
/// Each entry includes how many files are already indexed, so the UI can offer
/// to finish the job when a database is opened.
#[tauri::command]
pub async fn directories_incomplete(
    state: State<'_, AppState>,
) -> Result<Vec<IncompleteDirectory>> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let dir_repo = DirectoryRepository::new(db.conn());
    let file_repo = SoundFileRepository::new(db.conn());

    let mut incomplete = Vec::new();
    for dir in dir_repo.list_incomplete()? {
        let indexed_count = file_repo.count_by_directory(dir.id)?;
        incomplete.push(IncompleteDirectory {
            id: dir.id,
            path: dir.path,
            indexed_count,
        });
    }

    Ok(incomplete)
}
