//! Indexing commands with progress events.

use tauri::{AppHandle, Emitter, State};

use crate::db::DatabaseConnection;
use crate::error::{Result, SfxError};
use crate::indexing::{cancel_indexing, IndexingResult, IndexingService};
use crate::state::AppState;

/// Start indexing a new directory.
#[tauri::command]
pub async fn indexing_start(
    app: AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> Result<IndexingResult> {
    // Clone what we need before spawning
    let cancel_flag = state.indexing_cancel.clone();
    let app_clone = app.clone();

    // Get database path
    let db_path = state
        .get_db_path()
        .ok_or(SfxError::DatabaseNotOpen)?;

    // Run indexing in a blocking task
    let result = tokio::task::spawn_blocking(move || {
        // Open a new connection for this thread
        let db = DatabaseConnection::open(&db_path)?;
        let service = IndexingService::new(cancel_flag);

        service.index_directory(&db, &path, |progress| {
            let _ = app_clone.emit("indexing:progress", &progress);
        })
    })
    .await
    .map_err(|e| SfxError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))??;

    // Emit completion
    let _ = app.emit("indexing:complete", &result);

    Ok(result)
}

/// Resync an existing directory.
#[tauri::command]
pub async fn indexing_resync(
    app: AppHandle,
    directory_id: i64,
    state: State<'_, AppState>,
) -> Result<IndexingResult> {
    let cancel_flag = state.indexing_cancel.clone();
    let app_clone = app.clone();

    let db_path = state
        .get_db_path()
        .ok_or(SfxError::DatabaseNotOpen)?;

    let result = tokio::task::spawn_blocking(move || {
        let db = DatabaseConnection::open(&db_path)?;
        let service = IndexingService::new(cancel_flag);

        service.resync_directory(&db, directory_id, |progress| {
            let _ = app_clone.emit("indexing:progress", &progress);
        })
    })
    .await
    .map_err(|e| SfxError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))??;

    let _ = app.emit("indexing:complete", &result);

    Ok(result)
}

/// Cancel ongoing indexing operation.
#[tauri::command]
pub async fn indexing_cancel(state: State<'_, AppState>) -> Result<()> {
    cancel_indexing(&state.indexing_cancel);
    Ok(())
}
