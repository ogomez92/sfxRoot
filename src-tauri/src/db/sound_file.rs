//! Sound file repository for managing audio file metadata.

use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;

/// Represents a sound file with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundFile {
    pub id: i64,
    pub directory_id: i64,
    pub relative_path: String,
    pub filename: String,
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
    pub indexed_at: i64,
}

/// Data for inserting a new sound file.
#[derive(Debug, Clone)]
pub struct SoundFileInsert {
    pub directory_id: i64,
    pub relative_path: String,
    pub filename: String,
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

/// Data for updating a sound file.
#[derive(Debug, Clone, Default)]
pub struct SoundFileUpdate {
    pub file_size: Option<i64>,
    pub modified_at: Option<i64>,
    pub duration_ms: Option<Option<i64>>,
    pub sample_rate: Option<Option<i64>>,
    pub channels: Option<Option<i64>>,
    pub bit_rate: Option<Option<i64>>,
    pub codec: Option<Option<String>>,
    pub title: Option<Option<String>>,
    pub artist: Option<Option<String>>,
    pub album: Option<Option<String>>,
    pub genre: Option<Option<String>>,
    pub comment: Option<Option<String>>,
}

const INSERT_SQL: &str = r#"
INSERT INTO sound_files (
    directory_id, relative_path, filename, filename_lower, full_path,
    extension, file_size, modified_at, duration_ms, sample_rate,
    channels, bit_rate, codec, title, artist, album, genre, comment
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#;

const SELECT_ALL_FIELDS: &str = r#"
SELECT id, directory_id, relative_path, filename, full_path, extension,
       file_size, modified_at, duration_ms, sample_rate, channels, bit_rate,
       codec, title, artist, album, genre, comment, indexed_at
FROM sound_files
"#;

/// Repository for sound file CRUD operations.
pub struct SoundFileRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SoundFileRepository<'a> {
    /// Create a new SoundFileRepository with the given connection.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Parse a row into a SoundFile struct.
    fn row_to_sound_file(row: &rusqlite::Row) -> rusqlite::Result<SoundFile> {
        Ok(SoundFile {
            id: row.get(0)?,
            directory_id: row.get(1)?,
            relative_path: row.get(2)?,
            filename: row.get(3)?,
            full_path: row.get(4)?,
            extension: row.get(5)?,
            file_size: row.get(6)?,
            modified_at: row.get(7)?,
            duration_ms: row.get(8)?,
            sample_rate: row.get(9)?,
            channels: row.get(10)?,
            bit_rate: row.get(11)?,
            codec: row.get(12)?,
            title: row.get(13)?,
            artist: row.get(14)?,
            album: row.get(15)?,
            genre: row.get(16)?,
            comment: row.get(17)?,
            indexed_at: row.get(18)?,
        })
    }

    /// Insert a batch of sound files efficiently using a transaction.
    pub fn insert_batch(&self, files: &[SoundFileInsert]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        // Wrap all inserts in a single transaction for performance
        self.conn.execute("BEGIN IMMEDIATE", [])?;

        let result = (|| {
            let mut stmt = self.conn.prepare(INSERT_SQL)?;

            for file in files {
                stmt.execute(params![
                    file.directory_id,
                    file.relative_path,
                    file.filename,
                    file.filename.to_lowercase(),
                    file.full_path,
                    file.extension,
                    file.file_size,
                    file.modified_at,
                    file.duration_ms,
                    file.sample_rate,
                    file.channels,
                    file.bit_rate,
                    file.codec,
                    file.title,
                    file.artist,
                    file.album,
                    file.genre,
                    file.comment,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.conn.execute("COMMIT", [])?;
                Ok(())
            }
            Err(e) => {
                let _ = self.conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Insert a batch of sound files within an existing transaction.
    pub fn insert_batch_in_transaction(
        tx: &Transaction,
        files: &[SoundFileInsert],
    ) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let mut stmt = tx.prepare(INSERT_SQL)?;

        for file in files {
            stmt.execute(params![
                file.directory_id,
                file.relative_path,
                file.filename,
                file.filename.to_lowercase(),
                file.full_path,
                file.extension,
                file.file_size,
                file.modified_at,
                file.duration_ms,
                file.sample_rate,
                file.channels,
                file.bit_rate,
                file.codec,
                file.title,
                file.artist,
                file.album,
                file.genre,
                file.comment,
            ])?;
        }

        Ok(())
    }

    /// Get a sound file by ID.
    pub fn get_by_id(&self, id: i64) -> Result<Option<SoundFile>> {
        let sql = format!("{} WHERE id = ?", SELECT_ALL_FIELDS);
        let result = self
            .conn
            .query_row(&sql, params![id], Self::row_to_sound_file);

        match result {
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get a sound file by full path.
    pub fn get_by_path(&self, full_path: &str) -> Result<Option<SoundFile>> {
        let sql = format!("{} WHERE full_path = ?", SELECT_ALL_FIELDS);
        let result = self
            .conn
            .query_row(&sql, params![full_path], Self::row_to_sound_file);

        match result {
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all sound files in a directory.
    pub fn list_by_directory(&self, directory_id: i64) -> Result<Vec<SoundFile>> {
        let sql = format!(
            "{} WHERE directory_id = ? ORDER BY filename_lower",
            SELECT_ALL_FIELDS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let files = stmt
            .query_map(params![directory_id], Self::row_to_sound_file)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(files)
    }

    /// List only the `full_path` of every sound file in a directory.
    ///
    /// A lightweight alternative to [`list_by_directory`] for cases that only
    /// need to know which files are already indexed (e.g. resuming an
    /// interrupted index). Avoids materializing full `SoundFile` rows, which
    /// matters for directories with millions of entries.
    pub fn list_paths_by_directory(&self, directory_id: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT full_path FROM sound_files WHERE directory_id = ?")?;
        let paths = stmt
            .query_map(params![directory_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(paths)
    }

    /// Count sound files in a directory.
    pub fn count_by_directory(&self, directory_id: i64) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sound_files WHERE directory_id = ?",
            params![directory_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Delete a sound file by ID.
    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM sound_files WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Delete all sound files in a directory.
    pub fn delete_by_directory(&self, directory_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM sound_files WHERE directory_id = ?",
            params![directory_id],
        )?;
        Ok(())
    }

    /// Delete multiple sound files by IDs.
    pub fn delete_by_ids(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM sound_files WHERE id IN ({})", placeholders);

        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        stmt.execute(params.as_slice())?;

        Ok(())
    }

    /// Update a sound file's metadata.
    pub fn update(&self, id: i64, update: &SoundFileUpdate) -> Result<()> {
        let mut updates = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(file_size) = update.file_size {
            updates.push("file_size = ?");
            values.push(Box::new(file_size));
        }
        if let Some(modified_at) = update.modified_at {
            updates.push("modified_at = ?");
            values.push(Box::new(modified_at));
        }
        if let Some(ref duration_ms) = update.duration_ms {
            updates.push("duration_ms = ?");
            values.push(Box::new(*duration_ms));
        }
        if let Some(ref sample_rate) = update.sample_rate {
            updates.push("sample_rate = ?");
            values.push(Box::new(*sample_rate));
        }
        if let Some(ref channels) = update.channels {
            updates.push("channels = ?");
            values.push(Box::new(*channels));
        }
        if let Some(ref bit_rate) = update.bit_rate {
            updates.push("bit_rate = ?");
            values.push(Box::new(*bit_rate));
        }
        if let Some(ref codec) = update.codec {
            updates.push("codec = ?");
            values.push(Box::new(codec.clone()));
        }
        if let Some(ref title) = update.title {
            updates.push("title = ?");
            values.push(Box::new(title.clone()));
        }
        if let Some(ref artist) = update.artist {
            updates.push("artist = ?");
            values.push(Box::new(artist.clone()));
        }
        if let Some(ref album) = update.album {
            updates.push("album = ?");
            values.push(Box::new(album.clone()));
        }
        if let Some(ref genre) = update.genre {
            updates.push("genre = ?");
            values.push(Box::new(genre.clone()));
        }
        if let Some(ref comment) = update.comment {
            updates.push("comment = ?");
            values.push(Box::new(comment.clone()));
        }

        if updates.is_empty() {
            return Ok(());
        }

        // Always update indexed_at
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        updates.push("indexed_at = ?");
        values.push(Box::new(now));

        // Add id at the end
        values.push(Box::new(id));

        let sql = format!(
            "UPDATE sound_files SET {} WHERE id = ?",
            updates.join(", ")
        );

        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        self.conn.execute(&sql, params.as_slice())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{DatabaseConnection, DirectoryRepository};
    use tempfile::tempdir;

    fn create_test_db() -> DatabaseConnection {
        let dir = tempdir().unwrap();
        let db_path = dir.keep().join("test.db");
        DatabaseConnection::create(&db_path).unwrap()
    }

    fn create_test_insert(directory_id: i64, filename: &str) -> SoundFileInsert {
        SoundFileInsert {
            directory_id,
            relative_path: filename.to_string(),
            filename: filename.to_string(),
            full_path: format!("/test/{}", filename),
            extension: "mp3".to_string(),
            file_size: 1000,
            modified_at: 1704067200,
            duration_ms: Some(3000),
            sample_rate: Some(44100),
            channels: Some(2),
            bit_rate: Some(320),
            codec: Some("mp3".to_string()),
            title: Some("Test Title".to_string()),
            artist: Some("Test Artist".to_string()),
            album: None,
            genre: None,
            comment: None,
        }
    }

    #[test]
    fn test_insert_batch() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();

        let files = vec![
            create_test_insert(dir.id, "file1.mp3"),
            create_test_insert(dir.id, "file2.mp3"),
        ];

        file_repo.insert_batch(&files).unwrap();

        let count = file_repo.count_by_directory(dir.id).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_by_path() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let insert = create_test_insert(dir.id, "file1.mp3");
        file_repo.insert_batch(&[insert]).unwrap();

        let found = file_repo.get_by_path("/test/file1.mp3").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().filename, "file1.mp3");
    }

    #[test]
    fn test_list_by_directory() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files = vec![
            create_test_insert(dir.id, "b_file.mp3"),
            create_test_insert(dir.id, "a_file.mp3"),
        ];
        file_repo.insert_batch(&files).unwrap();

        let listed = file_repo.list_by_directory(dir.id).unwrap();
        assert_eq!(listed.len(), 2);
        // Should be sorted by filename_lower
        assert_eq!(listed[0].filename, "a_file.mp3");
        assert_eq!(listed[1].filename, "b_file.mp3");
    }

    #[test]
    fn test_delete_by_ids() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files = vec![
            create_test_insert(dir.id, "file1.mp3"),
            create_test_insert(dir.id, "file2.mp3"),
            create_test_insert(dir.id, "file3.mp3"),
        ];
        file_repo.insert_batch(&files).unwrap();

        let all_files = file_repo.list_by_directory(dir.id).unwrap();
        let ids_to_delete: Vec<i64> = all_files.iter().take(2).map(|f| f.id).collect();

        file_repo.delete_by_ids(&ids_to_delete).unwrap();

        let remaining = file_repo.count_by_directory(dir.id).unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn test_update() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        file_repo
            .insert_batch(&[create_test_insert(dir.id, "file1.mp3")])
            .unwrap();

        let file = file_repo.get_by_path("/test/file1.mp3").unwrap().unwrap();

        let update = SoundFileUpdate {
            duration_ms: Some(Some(5000)),
            title: Some(Some("New Title".to_string())),
            ..Default::default()
        };

        file_repo.update(file.id, &update).unwrap();

        let updated = file_repo.get_by_id(file.id).unwrap().unwrap();
        assert_eq!(updated.duration_ms, Some(5000));
        assert_eq!(updated.title, Some("New Title".to_string()));
    }
}
