//! MCP server management commands.

use std::process::Command as StdCommand;
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

use crate::error::{Result, SfxError};
use crate::state::AppState;

/// MCP server status info returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub running: bool,
    pub port: u16,
    pub db_path: Option<String>,
    pub mcp_script_path: Option<String>,
}

/// Shared state for the MCP child process.
pub struct McpState {
    child: Mutex<Option<std::process::Child>>,
    port: Mutex<u16>,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            port: Mutex::new(3839),
        }
    }

    /// Kill the MCP child process if running.
    pub fn kill_child(&self) {
        let mut child_lock = self.child.lock().unwrap();
        if let Some(ref mut child) = *child_lock {
            let _ = child.kill();
            let _ = child.wait();
        }
        *child_lock = None;
    }
}

impl Drop for McpState {
    fn drop(&mut self) {
        self.kill_child();
    }
}

/// Start the MCP server as a child process.
#[tauri::command]
pub async fn mcp_start(
    port: u16,
    app_state: State<'_, AppState>,
    mcp_state: State<'_, McpState>,
) -> Result<McpStatus> {
    // Check if already running
    {
        let mut child_lock = mcp_state.child.lock().unwrap();
        if let Some(ref mut child) = *child_lock {
            // Check if still alive
            match child.try_wait() {
                Ok(None) => {
                    // Still running
                    let db_path = app_state.get_db_path().map(|p| p.display().to_string());
                    return Ok(McpStatus {
                        running: true,
                        port,
                        db_path,
                        mcp_script_path: find_mcp_script().ok(),
                    });
                }
                _ => {
                    // Dead, clear it
                    *child_lock = None;
                }
            }
        }
    }

    let db_path = app_state
        .get_db_path()
        .ok_or(SfxError::DatabaseNotOpen)?;

    let db_path_str = db_path.display().to_string();

    // Find the MCP server script relative to the app
    // In development, it's at ../../mcp-server/dist/index.js relative to src-tauri
    // In production, it's bundled as a resource
    let mcp_script = find_mcp_script()?;

    let mut cmd = StdCommand::new("node");
    cmd.arg(&mcp_script)
        .env("SFXROOT_DB_PATH", &db_path_str)
        .env("MCP_TRANSPORT", "sse")
        .env("MCP_PORT", port.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // On Windows, prevent a console window from flashing and ensure the
    // child process survives without a valid console handle.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd.spawn().map_err(SfxError::Io)?;

    *mcp_state.child.lock().unwrap() = Some(child);
    *mcp_state.port.lock().unwrap() = port;

    Ok(McpStatus {
        running: true,
        port,
        db_path: Some(db_path_str),
        mcp_script_path: Some(mcp_script),
    })
}

/// Stop the MCP server.
#[tauri::command]
pub async fn mcp_stop(mcp_state: State<'_, McpState>) -> Result<()> {
    let mut child_lock = mcp_state.child.lock().unwrap();
    if let Some(ref mut child) = *child_lock {
        let _ = child.kill();
        let _ = child.wait();
    }
    *child_lock = None;
    Ok(())
}

/// Get MCP server status.
#[tauri::command]
pub async fn mcp_status(
    app_state: State<'_, AppState>,
    mcp_state: State<'_, McpState>,
) -> Result<McpStatus> {
    let port = *mcp_state.port.lock().unwrap();
    let db_path = app_state.get_db_path().map(|p| p.display().to_string());

    let mut child_lock = mcp_state.child.lock().unwrap();
    let running = if let Some(ref mut child) = *child_lock {
        match child.try_wait() {
            Ok(None) => true,
            _ => {
                *child_lock = None;
                false
            }
        }
    } else {
        false
    };

    Ok(McpStatus {
        running,
        port,
        db_path,
        mcp_script_path: find_mcp_script().ok(),
    })
}

/// Get the current database path (for display in MCP tab).
#[tauri::command]
pub async fn mcp_get_db_path(app_state: State<'_, AppState>) -> Result<Option<String>> {
    Ok(app_state.get_db_path().map(|p| p.display().to_string()))
}

/// Save the Claude skill file to a user-chosen project directory.
#[tauri::command]
pub async fn mcp_save_skill(app: tauri::AppHandle) -> Result<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app.dialog().file().blocking_pick_folder();
    let folder = match folder {
        Some(f) => f
            .into_path()
            .map_err(|e| SfxError::InvalidPath(e.to_string()))?,
        None => return Ok(None),
    };

    let skill_dir = folder.join(".claude").join("skills").join("sfxroot");
    std::fs::create_dir_all(&skill_dir).map_err(SfxError::Io)?;

    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_path, SKILL_CONTENT).map_err(SfxError::Io)?;

    Ok(Some(skill_path.display().to_string()))
}

