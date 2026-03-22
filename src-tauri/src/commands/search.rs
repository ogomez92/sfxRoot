//! Search and query commands.

use tauri::State;

use crate::db::{QueryOptions, SearchRepository, SoundFile};
use crate::error::{Result, SfxError};
use crate::state::AppState;

/// Query sound files with filters and pagination.
#[tauri::command]
pub async fn viewer_query(
    options: QueryOptions,
    state: State<'_, AppState>,
) -> Result<Vec<SoundFile>> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = SearchRepository::new(db.conn());
    repo.search(&options)
}

/// Count sound files matching the query.
#[tauri::command]
pub async fn viewer_count(options: QueryOptions, state: State<'_, AppState>) -> Result<i64> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = SearchRepository::new(db.conn());
    repo.count(&options)
}

/// Find the index of the first file starting with a prefix (for keyboard navigation).
#[tauri::command]
pub async fn viewer_find_prefix_index(
    options: QueryOptions,
    prefix: String,
    state: State<'_, AppState>,
) -> Result<Option<i64>> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = SearchRepository::new(db.conn());
    repo.find_prefix_index(&options, &prefix)
}

/// Get file paths for clipboard copy (supports range selection).
#[tauri::command]
pub async fn viewer_get_paths(
    options: QueryOptions,
    from_index: Option<i64>,
    to_index: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<String>> {
    let db_guard = state.db.lock().unwrap();
    let db = db_guard.as_ref().ok_or(SfxError::DatabaseNotOpen)?;
    let repo = SearchRepository::new(db.conn());
    repo.get_paths(&options, from_index, to_index)
}
