//! Search repository for querying sound files with FTS5.

use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::sound_file::SoundFile;

/// Query options for searching sound files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptions {
    pub query: Option<String>,
    pub directory_id: Option<i64>,
    pub min_duration_ms: Option<i64>,
    pub max_duration_ms: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Repository for searching sound files using FTS5.
pub struct SearchRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SearchRepository<'a> {
    /// Create a new SearchRepository with the given connection.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Search for sound files matching the given options.
    pub fn search(&self, options: &QueryOptions) -> Result<Vec<SoundFile>> {
        let (sql, params) = self.build_search_query(options, false);
        let mut stmt = self.conn.prepare(&sql)?;

        let files = stmt
            .query_map(params_from_iter(params.iter()), Self::row_to_sound_file)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Count sound files matching the given options.
    pub fn count(&self, options: &QueryOptions) -> Result<i64> {
        let (sql, params) = self.build_search_query(options, true);
        let count: i64 =
            self.conn
                .query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))?;
        Ok(count)
    }

    /// Get all file paths matching the query options (for clipboard copy).
    /// Can optionally specify a range with from_index and to_index.
    pub fn get_paths(
        &self,
        options: &QueryOptions,
        from_index: Option<i64>,
        to_index: Option<i64>,
    ) -> Result<Vec<String>> {
        // Build query for just paths
        let mut params: Vec<String> = Vec::new();
        let mut conditions: Vec<String> = Vec::new();
        let uses_fts = options.query.as_ref().map(|q| !q.trim().is_empty()).unwrap_or(false);

        if let Some(ref query) = options.query {
            let trimmed = query.trim();
            if !trimmed.is_empty() {
                let fts_query = trimmed
                    .split_whitespace()
                    .map(|term| format!("{}*", term))
                    .collect::<Vec<_>>()
                    .join(" ");
                params.push(fts_query);
            }
        }

        if let Some(directory_id) = options.directory_id {
            conditions.push("sf.directory_id = ?".to_string());
            params.push(directory_id.to_string());
        }

        if let Some(min_duration) = options.min_duration_ms {
            conditions.push("sf.duration_ms >= ?".to_string());
            params.push(min_duration.to_string());
        }
        if let Some(max_duration) = options.max_duration_ms {
            conditions.push("sf.duration_ms <= ?".to_string());
            params.push(max_duration.to_string());
        }

        let mut sql = if uses_fts {
            "SELECT sf.full_path FROM sound_files sf \
             INNER JOIN sound_files_fts fts ON sf.id = fts.rowid \
             WHERE fts.sound_files_fts MATCH ?".to_string()
        } else {
            "SELECT sf.full_path FROM sound_files sf WHERE 1=1".to_string()
        };

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }

        // Add sorting
        let sort_column = match options.sort_by.as_deref() {
            Some("duration") => "sf.duration_ms",
            Some("modifiedAt") => "sf.modified_at",
            Some("size") => "sf.file_size",
            _ => "sf.filename_lower",
        };
        let sort_order = match options.sort_order.as_deref() {
            Some("desc") => "DESC",
            _ => "ASC",
        };
        sql.push_str(&format!(" ORDER BY {} {}", sort_column, sort_order));

        // Add limit/offset for range selection
        if let (Some(from), Some(to)) = (from_index, to_index) {
            let limit = (to - from + 1).max(0);
            sql.push_str(" LIMIT ? OFFSET ?");
            params.push(limit.to_string());
            params.push(from.to_string());
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let paths = stmt
            .query_map(params_from_iter(params.iter()), |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(paths)
    }

    /// Find the index of the first file starting with a given prefix.
    /// Returns the 0-based index in the sorted result set, or None if not found.
    pub fn find_prefix_index(&self, options: &QueryOptions, prefix: &str) -> Result<Option<i64>> {
        // Count files that come before the prefix alphabetically
        let prefix_lower = prefix.to_lowercase();

        // Build base conditions from options
        let mut params: Vec<String> = Vec::new();
        let mut conditions: Vec<String> = Vec::new();
        let uses_fts = options.query.as_ref().map(|q| !q.trim().is_empty()).unwrap_or(false);

        // Handle FTS search
        if let Some(ref query) = options.query {
            let trimmed = query.trim();
            if !trimmed.is_empty() {
                let fts_query = trimmed
                    .split_whitespace()
                    .map(|term| format!("{}*", term))
                    .collect::<Vec<_>>()
                    .join(" ");
                params.push(fts_query);
            }
        }

        // Directory filter
        if let Some(directory_id) = options.directory_id {
            conditions.push("sf.directory_id = ?".to_string());
            params.push(directory_id.to_string());
        }

        // Duration filters
        if let Some(min_duration) = options.min_duration_ms {
            conditions.push("sf.duration_ms >= ?".to_string());
            params.push(min_duration.to_string());
        }
        if let Some(max_duration) = options.max_duration_ms {
            conditions.push("sf.duration_ms <= ?".to_string());
            params.push(max_duration.to_string());
        }

        // For filename sort (default), count files with filename_lower < prefix
        let sort_by = options.sort_by.as_deref().unwrap_or("filename");
        let sort_order = options.sort_order.as_deref().unwrap_or("asc");

        // Only support prefix search for filename sorting
        if sort_by != "filename" {
            return Ok(None);
        }

        // Add prefix condition based on sort order
        if sort_order == "asc" {
            conditions.push("sf.filename_lower < ?".to_string());
        } else {
            conditions.push("sf.filename_lower > ?".to_string());
        }
        params.push(prefix_lower);

        let sql = if uses_fts {
            format!(
                "SELECT COUNT(*) FROM sound_files sf \
                 INNER JOIN sound_files_fts fts ON sf.id = fts.rowid \
                 WHERE fts.sound_files_fts MATCH ? AND {}",
                conditions.join(" AND ")
            )
        } else {
            format!(
                "SELECT COUNT(*) FROM sound_files sf WHERE {}",
                conditions.join(" AND ")
            )
        };

        let count: i64 = self.conn.query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))?;
        Ok(Some(count))
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

    /// Build the search SQL query with parameters.
    fn build_search_query(
        &self,
        options: &QueryOptions,
        count_only: bool,
    ) -> (String, Vec<String>) {
        let mut params: Vec<String> = Vec::new();
        let mut conditions: Vec<String> = Vec::new();
        let uses_fts = options
            .query
            .as_ref()
            .map(|q| !q.trim().is_empty())
            .unwrap_or(false);

        // Handle FTS search
        if let Some(ref query) = options.query {
            let trimmed = query.trim();
            if !trimmed.is_empty() {
                // Convert query to FTS5 format: "word1 word2" -> "word1* word2*"
                let fts_query = trimmed
                    .split_whitespace()
                    .map(|term| format!("{}*", term))
                    .collect::<Vec<_>>()
                    .join(" ");
                params.push(fts_query);
            }
        }

        // Directory filter
        if let Some(directory_id) = options.directory_id {
            conditions.push("sf.directory_id = ?".to_string());
            params.push(directory_id.to_string());
        }

        // Duration filters
        if let Some(min_duration) = options.min_duration_ms {
            conditions.push("sf.duration_ms >= ?".to_string());
            params.push(min_duration.to_string());
        }
        if let Some(max_duration) = options.max_duration_ms {
            conditions.push("sf.duration_ms <= ?".to_string());
            params.push(max_duration.to_string());
        }

        // Build the query
        let mut sql = if count_only {
            if uses_fts {
                "SELECT COUNT(*) FROM sound_files sf \
                 INNER JOIN sound_files_fts fts ON sf.id = fts.rowid \
                 WHERE fts.sound_files_fts MATCH ?"
                    .to_string()
            } else {
                "SELECT COUNT(*) FROM sound_files sf WHERE 1=1".to_string()
            }
        } else {
            let select_fields = "sf.id, sf.directory_id, sf.relative_path, sf.filename, \
                                 sf.full_path, sf.extension, sf.file_size, sf.modified_at, \
                                 sf.duration_ms, sf.sample_rate, sf.channels, sf.bit_rate, \
                                 sf.codec, sf.title, sf.artist, sf.album, sf.genre, \
                                 sf.comment, sf.indexed_at";

            if uses_fts {
                format!(
                    "SELECT {} FROM sound_files sf \
                     INNER JOIN sound_files_fts fts ON sf.id = fts.rowid \
                     WHERE fts.sound_files_fts MATCH ?",
                    select_fields
                )
            } else {
                format!("SELECT {} FROM sound_files sf WHERE 1=1", select_fields)
            }
        };

        // Add conditions
        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }

        // Add sorting (not for count)
        if !count_only {
            let sort_column = match options.sort_by.as_deref() {
                Some("duration") => "sf.duration_ms",
                Some("modifiedAt") => "sf.modified_at",
                Some("size") => "sf.file_size",
                _ => "sf.filename_lower",
            };
            let sort_order = match options.sort_order.as_deref() {
                Some("desc") => "DESC",
                _ => "ASC",
            };
            sql.push_str(&format!(" ORDER BY {} {}", sort_column, sort_order));

            // Add pagination
            if let Some(limit) = options.limit {
                sql.push_str(" LIMIT ?");
                params.push(limit.to_string());
            }
            if let Some(offset) = options.offset {
                sql.push_str(" OFFSET ?");
                params.push(offset.to_string());
            }
        }

        (sql, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{DatabaseConnection, DirectoryRepository, SoundFileInsert, SoundFileRepository};
    use tempfile::tempdir;

    fn create_test_db() -> DatabaseConnection {
        let dir = tempdir().unwrap();
        let db_path = dir.keep().join("test.db");
        DatabaseConnection::create(&db_path).unwrap()
    }

    fn create_test_file(directory_id: i64, filename: &str, duration_ms: Option<i64>) -> SoundFileInsert {
        SoundFileInsert {
            directory_id,
            relative_path: filename.to_string(),
            filename: filename.to_string(),
            full_path: format!("/test/{}", filename),
            extension: "mp3".to_string(),
            file_size: 1000,
            modified_at: 1704067200,
            duration_ms,
            sample_rate: Some(44100),
            channels: Some(2),
            bit_rate: Some(320),
            codec: Some("mp3".to_string()),
            title: Some(format!("Title for {}", filename)),
            artist: Some("Test Artist".to_string()),
            album: None,
            genre: None,
            comment: None,
        }
    }

    #[test]
    fn test_search_all() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());
        let search_repo = SearchRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files = vec![
            create_test_file(dir.id, "file1.mp3", Some(3000)),
            create_test_file(dir.id, "file2.mp3", Some(5000)),
        ];
        file_repo.insert_batch(&files).unwrap();

        let results = search_repo.search(&QueryOptions::default()).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_with_fts() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());
        let search_repo = SearchRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files = vec![
            create_test_file(dir.id, "kick_drum.mp3", Some(3000)),
            create_test_file(dir.id, "snare_hit.mp3", Some(2000)),
        ];
        file_repo.insert_batch(&files).unwrap();

        let options = QueryOptions {
            query: Some("kick".to_string()),
            ..Default::default()
        };
        let results = search_repo.search(&options).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "kick_drum.mp3");
    }

    #[test]
    fn test_search_with_duration_filter() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());
        let search_repo = SearchRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files = vec![
            create_test_file(dir.id, "short.mp3", Some(1000)),
            create_test_file(dir.id, "medium.mp3", Some(5000)),
            create_test_file(dir.id, "long.mp3", Some(10000)),
        ];
        file_repo.insert_batch(&files).unwrap();

        let options = QueryOptions {
            min_duration_ms: Some(3000),
            max_duration_ms: Some(8000),
            ..Default::default()
        };
        let results = search_repo.search(&options).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "medium.mp3");
    }

    #[test]
    fn test_count() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());
        let search_repo = SearchRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files = vec![
            create_test_file(dir.id, "file1.mp3", Some(3000)),
            create_test_file(dir.id, "file2.mp3", Some(5000)),
            create_test_file(dir.id, "file3.mp3", Some(7000)),
        ];
        file_repo.insert_batch(&files).unwrap();

        let count = search_repo.count(&QueryOptions::default()).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_search_with_pagination() {
        let db = create_test_db();
        let dir_repo = DirectoryRepository::new(db.conn());
        let file_repo = SoundFileRepository::new(db.conn());
        let search_repo = SearchRepository::new(db.conn());

        let dir = dir_repo.add("/test").unwrap();
        let files: Vec<_> = (1..=10)
            .map(|i| create_test_file(dir.id, &format!("file{:02}.mp3", i), Some(i * 1000)))
            .collect();
        file_repo.insert_batch(&files).unwrap();

        let options = QueryOptions {
            limit: Some(3),
            offset: Some(2),
            ..Default::default()
        };
        let results = search_repo.search(&options).unwrap();
        assert_eq!(results.len(), 3);
    }
}
