//! Folder scanning commands (without database).

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;

use crate::error::{Result, SfxError};
use crate::indexing::{extract_batch, scan_directory};

/// Scanned file with metadata for folder mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderScanFile {
    pub filename: String,
    pub relative_path: String,
    pub full_path: String,
    pub extension: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bit_rate: Option<i64>,
    pub codec: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
}

/// Progress for folder scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderScanProgress {
    pub scanned: usize,
    pub current_file: String,
}

/// Browse for a folder to scan.
#[tauri::command]
pub async fn folder_browse(app: AppHandle) -> Result<Option<String>> {
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

/// Scan a folder without saving to database.
#[tauri::command]
pub async fn folder_scan(app: AppHandle, path: String) -> Result<()> {
    let app_clone = app.clone();

    tokio::task::spawn_blocking(move || {
        let folder_path = Path::new(&path);

        // Scan directory
        let scanned_files = scan_directory(folder_path);

        if scanned_files.is_empty() {
            let _ = app_clone.emit("folder:complete", Vec::<FolderScanFile>::new());
            return;
        }

        // Extract metadata
        let paths: Vec<String> = scanned_files
            .iter()
            .map(|f| f.full_path.to_string_lossy().into_owned())
            .collect();

        let metadata_results = extract_batch(&paths, Some(|current: usize, _total: usize, file: &str| {
            let _ = app_clone.emit(
                "folder:progress",
                FolderScanProgress {
                    scanned: current,
                    current_file: file.to_string(),
                },
            );
        }));

        // Build result
        let files: Vec<FolderScanFile> = scanned_files
            .iter()
            .zip(metadata_results.iter())
            .map(|(scanned, meta_result)| {
                let meta = meta_result.metadata.as_ref();
                FolderScanFile {
                    filename: scanned.filename.clone(),
                    relative_path: scanned.relative_path.clone(),
                    full_path: scanned.full_path.to_string_lossy().into_owned(),
                    extension: scanned.extension.clone(),
                    file_size: scanned.file_size as i64,
                    modified_at: scanned.modified_at,
                    duration_ms: meta.and_then(|m| m.duration_ms),
                    sample_rate: meta.and_then(|m| m.sample_rate),
                    channels: meta.and_then(|m| m.channels),
                    bit_rate: meta.and_then(|m| m.bit_rate),
                    codec: meta.and_then(|m| m.codec.clone()),
                    title: meta.and_then(|m| m.title.clone()),
                    artist: meta.and_then(|m| m.artist.clone()),
                    album: meta.and_then(|m| m.album.clone()),
                    genre: meta.and_then(|m| m.genre.clone()),
                    comment: meta.and_then(|m| m.comment.clone()),
                }
            })
            .collect();

        let _ = app_clone.emit("folder:complete", &files);
    });

    Ok(())
}
