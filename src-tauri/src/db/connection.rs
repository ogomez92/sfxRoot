//! SQLite database connection management.

use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::error::{Result, SfxError};

use super::schema::SCHEMA;

/// Database connection wrapper with lifecycle management.
pub struct DatabaseConnection {
    conn: Connection,
    path: PathBuf,
}

impl DatabaseConnection {
    /// Create a new database at the specified path.
    ///
    /// If a database already exists at the path, it will be deleted first
    /// (the save dialog already confirms overwrite with the user).
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Delete existing file if present (save dialog confirms overwrite)
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        let conn = Connection::open(&path)?;
        Self::initialize_pragmas(&conn)?;
        Self::create_schema(&conn)?;

        Ok(Self { conn, path })
    }

    /// Open an existing database at the specified path.
    ///
    /// Returns an error if the database does not exist.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        if !path.exists() {
            return Err(SfxError::DatabaseNotFound(path.display().to_string()));
        }

        let conn = Connection::open(&path)?;
        Self::initialize_pragmas(&conn)?;

        Ok(Self { conn, path })
    }

    /// Initialize database pragmas for optimal performance.
    fn initialize_pragmas(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;",
        )?;
        Ok(())
    }

    /// Create the database schema.
    fn create_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    /// Get the underlying connection for direct access.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Get the database file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Force a WAL checkpoint to write pending changes to the main database file.
    pub fn checkpoint(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        Ok(())
    }

    /// Clean up WAL and SHM files by checkpointing and briefly switching journal mode.
    /// Call this after large operations like indexing to leave only the .db file.
    pub fn cleanup_wal(&self) -> Result<()> {
        // Checkpoint to flush all changes
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        // Switch to DELETE mode (removes WAL and SHM files)
        self.conn.execute_batch("PRAGMA journal_mode=DELETE")?;
        // Switch back to WAL mode for future operations
        self.conn.execute_batch("PRAGMA journal_mode=WAL")?;
        Ok(())
    }

    /// Get the ID of the last inserted row.
    pub fn last_insert_rowid(&self) -> i64 {
        self.conn.last_insert_rowid()
    }

    /// Get all table names in the database.
    pub fn get_tables(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' OR type='virtual table'",
        )?;
        let tables = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = DatabaseConnection::create(&db_path).unwrap();
        assert!(db_path.exists());

        let tables = db.get_tables().unwrap();
        assert!(tables.contains(&"directories".to_string()));
        assert!(tables.contains(&"sound_files".to_string()));
        assert!(tables.contains(&"sound_files_fts".to_string()));
    }

    #[test]
    fn test_create_existing_database_overwrites() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create first database
        let _db = DatabaseConnection::create(&db_path).unwrap();
        drop(_db);

        // Create again should succeed (overwrites)
        let db = DatabaseConnection::create(&db_path).unwrap();
        let tables = db.get_tables().unwrap();
        assert!(tables.contains(&"directories".to_string()));
    }

    #[test]
    fn test_open_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create first
        drop(DatabaseConnection::create(&db_path).unwrap());

        // Then open
        let db = DatabaseConnection::open(&db_path).unwrap();
        assert_eq!(db.path(), db_path);
    }

    #[test]
    fn test_open_nonexistent_database_fails() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("nonexistent.db");

        let result = DatabaseConnection::open(&db_path);
        assert!(matches!(result, Err(SfxError::DatabaseNotFound(_))));
    }
}
