//! Progress reporting types for indexing operations.

use serde::{Deserialize, Serialize};

/// Progress information during indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingProgress {
    pub phase: String, // "scanning", "comparing", "extracting", "saving"
    pub current: usize,
    pub total: usize,
    pub current_file: Option<String>,
    pub stats: Option<ResyncStats>,
}

/// Statistics for smart resync operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResyncStats {
    pub unchanged: usize,
    pub modified: usize,
    pub added: usize,
    pub deleted: usize,
}

/// Result of an indexing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingResult {
    pub directory_id: i64,
    pub total_files: usize,
    pub success_count: usize,
    pub error_count: usize,
    pub cancelled: bool,
    // Smart resync stats (optional)
    pub unchanged: Option<usize>,
    pub modified: Option<usize>,
    pub added: Option<usize>,
    pub deleted: Option<usize>,
}

impl IndexingResult {
    /// Create a new result for initial indexing.
    pub fn new(directory_id: i64, total_files: usize, success_count: usize, error_count: usize) -> Self {
        Self {
            directory_id,
            total_files,
            success_count,
            error_count,
            cancelled: false,
            unchanged: None,
            modified: None,
            added: None,
            deleted: None,
        }
    }

    /// Create a cancelled result.
    pub fn cancelled(directory_id: i64, processed: usize) -> Self {
        Self {
            directory_id,
            total_files: 0,
            success_count: processed,
            error_count: 0,
            cancelled: true,
            unchanged: None,
            modified: None,
            added: None,
            deleted: None,
        }
    }

    /// Create a result with resync stats.
    pub fn with_resync_stats(mut self, stats: &ResyncStats) -> Self {
        self.unchanged = Some(stats.unchanged);
        self.modified = Some(stats.modified);
        self.added = Some(stats.added);
        self.deleted = Some(stats.deleted);
        self
    }
}
