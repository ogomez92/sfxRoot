//! Indexing engine for audio file discovery and metadata extraction.
//!
//! This module provides parallel directory scanning and native audio
//! metadata extraction using pure Rust libraries (symphonia, lofty).

pub mod metadata;
pub mod progress;
pub mod scanner;
pub mod service;

pub use metadata::{extract_batch, extract_metadata, AudioMetadata, MetadataResult};
pub use progress::{IndexingProgress, IndexingResult, ResyncStats};
pub use scanner::{scan_directory, scan_directory_sorted, ScannedFile, SUPPORTED_EXTENSIONS};
pub use service::{cancel_indexing, IndexingService};
