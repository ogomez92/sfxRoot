//! Database schema definition.
//!
//! This schema is identical to the Electron app's schema to ensure
//! database compatibility between the two versions.

/// Complete database schema including tables, indexes, triggers, and FTS5.
pub const SCHEMA: &str = r#"
-- directories table
CREATE TABLE IF NOT EXISTS directories (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    file_count INTEGER DEFAULT 0,
    last_synced_at INTEGER,
    created_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- sound_files table
CREATE TABLE IF NOT EXISTS sound_files (
    id INTEGER PRIMARY KEY,
    directory_id INTEGER REFERENCES directories(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    filename TEXT NOT NULL,
    filename_lower TEXT NOT NULL,
    full_path TEXT UNIQUE NOT NULL,
    extension TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    duration_ms INTEGER,
    sample_rate INTEGER,
    channels INTEGER,
    bit_rate INTEGER,
    codec TEXT,
    title TEXT,
    artist TEXT,
    album TEXT,
    genre TEXT,
    comment TEXT,
    indexed_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS sound_files_fts USING fts5(
    relative_path, filename, title, artist, album, genre, comment,
    content='sound_files',
    content_rowid='id'
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_sound_files_directory ON sound_files(directory_id);
CREATE INDEX IF NOT EXISTS idx_sound_files_duration ON sound_files(duration_ms);
CREATE INDEX IF NOT EXISTS idx_sound_files_filename_lower ON sound_files(filename_lower);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS sound_files_ai AFTER INSERT ON sound_files BEGIN
    INSERT INTO sound_files_fts(rowid, relative_path, filename, title, artist, album, genre, comment)
    VALUES (NEW.id, NEW.relative_path, NEW.filename, NEW.title, NEW.artist, NEW.album, NEW.genre, NEW.comment);
END;

CREATE TRIGGER IF NOT EXISTS sound_files_ad AFTER DELETE ON sound_files BEGIN
    INSERT INTO sound_files_fts(sound_files_fts, rowid, relative_path, filename, title, artist, album, genre, comment)
    VALUES ('delete', OLD.id, OLD.relative_path, OLD.filename, OLD.title, OLD.artist, OLD.album, OLD.genre, OLD.comment);
END;

CREATE TRIGGER IF NOT EXISTS sound_files_au AFTER UPDATE ON sound_files BEGIN
    INSERT INTO sound_files_fts(sound_files_fts, rowid, relative_path, filename, title, artist, album, genre, comment)
    VALUES ('delete', OLD.id, OLD.relative_path, OLD.filename, OLD.title, OLD.artist, OLD.album, OLD.genre, OLD.comment);
    INSERT INTO sound_files_fts(rowid, relative_path, filename, title, artist, album, genre, comment)
    VALUES (NEW.id, NEW.relative_path, NEW.filename, NEW.title, NEW.artist, NEW.album, NEW.genre, NEW.comment);
END;
"#;