const SKILL_CONTENT: &str = r#"---
name: sfxroot
description: Use the SFX Root MCP tools to search for sound effects, get audio file details, copy sounds into your project, and browse the sound library. Trigger when user asks about sounds, SFX, audio files, sound effects, or wants to find/copy/use audio.
---

# SFX Root - Sound Effects Library

You have access to the **sfxroot** MCP server which provides a searchable library of indexed sound effects and audio files (WAV, MP3, OGG, FLAC, AIFF, M4A, Opus, AAC).

## Tools

### `search_sounds`
Full-text search across filenames, paths, titles, artists, albums, genres, and comments.

**Parameters:**
- `query` (string) — search text, e.g. "explosion", "footstep wood", "UI click". Partial words work ("explo" finds "explosion").
- `extension` (string) — filter by format: "mp3", "wav", "flac", "ogg", "aiff", "m4a", "opus", "aac"
- `codec` (string) — filter by codec: "mp3", "aac", "flac", "opus", "vorbis", "pcm"
- `min_duration_secs` / `max_duration_secs` (number) — duration range in seconds
- `channels` (number) — 1 = mono, 2 = stereo
- `min_sample_rate` / `max_sample_rate` (number) — in Hz, e.g. 44100, 48000
- `sort_by` — "filename", "duration", "size", "modified", "sample_rate"
- `sort_order` — "asc" or "desc"
- `limit` (number) — max results, default 50, max 500
- `offset` (number) — for pagination
- `directory_id` (number) — filter to a specific indexed directory

### `list_directories`
Lists all indexed directories with file counts and last sync times. No parameters.

### `sound_stats`
Aggregate stats: total files, total size/duration, breakdowns by codec, sample rate, channels, extension. No parameters.

### `get_sound`
Full details for one sound by `id` (number) or `path` (string).

## How to Find Sounds

1. Call `search_sounds` with descriptive terms matching what the user needs
2. If too many results, narrow with filters (duration, format, channels)
3. If too few, try synonyms or broader terms
4. Use `list_directories` to see what collections exist, then filter by `directory_id`

## How to Copy Sounds Into This Project

Each search result includes a `full_path` — the absolute filesystem path to the audio file. To bring a sound into the current project:

```bash
cp "<full_path>" "<destination_in_project>"
```

For multiple files:
```bash
cp "<path1>" "<path2>" "<path3>" "<destination_dir>/"
```

Always:
- Create the destination directory first if it doesn't exist
- Tell the user which files were copied and where
- Show the filename, duration, and format so they can verify it's what they wanted

## Example Workflow

User: "I need a short explosion sound, WAV format"

1. `search_sounds({ query: "explosion", extension: "wav", max_duration_secs: 3 })`
2. Present the top matches with name, duration, size
3. User picks one (or you pick the best match)
4. `cp "/library/path/explosion_01.wav" "./assets/audio/explosion.wav"`
5. Confirm: "Copied explosion_01.wav (1.2s, 44.1kHz stereo WAV, 210KB) to ./assets/audio/"
"#;

/// Find the MCP server script path.
fn find_mcp_script() -> Result<String> {
    // Try several locations
    let candidates = [
        // Development: relative to the project root
        std::env::current_dir()
            .unwrap_or_default()
            .join("mcp-server/dist/index.js"),
        // Development: relative to src-tauri
        std::env::current_dir()
            .unwrap_or_default()
            .join("../mcp-server/dist/index.js"),
        // Alongside the executable
        std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("mcp-server/dist/index.js"),
        // macOS app bundle: Contents/Resources
        std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("../Resources/mcp-server/dist/index.js"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            // Normalize away ".." segments so the path is clean for Node.js.
            // We avoid std canonicalize() because on Windows it produces
            // \\?\ UNC prefixed paths that Node.js cannot load.
            let path = normalize_path(candidate);
            return Ok(path.display().to_string());
        }
    }

    Err(SfxError::InvalidPath(
        "MCP server script not found. Make sure mcp-server is built (cd mcp-server && npm run build)".to_string(),
    ))
}

/// Resolve `.` and `..` segments without calling `canonicalize()`,
/// which on Windows adds a `\\?\` UNC prefix that breaks Node.js.
fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => { out.pop(); }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}
