//! Application state management.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::db::connection::DatabaseConnection;

/// Shared application state managed by Tauri.
pub struct AppState {
    /// Current database connection (if open).
    /// Using Mutex because rusqlite::Connection is Send but not Sync.
    pub db: Mutex<Option<DatabaseConnection>>,
    /// Flag to cancel ongoing indexing operations.
    pub indexing_cancel: Arc<AtomicBool>,
}

impl AppState {
    /// Create a new AppState instance.
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
            indexing_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if a database is currently open.
    pub fn is_db_open(&self) -> bool {
        self.db.lock().unwrap().is_some()
    }

    /// Get the database path if open.
    pub fn get_db_path(&self) -> Option<std::path::PathBuf> {
        self.db.lock().unwrap().as_ref().map(|db| db.path().to_path_buf())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// AppState is Send + Sync because:
// - Mutex<Option<DatabaseConnection>> is Send + Sync (Mutex provides thread safety)
// - Arc<AtomicBool> is Send + Sync
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}
