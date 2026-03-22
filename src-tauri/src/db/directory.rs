//! Directory repository for managing indexed directories.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{Result, SfxError};

/// Represents an indexed directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
    pub id: i64,
    pub path: String,
    pub file_count: i64,
    pub last_synced_at: Option<i64>,
    pub created_at: i64,
}

/// Repository for directory CRUD operations.
pub struct DirectoryRepository<'a> {
    conn: &'a Connection,
}

impl<'a> DirectoryRepository<'a> {
    /// Create a new DirectoryRepository with the given connection.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Add a new directory to the database.
    ///
    /// Returns an error if the directory already exists.
    pub fn add(&self, path: &str) -> Result<Directory> {
        // Check if already exists
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM directories WHERE path = ?",
                params![path],
                |row| row.get(0),
            )
            .ok();

        if existing.is_some() {
            return Err(SfxError::DirectoryAlreadyIndexed(path.to_string()));
        }

        self.conn
            .execute("INSERT INTO directories (path) VALUES (?)", params![path])?;

        let id = self.conn.last_insert_rowid();
        self.get_by_id(id)?
            .ok_or_else(|| SfxError::Database(rusqlite::Error::QueryReturnedNoRows))
    }

    /// List all directories ordered by path.
    pub fn list(&self) -> Result<Vec<Directory>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, file_count, last_synced_at, created_at FROM directories ORDER BY path")?;

        let directories = stmt
            .query_map([], |row| {
                Ok(Directory {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    file_count: row.get(2)?,
                    last_synced_at: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(directories)
    }

    /// Get a directory by ID.
    pub fn get_by_id(&self, id: i64) -> Result<Option<Directory>> {
        let result = self.conn.query_row(
            "SELECT id, path, file_count, last_synced_at, created_at FROM directories WHERE id = ?",
            params![id],
            |row| {
                Ok(Directory {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    file_count: row.get(2)?,
                    last_synced_at: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        );

        match result {
            Ok(dir) => Ok(Some(dir)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Remove a directory by ID (cascades to sound_files).
    pub fn remove(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM directories WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Update the file count for a directory.
    pub fn update_file_count(&self, id: i64, count: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE directories SET file_count = ? WHERE id = ?",
            params![count, id],
        )?;
        Ok(())
    }

    /// Update the last synced timestamp for a directory.
    pub fn update_last_synced_at(&self, id: i64, timestamp: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE directories SET last_synced_at = ? WHERE id = ?",
            params![timestamp, id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DatabaseConnection;
    use tempfile::tempdir;

    fn create_test_db() -> DatabaseConnection {
        let dir = tempdir().unwrap();
        let db_path = dir.keep().join("test.db");
        DatabaseConnection::create(&db_path).unwrap()
    }

    #[test]
    fn test_add_directory() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        let dir = repo.add("/test/path").unwrap();
        assert_eq!(dir.path, "/test/path");
        assert_eq!(dir.file_count, 0);
        assert!(dir.last_synced_at.is_none());
    }

    #[test]
    fn test_add_duplicate_directory_fails() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        repo.add("/test/path").unwrap();
        let result = repo.add("/test/path");

        assert!(matches!(result, Err(SfxError::DirectoryAlreadyIndexed(_))));
    }

    #[test]
    fn test_list_directories() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        repo.add("/path/a").unwrap();
        repo.add("/path/b").unwrap();

        let dirs = repo.list().unwrap();
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].path, "/path/a");
        assert_eq!(dirs[1].path, "/path/b");
    }

    #[test]
    fn test_get_by_id() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        let created = repo.add("/test/path").unwrap();
        let found = repo.get_by_id(created.id).unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().path, "/test/path");
    }

    #[test]
    fn test_remove_directory() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        let dir = repo.add("/test/path").unwrap();
        repo.remove(dir.id).unwrap();

        let found = repo.get_by_id(dir.id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_update_file_count() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        let dir = repo.add("/test/path").unwrap();
        repo.update_file_count(dir.id, 42).unwrap();

        let updated = repo.get_by_id(dir.id).unwrap().unwrap();
        assert_eq!(updated.file_count, 42);
    }

    #[test]
    fn test_update_last_synced_at() {
        let db = create_test_db();
        let repo = DirectoryRepository::new(db.conn());

        let dir = repo.add("/test/path").unwrap();
        let timestamp = 1704067200; // 2024-01-01 00:00:00 UTC
        repo.update_last_synced_at(dir.id, timestamp).unwrap();

        let updated = repo.get_by_id(dir.id).unwrap().unwrap();
        assert_eq!(updated.last_synced_at, Some(timestamp));
    }
}
