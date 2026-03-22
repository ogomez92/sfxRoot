//! File scanner for parallel directory traversal.

use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

/// Supported audio file extensions (standard formats with pure Rust support).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "ogg", "flac", "aiff", "aif", "m4a", "opus", "aac",
];

/// Represents a scanned audio file with basic metadata.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub filename: String,
    pub relative_path: String,
    pub full_path: PathBuf,
    pub extension: String,
    pub file_size: u64,
    pub modified_at: i64,
}

/// Check if a file extension is supported.
pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Scan a directory recursively for supported audio files.
///
/// Uses walkdir for efficient traversal and rayon for parallel processing.
pub fn scan_directory(base_path: &Path) -> Vec<ScannedFile> {
    scan_directory_with_progress(base_path, None::<fn(&str, usize, usize, &str)>)
}

/// Scan a directory with progress callback.
///
/// The callback receives (phase, files_found, total, current_directory).
/// Phase: "discovering" during walkdir, "preparing" during metadata collection.
pub fn scan_directory_with_progress<F>(base_path: &Path, progress: Option<F>) -> Vec<ScannedFile>
where
    F: Fn(&str, usize, usize, &str) + Sync,
{
    let mut entries = Vec::new();
    let mut last_progress_count = 0;
    let progress_interval = 100; // Report every 100 files found

    // Phase 1: Discover files with progress reporting
    for entry in WalkDir::new(base_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let is_audio = entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(is_supported_extension)
                .unwrap_or(false);

            if is_audio {
                entries.push(entry);

                // Report progress periodically
                if let Some(ref cb) = progress {
                    if entries.len() >= last_progress_count + progress_interval {
                        last_progress_count = entries.len();
                        let dir = entries.last()
                            .and_then(|e| e.path().parent())
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        cb("discovering", entries.len(), 0, &dir);
                    }
                }
            }
        }
    }

    let total_entries = entries.len();

    // Final discovery report
    if let Some(ref cb) = progress {
        cb("discovering", total_entries, total_entries, "");
    }

    if total_entries == 0 {
        return Vec::new();
    }

    // Phase 2: Collect file metadata in parallel with progress
    let processed = Arc::new(AtomicUsize::new(0));
    let progress_interval_prepare = 500; // Report every 500 files during preparation

    let results: Vec<ScannedFile> = entries
        .into_par_iter()
        .filter_map(|entry| {
            let full_path = entry.path().to_path_buf();
            let metadata = entry.metadata().ok()?;

            let relative_path = full_path
                .strip_prefix(base_path)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");

            let extension = full_path
                .extension()?
                .to_str()?
                .to_lowercase();

            let modified_at = metadata
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;

            // Report progress during preparation
            let current = processed.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(ref cb) = progress {
                if current % progress_interval_prepare == 0 || current == total_entries {
                    cb("preparing", current, total_entries, &full_path.to_string_lossy());
                }
            }

            Some(ScannedFile {
                filename: entry.file_name().to_string_lossy().into_owned(),
                relative_path,
                full_path,
                extension,
                file_size: metadata.len(),
                modified_at,
            })
        })
        .collect();

    // Final preparation report
    if let Some(ref cb) = progress {
        cb("preparing", total_entries, total_entries, "");
    }

    results
}

/// Scan a directory and return results sorted by filename.
pub fn scan_directory_sorted(base_path: &Path) -> Vec<ScannedFile> {
    let mut files = scan_directory(base_path);
    files.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    fn create_test_file(dir: &Path, name: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        File::create(path).unwrap();
    }

    #[test]
    fn test_is_supported_extension() {
        assert!(is_supported_extension("mp3"));
        assert!(is_supported_extension("MP3"));
        assert!(is_supported_extension("wav"));
        assert!(is_supported_extension("flac"));
        assert!(!is_supported_extension("txt"));
        assert!(!is_supported_extension("exe"));
    }

    #[test]
    fn test_scan_directory() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        // Create test files
        create_test_file(base, "sound1.mp3");
        create_test_file(base, "sound2.wav");
        create_test_file(base, "subfolder/nested.flac");
        create_test_file(base, "ignored.txt");

        let files = scan_directory(base);
        assert_eq!(files.len(), 3);

        let extensions: Vec<_> = files.iter().map(|f| f.extension.as_str()).collect();
        assert!(extensions.contains(&"mp3"));
        assert!(extensions.contains(&"wav"));
        assert!(extensions.contains(&"flac"));
    }

    #[test]
    fn test_scan_directory_relative_paths() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        create_test_file(base, "root.mp3");
        create_test_file(base, "sub/nested.mp3");
        create_test_file(base, "sub/deep/very_nested.mp3");

        let files = scan_directory(base);
        let relative_paths: Vec<_> = files.iter().map(|f| f.relative_path.as_str()).collect();

        assert!(relative_paths.contains(&"root.mp3"));
        assert!(relative_paths.contains(&"sub/nested.mp3"));
        assert!(relative_paths.contains(&"sub/deep/very_nested.mp3"));
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let files = scan_directory(dir.path());
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_directory_sorted() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        create_test_file(base, "z_last.mp3");
        create_test_file(base, "a_first.mp3");
        create_test_file(base, "m_middle.mp3");

        let files = scan_directory_sorted(base);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].filename, "a_first.mp3");
        assert_eq!(files[1].filename, "m_middle.mp3");
        assert_eq!(files[2].filename, "z_last.mp3");
    }
}
