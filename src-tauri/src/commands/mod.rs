//! Tauri command handlers.
//!
//! This module contains all the IPC command handlers that bridge
//! the frontend to the Rust backend.

pub mod database;
pub mod directories;
pub mod folder;
pub mod indexing;
pub mod mcp;
pub mod search;
pub mod system;

pub use database::*;
pub use directories::*;
pub use folder::*;
pub use indexing::*;
pub use mcp::*;
pub use search::*;
pub use system::*;
