//! Database layer for SFX Root.
//!
//! This module provides SQLite database connectivity and repository
//! implementations for directories, sound files, and search operations.

pub mod connection;
pub mod directory;
pub mod schema;
pub mod search;
pub mod sound_file;

pub use connection::DatabaseConnection;
pub use directory::{Directory, DirectoryRepository};
pub use search::{QueryOptions, SearchRepository};
pub use sound_file::{SoundFile, SoundFileInsert, SoundFileRepository, SoundFileUpdate};
