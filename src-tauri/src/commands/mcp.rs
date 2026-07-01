//! MCP server management commands.
//!
//! The MCP server is now an in-process axum HTTP+SSE server (see
//! `crate::mcp_server`), so there is no Node sidecar to spawn anymore.

use std::sync::Mutex;

use serde::Serialize;
use tauri::State;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::error::{Result, SfxError};
use crate::mcp_server;
use crate::state::AppState;

/// MCP server status info returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub running: bool,
    pub port: u16,
    pub db_path: Option<String>,
}

struct RunningServer {
    shutdown: oneshot::Sender<()>,
    handle: JoinHandle<()>,
}

/// Shared state for the in-process MCP server.
pub struct McpState {
    inner: Mutex<Option<RunningServer>>,
    port: Mutex<u16>,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            port: Mutex::new(3839),
        }
    }

    /// Stop the running MCP server, if any. Safe to call from non-async
    /// contexts (e.g. the window-destroyed event handler).
    pub fn shutdown(&self) {
        let running = self.inner.lock().unwrap().take();
        if let Some(running) = running {
            let _ = running.shutdown.send(());
            running.handle.abort();
        }
    }
}

impl Drop for McpState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn is_running(state: &McpState) -> bool {
    let mut guard = state.inner.lock().unwrap();
    if let Some(running) = guard.as_ref() {
        if running.handle.is_finished() {
            *guard = None;
            false
        } else {
            true
        }
    } else {
        false
    }
}

/// Start the in-process MCP server on the given port.
#[tauri::command]
pub async fn mcp_start(
    app: tauri::AppHandle,
    port: u16,
    app_state: State<'_, AppState>,
    mcp_state: State<'_, McpState>,
) -> Result<McpStatus> {
    if is_running(&mcp_state) {
        let db_path = app_state.get_db_path().map(|p| p.display().to_string());
        return Ok(McpStatus {
            running: true,
            port: *mcp_state.port.lock().unwrap(),
            db_path,
        });
    }

    if !app_state.is_db_open() {
        return Err(SfxError::DatabaseNotOpen);
    }

    let (handle, shutdown) = mcp_server::start(app.clone(), port)
        .await
        .map_err(SfxError::Io)?;

    *mcp_state.inner.lock().unwrap() = Some(RunningServer { shutdown, handle });
    *mcp_state.port.lock().unwrap() = port;

    let db_path = app_state.get_db_path().map(|p| p.display().to_string());
    Ok(McpStatus {
        running: true,
        port,
        db_path,
    })
}

/// Stop the MCP server.
#[tauri::command]
pub async fn mcp_stop(mcp_state: State<'_, McpState>) -> Result<()> {
    mcp_state.shutdown();
    Ok(())
}

/// Get MCP server status.
#[tauri::command]
pub async fn mcp_status(
    app_state: State<'_, AppState>,
    mcp_state: State<'_, McpState>,
) -> Result<McpStatus> {
    let running = is_running(&mcp_state);
    let port = *mcp_state.port.lock().unwrap();
    let db_path = app_state.get_db_path().map(|p| p.display().to_string());

    Ok(McpStatus {
        running,
        port,
        db_path,
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
