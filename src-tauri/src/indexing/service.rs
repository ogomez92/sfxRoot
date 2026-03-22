//! Indexing service for orchestrating directory scanning and metadata extraction.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::{
    DatabaseConnection, DirectoryRepository, SoundFile, SoundFileInsert, SoundFileRepository,
    SoundFileUpdate,
};
use crate::error::{Result, SfxError};

use super::metadata::extract_batch;
use super::progress::{IndexingProgress, IndexingResult, ResyncStats};
use super::scanner::{scan_directory_with_progress, ScannedFile};

/// Batch size for database operations.
const BATCH_SIZE: usize = 1000;

/// Service for indexing audio files from directories.
pub struct IndexingService {
    cancel_flag: Arc<AtomicBool>,
}

impl IndexingService {
    /// Create a new IndexingService.
    pub fn new(cancel_flag: Arc<AtomicBool>) -> Self {
        Self { cancel_flag }
    }

    /// Check if indexing has been cancelled.
    fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    /// Index a new directory.
    pub fn index_directory<F>(
        &self,
        db: &DatabaseConnection,
        directory_path: &str,
        progress_callback: F,
    ) -> Result<IndexingResult>
    where
        F: Fn(IndexingProgress) + Send + Sync,
    {
        self.cancel_flag.store(false, Ordering::SeqCst);

        // Verify directory exists
        let path = Path::new(directory_path);
        if !path.exists() || !path.is_dir() {
            return Err(SfxError::DirectoryNotFound(directory_path.to_string()));
        }

        // Phase 1: Scanning with progress (discovering + preparing)
        progress_callback(IndexingProgress {
            phase: "discovering".to_string(),
            current: 0,
            total: 0,
            current_file: None,
            stats: None,
        });

        let scanned_files = scan_directory_with_progress(path, Some(|phase: &str, current: usize, total: usize, file: &str| {
            progress_callback(IndexingProgress {
                phase: phase.to_string(),
                current,
                total,
                current_file: if file.is_empty() { None } else { Some(file.to_string()) },
                stats: None,
            });
        }));

        // Add directory to database
        let dir_repo = DirectoryRepository::new(db.conn());
        let directory = dir_repo.add(directory_path)?;

        if scanned_files.is_empty() {
            dir_repo.update_last_synced_at(directory.id, Self::now())?;
            return Ok(IndexingResult::new(directory.id, 0, 0, 0));
        }

        if self.is_cancelled() {
            return Ok(IndexingResult::cancelled(directory.id, 0));
        }

        // Phase 2: Extract metadata and save in batches
        let total = scanned_files.len();
        let file_repo = SoundFileRepository::new(db.conn());
        let mut error_count = 0;
        let mut processed_count = 0;

        // Emit initial extracting progress immediately
        progress_callback(IndexingProgress {
            phase: "extracting".to_string(),
            current: 0,
            total,
            current_file: None,
            stats: None,
        });

        for (batch_idx, batch) in scanned_files.chunks(BATCH_SIZE).enumerate() {
            if self.is_cancelled() {
                return Ok(IndexingResult::cancelled(directory.id, processed_count));
            }

            let batch_start = batch_idx * BATCH_SIZE;

            // Emit progress at batch start
            progress_callback(IndexingProgress {
                phase: "extracting".to_string(),
                current: batch_start,
                total,
                current_file: Some(format!("Batch {} of {}", batch_idx + 1, (total + BATCH_SIZE - 1) / BATCH_SIZE)),
                stats: None,
            });

            // Extract metadata for this batch
            let paths: Vec<String> = batch.iter().map(|f| f.full_path.to_string_lossy().into_owned()).collect();

            let metadata_results = extract_batch(&paths, Some(|current: usize, _total: usize, file: &str| {
                progress_callback(IndexingProgress {
                    phase: "extracting".to_string(),
                    current: batch_start + current,
                    total,
                    current_file: Some(file.to_string()),
                    stats: None,
                });
            }));

            if self.is_cancelled() {
                return Ok(IndexingResult::cancelled(directory.id, processed_count));
            }

            // Build sound file records
            let sound_files: Vec<SoundFileInsert> = batch
                .iter()
                .zip(metadata_results.iter())
                .map(|(scanned, meta_result)| {
                    if meta_result.error.is_some() {
                        error_count += 1;
                    }
                    let meta = meta_result.metadata.as_ref();

                    SoundFileInsert {
                        directory_id: directory.id,
                        relative_path: scanned.relative_path.clone(),
                        filename: scanned.filename.clone(),
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

            // Save batch
            progress_callback(IndexingProgress {
                phase: "saving".to_string(),
                current: batch_start + batch.len(),
                total,
                current_file: None,
                stats: None,
            });

            file_repo.insert_batch(&sound_files)?;
            processed_count += sound_files.len();

            // Checkpoint every 10 batches (10,000 files) to balance durability vs performance
            if (batch_idx + 1) % 10 == 0 {
                db.checkpoint()?;
            }
        }

        // Final checkpoint before cleanup
        db.checkpoint()?;

        // Update directory stats
        dir_repo.update_file_count(directory.id, total as i64)?;
        dir_repo.update_last_synced_at(directory.id, Self::now())?;

        // Clean up WAL/SHM files (non-fatal if it fails)
        let _ = db.cleanup_wal();

        Ok(IndexingResult::new(
            directory.id,
            total,
            processed_count,
            error_count,
        ))
    }

    /// Resync an existing directory (smart resync).
    pub fn resync_directory<F>(
        &self,
        db: &DatabaseConnection,
        directory_id: i64,
        progress_callback: F,
    ) -> Result<IndexingResult>
    where
        F: Fn(IndexingProgress) + Send + Sync,
    {
        self.cancel_flag.store(false, Ordering::SeqCst);

        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());

        // Get directory
        let directory = dir_repo
            .get_by_id(directory_id)?
            .ok_or_else(|| SfxError::DirectoryNotFound(format!("ID: {}", directory_id)))?;

        // Verify directory exists on disk
        let path = Path::new(&directory.path);
        if !path.exists() || !path.is_dir() {
            return Err(SfxError::DirectoryNotFound(directory.path.clone()));
        }

        // Phase 1: Scanning with progress (discovering + preparing)
        progress_callback(IndexingProgress {
            phase: "discovering".to_string(),
            current: 0,
            total: 0,
            current_file: None,
            stats: None,
        });

        let scanned_files = scan_directory_with_progress(path, Some(|phase: &str, current: usize, total: usize, file: &str| {
            progress_callback(IndexingProgress {
                phase: phase.to_string(),
                current,
                total,
                current_file: if file.is_empty() { None } else { Some(file.to_string()) },
                stats: None,
            });
        }));

        // Phase 2: Comparing
        progress_callback(IndexingProgress {
            phase: "comparing".to_string(),
            current: 0,
            total: scanned_files.len(),
            current_file: None,
            stats: None,
        });

        let existing_files = file_repo.list_by_directory(directory_id)?;

        // Create lookup map for existing files
        let existing_by_path: HashMap<String, SoundFile> = existing_files
            .into_iter()
            .map(|f| (f.full_path.clone(), f))
            .collect();

        // Categorize files
        let mut unchanged_files: Vec<(&ScannedFile, &SoundFile)> = Vec::new();
        let mut modified_files: Vec<(&ScannedFile, &SoundFile)> = Vec::new();
        let mut new_files: Vec<&ScannedFile> = Vec::new();
        let mut scanned_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

        for scanned in &scanned_files {
            let full_path = scanned.full_path.to_string_lossy().into_owned();
            scanned_paths.insert(full_path.clone());

            if let Some(existing) = existing_by_path.get(&full_path) {
                if existing.file_size as u64 != scanned.file_size {
                    modified_files.push((scanned, existing));
                } else {
                    unchanged_files.push((scanned, existing));
                }
            } else {
                new_files.push(scanned);
            }
        }

        // Find deleted files
        let deleted_files: Vec<&SoundFile> = existing_by_path
            .values()
            .filter(|f| !scanned_paths.contains(&f.full_path))
            .collect();

        let stats = ResyncStats {
            unchanged: unchanged_files.len(),
            modified: modified_files.len(),
            added: new_files.len(),
            deleted: deleted_files.len(),
        };

        progress_callback(IndexingProgress {
            phase: "comparing".to_string(),
            current: scanned_files.len(),
            total: scanned_files.len(),
            current_file: None,
            stats: Some(stats.clone()),
        });

        if self.is_cancelled() {
            return Ok(IndexingResult::cancelled(directory_id, 0));
        }

        // Phase 3: Extract metadata for new and modified files
        let files_to_extract: Vec<String> = new_files
            .iter()
            .map(|f| f.full_path.to_string_lossy().into_owned())
            .chain(modified_files.iter().map(|(f, _)| f.full_path.to_string_lossy().into_owned()))
            .collect();

        let mut metadata_map: HashMap<String, super::metadata::AudioMetadata> = HashMap::new();
        let mut error_count = 0;

        if !files_to_extract.is_empty() {
            let total_to_extract = files_to_extract.len();

            // Emit initial extracting progress immediately
            progress_callback(IndexingProgress {
                phase: "extracting".to_string(),
                current: 0,
                total: total_to_extract,
                current_file: None,
                stats: Some(stats.clone()),
            });

            let metadata_results = extract_batch(&files_to_extract, Some(|current: usize, _total: usize, file: &str| {
                progress_callback(IndexingProgress {
                    phase: "extracting".to_string(),
                    current,
                    total: total_to_extract,
                    current_file: Some(file.to_string()),
                    stats: Some(stats.clone()),
                });
            }));

            for result in metadata_results {
                if let Some(meta) = result.metadata {
                    metadata_map.insert(result.path, meta);
                } else {
                    error_count += 1;
                }
            }
        }

        if self.is_cancelled() {
            return Ok(IndexingResult::cancelled(directory_id, 0));
        }

        // Phase 4: Save changes
        progress_callback(IndexingProgress {
            phase: "saving".to_string(),
            current: 0,
            total: new_files.len() + modified_files.len() + deleted_files.len(),
            current_file: None,
            stats: Some(stats.clone()),
        });

        // Delete removed files
        if !deleted_files.is_empty() {
            let ids: Vec<i64> = deleted_files.iter().map(|f| f.id).collect();
            file_repo.delete_by_ids(&ids)?;
        }

        // Update modified files
        for (scanned, existing) in &modified_files {
            let full_path = scanned.full_path.to_string_lossy().into_owned();
            let meta = metadata_map.get(&full_path);

            let update = SoundFileUpdate {
                file_size: Some(scanned.file_size as i64),
                modified_at: Some(scanned.modified_at),
                duration_ms: Some(meta.and_then(|m| m.duration_ms)),
                sample_rate: Some(meta.and_then(|m| m.sample_rate)),
                channels: Some(meta.and_then(|m| m.channels)),
                bit_rate: Some(meta.and_then(|m| m.bit_rate)),
                codec: Some(meta.and_then(|m| m.codec.clone())),
                title: Some(meta.and_then(|m| m.title.clone())),
                artist: Some(meta.and_then(|m| m.artist.clone())),
                album: Some(meta.and_then(|m| m.album.clone())),
                genre: Some(meta.and_then(|m| m.genre.clone())),
                comment: Some(meta.and_then(|m| m.comment.clone())),
            };

            file_repo.update(existing.id, &update)?;
        }

        // Insert new files
        if !new_files.is_empty() {
            let new_sound_files: Vec<SoundFileInsert> = new_files
                .iter()
                .map(|scanned| {
                    let full_path = scanned.full_path.to_string_lossy().into_owned();
                    let meta = metadata_map.get(&full_path);

                    SoundFileInsert {
                        directory_id,
                        relative_path: scanned.relative_path.clone(),
                        filename: scanned.filename.clone(),
                        full_path,
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

            file_repo.insert_batch(&new_sound_files)?;
        }

        // Update directory stats
        let final_count = unchanged_files.len() + modified_files.len() + new_files.len();
        dir_repo.update_file_count(directory_id, final_count as i64)?;
        dir_repo.update_last_synced_at(directory_id, Self::now())?;

        // Clean up WAL/SHM files (non-fatal if it fails)
        let _ = db.cleanup_wal();

        Ok(IndexingResult::new(directory_id, final_count, final_count, error_count)
            .with_resync_stats(&stats))
    }

    /// Get current Unix timestamp.
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

/// Cancel the indexing operation.
pub fn cancel_indexing(cancel_flag: &Arc<AtomicBool>) {
    cancel_flag.store(true, Ordering::SeqCst);
}
