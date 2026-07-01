//! In-process MCP (Model Context Protocol) server.
//!
//! Replaces the former Node.js sidecar. Speaks MCP JSON-RPC over an
//! HTTP + SSE transport (`/sse` for the event stream, `/messages` for
//! client→server messages), so external Claude Code clients connect
//! the same way as before with `claude mcp add ... --transport sse`.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::{TimeZone, Utc};
use futures_util::stream::Stream;
use rusqlite::{params_from_iter, Connection, OpenFlags};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use tauri::Manager;

use crate::state::AppState;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "sfxroot";
const SERVER_VERSION: &str = "1.0.0";

type SessionMap = Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<Event>>>>;

#[derive(Clone)]
struct ServerState {
    app: tauri::AppHandle,
    sessions: SessionMap,
}

/// Bind a TCP listener and start the MCP HTTP server on a tokio task.
///
/// Returns a join handle for the server task plus a oneshot sender for
/// graceful shutdown. If binding the port fails, the error is returned
/// to the caller before the task is spawned.
pub async fn start(
    app: tauri::AppHandle,
    port: u16,
) -> std::io::Result<(tokio::task::JoinHandle<()>, oneshot::Sender<()>)> {
    let state = ServerState {
        app,
        sessions: Arc::new(AsyncMutex::new(HashMap::new())),
    };

    let router = Router::new()
        .route("/", get(root_handler))
        .route("/sse", get(sse_handler))
        .route("/messages", post(messages_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok((handle, shutdown_tx))
}

async fn root_handler() -> impl IntoResponse {
    let mut resp = "SFX Root MCP Server. Connect via SSE at /sse".into_response();
    add_cors(resp.headers_mut());
    resp
}

#[derive(Deserialize)]
struct MessagesQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

async fn sse_handler(
    State(state): State<ServerState>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    let session_id = Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::unbounded_channel::<Event>();

    // Per the MCP SSE transport spec, the very first event the server sends
    // tells the client where to POST messages. The data is the relative URL
    // including the session id.
    let endpoint = format!("/messages?sessionId={}", session_id);
    let _ = tx.send(Event::default().event("endpoint").data(endpoint));

    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), tx);

    // When the client disconnects, axum drops the stream which drops the
    // receiver. We can't reliably hook drop here without a guard, so we
    // prune dead senders lazily in messages_handler / send_to_session.
    let stream = UnboundedReceiverStream::new(rx).map(Ok::<_, Infallible>);

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn messages_handler(
    State(state): State<ServerState>,
    Query(q): Query<MessagesQuery>,
    body: String,
) -> impl IntoResponse {
    // Try to parse as a single JSON-RPC message.
    let request: JsonRpcRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            let mut resp = (
                StatusCode::BAD_REQUEST,
                format!("Invalid JSON-RPC body: {}", e),
            )
                .into_response();
            add_cors(resp.headers_mut());
            return resp;
        }
    };

    let response = handle_jsonrpc(&state, request).await;

    if let Some(response) = response {
        let payload = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
        let event = Event::default().event("message").data(payload);
        send_to_session(&state.sessions, &q.session_id, event).await;
    }

    let mut resp = StatusCode::ACCEPTED.into_response();
    add_cors(resp.headers_mut());
    resp
}

async fn send_to_session(sessions: &SessionMap, session_id: &str, event: Event) {
    let mut map = sessions.lock().await;
    let dead = match map.get(session_id) {
        Some(tx) => tx.send(event).is_err(),
        None => false,
    };
    if dead {
        map.remove(session_id);
    }
}

fn add_cors(headers: &mut axum::http::HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type"),
    );
}

// ----------------------------------------------------------------------------
// JSON-RPC layer
// ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    /// Notifications omit `id`; requests carry one.
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcError {
    fn method_not_found(name: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", name),
        }
    }
    fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
        }
    }
}

async fn handle_jsonrpc(state: &ServerState, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    // Notifications (no id) get no response.
    let id = req.id.clone()?;

    let result: std::result::Result<Value, JsonRpcError> = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => handle_tool_call(state, req.params).await,
        "ping" => Ok(json!({})),
        other => Err(JsonRpcError::method_not_found(other)),
    };

    Some(match result {
        Ok(result) => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        },
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        },
    })
}

// ----------------------------------------------------------------------------
// Tool definitions and dispatch
// ----------------------------------------------------------------------------

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "search_sounds",
            "description": "Search for sounds in the SFX Root database. Supports full-text search across filenames, titles, artists, albums, genres, comments, and paths. Can filter by duration range, extension, codec, channels, sample rate, and directory. Returns detailed metadata for each match.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Full-text search query (filename, path, title, artist, album, genre, comment)" },
                    "extension": { "type": "string", "description": "Filter by file extension, e.g. 'mp3', 'wav', 'flac'" },
                    "codec": { "type": "string", "description": "Filter by audio codec, e.g. 'mp3', 'aac', 'flac', 'opus', 'vorbis', 'pcm'" },
                    "min_duration_ms": { "type": "number", "description": "Minimum duration in milliseconds" },
                    "max_duration_ms": { "type": "number", "description": "Maximum duration in milliseconds" },
                    "min_duration_secs": { "type": "number", "description": "Minimum duration in seconds (overrides min_duration_ms)" },
                    "max_duration_secs": { "type": "number", "description": "Maximum duration in seconds (overrides max_duration_ms)" },
                    "channels": { "type": "number", "description": "Filter by number of channels (1=mono, 2=stereo)" },
                    "min_sample_rate": { "type": "number", "description": "Minimum sample rate in Hz, e.g. 44100" },
                    "max_sample_rate": { "type": "number", "description": "Maximum sample rate in Hz" },
                    "directory_id": { "type": "number", "description": "Filter to a specific indexed directory by its ID" },
                    "sort_by": { "type": "string", "enum": ["filename", "duration", "size", "modified", "sample_rate"], "description": "Sort field (default: filename)" },
                    "sort_order": { "type": "string", "enum": ["asc", "desc"], "description": "Sort order (default: asc)" },
                    "limit": { "type": "number", "description": "Max results to return (default: 50, max: 500)" },
                    "offset": { "type": "number", "description": "Offset for pagination" }
                }
            }
        }),
        json!({
            "name": "list_directories",
            "description": "List all indexed directories in the SFX Root database, with file counts and last sync times.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sound_stats",
            "description": "Get statistics about the sound library: total files, total duration, format breakdown, codec breakdown, sample rate distribution, etc.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_sound",
            "description": "Get full details for a specific sound file by its database ID or file path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "number", "description": "Sound file database ID" },
                    "path": { "type": "string", "description": "Full file path to look up" }
                }
            }
        }),
    ]
}

async fn handle_tool_call(
    state: &ServerState,
    params: Value,
) -> std::result::Result<Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("missing tool name"))?
        .to_string();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let db_path = {
        let app_state = state.app.state::<AppState>();
        app_state.get_db_path()
    };

    let db_path = match db_path {
        Some(p) => p,
        None => return Ok(text_result("Error: No database is currently open in SFX Root.")),
    };

    // Run the (sync) sqlite work on a blocking thread.
    let join_result = tokio::task::spawn_blocking(move || -> std::result::Result<String, String> {
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("Error opening database: {}", e))?;
        // Match the main app's pragmas for read consistency.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "cache_size", -65536_i64);

        let result = match name.as_str() {
            "search_sounds" => tool_search_sounds(&conn, &arguments),
            "list_directories" => tool_list_directories(&conn),
            "sound_stats" => tool_sound_stats(&conn),
            "get_sound" => tool_get_sound(&conn, &arguments),
            other => Err(format!("Unknown tool: {}", other)),
        };
        result
    })
    .await;

    let text = match join_result {
        Ok(Ok(text)) => text,
        Ok(Err(msg)) => format!("Error: {}", msg),
        Err(join_err) => return Err(JsonRpcError::internal(join_err.to_string())),
    };

    Ok(text_result(text))
}

fn text_result(text: impl Into<String>) -> Value {
    json!({
        "content": [ { "type": "text", "text": text.into() } ]
    })
}

// ----------------------------------------------------------------------------
// Tool implementations
// ----------------------------------------------------------------------------

#[derive(Debug)]
struct SoundRow {
    id: i64,
    full_path: String,
    filename: String,
    extension: String,
    file_size: i64,
    duration_ms: Option<i64>,
    sample_rate: Option<i64>,
    channels: Option<i64>,
    bit_rate: Option<i64>,
    codec: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    comment: Option<String>,
}

fn row_to_sound(row: &rusqlite::Row) -> rusqlite::Result<SoundRow> {
    Ok(SoundRow {
        id: row.get("id")?,
        full_path: row.get("full_path")?,
        filename: row.get("filename")?,
        extension: row.get("extension")?,
        file_size: row.get("file_size")?,
        duration_ms: row.get("duration_ms")?,
        sample_rate: row.get("sample_rate")?,
        channels: row.get("channels")?,
        bit_rate: row.get("bit_rate")?,
        codec: row.get("codec")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        genre: row.get("genre")?,
        comment: row.get("comment")?,
    })
}

fn tool_search_sounds(conn: &Connection, args: &Value) -> std::result::Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let extension = args.get("extension").and_then(|v| v.as_str());
    let codec = args.get("codec").and_then(|v| v.as_str());
    let directory_id = args.get("directory_id").and_then(|v| v.as_i64());
    let channels = args.get("channels").and_then(|v| v.as_i64());

    let min_dur_secs = args.get("min_duration_secs").and_then(|v| v.as_f64());
    let max_dur_secs = args.get("max_duration_secs").and_then(|v| v.as_f64());
    let min_dur_ms = args.get("min_duration_ms").and_then(|v| v.as_i64());
    let max_dur_ms = args.get("max_duration_ms").and_then(|v| v.as_i64());
    let min_dur = min_dur_secs.map(|s| (s * 1000.0) as i64).or(min_dur_ms);
    let max_dur = max_dur_secs.map(|s| (s * 1000.0) as i64).or(max_dur_ms);

    let min_sample = args.get("min_sample_rate").and_then(|v| v.as_i64());
    let max_sample = args.get("max_sample_rate").and_then(|v| v.as_i64());

    let sort_by = args
        .get("sort_by")
        .and_then(|v| v.as_str())
        .unwrap_or("filename");
    let sort_order = args
        .get("sort_order")
        .and_then(|v| v.as_str())
        .unwrap_or("asc");
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(50)
        .clamp(1, 500);
    let offset = args
        .get("offset")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        .max(0);

    let mut filter_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();

    let uses_fts = query.is_some();
    if let Some(q) = query {
        let fts = q
            .split_whitespace()
            .map(|t| format!("{}*", t))
            .collect::<Vec<_>>()
            .join(" ");
        filter_params.push(Box::new(fts));
    }

    if let Some(d) = directory_id {
        conditions.push("sf.directory_id = ?".into());
        filter_params.push(Box::new(d));
    }
    if let Some(ext) = extension {
        // Scanner stores extensions lowercase, no leading dot (see
        // indexing/scanner.rs). Normalize the caller's input the same way.
        let ext = ext.trim_start_matches('.').to_lowercase();
        conditions.push("sf.extension = ?".into());
        filter_params.push(Box::new(ext));
    }
    if let Some(c) = codec {
        conditions.push("sf.codec = ?".into());
        filter_params.push(Box::new(c.to_string()));
    }
    if let Some(c) = channels {
        conditions.push("sf.channels = ?".into());
        filter_params.push(Box::new(c));
    }
    if let Some(d) = min_dur {
        conditions.push("sf.duration_ms >= ?".into());
        filter_params.push(Box::new(d));
    }
    if let Some(d) = max_dur {
        conditions.push("sf.duration_ms <= ?".into());
        filter_params.push(Box::new(d));
    }
    if let Some(s) = min_sample {
        conditions.push("sf.sample_rate >= ?".into());
        filter_params.push(Box::new(s));
    }
    if let Some(s) = max_sample {
        conditions.push("sf.sample_rate <= ?".into());
        filter_params.push(Box::new(s));
    }

    let select_fields = "sf.id, sf.directory_id, sf.relative_path, sf.filename, \
        sf.full_path, sf.extension, sf.file_size, sf.modified_at, \
        sf.duration_ms, sf.sample_rate, sf.channels, sf.bit_rate, \
        sf.codec, sf.title, sf.artist, sf.album, sf.genre, \
        sf.comment, sf.indexed_at";

    let (base_select, base_count) = if uses_fts {
        (
            format!(
                "SELECT {} FROM sound_files sf \
                 INNER JOIN sound_files_fts fts ON sf.id = fts.rowid \
                 WHERE fts.sound_files_fts MATCH ?",
                select_fields
            ),
            "SELECT COUNT(*) FROM sound_files sf \
             INNER JOIN sound_files_fts fts ON sf.id = fts.rowid \
             WHERE fts.sound_files_fts MATCH ?"
                .to_string(),
        )
    } else {
        (
            format!("SELECT {} FROM sound_files sf WHERE 1=1", select_fields),
            "SELECT COUNT(*) FROM sound_files sf WHERE 1=1".to_string(),
        )
    };

    let conditions_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    };

    let sort_col = match sort_by {
        "duration" => "sf.duration_ms",
        "size" => "sf.file_size",
        "modified" => "sf.modified_at",
        "sample_rate" => "sf.sample_rate",
        _ => "sf.filename_lower",
    };
    let sort_dir = if sort_order == "desc" { "DESC" } else { "ASC" };

    let count_sql = format!("{}{}", base_count, conditions_sql);
    let main_sql = format!(
        "{}{} ORDER BY {} {} LIMIT ? OFFSET ?",
        base_select, conditions_sql, sort_col, sort_dir
    );

    // Count first (using filter_params alone).
    let count_refs: Vec<&dyn rusqlite::ToSql> =
        filter_params.iter().map(|b| b.as_ref()).collect();
    let total: i64 = conn
        .query_row(&count_sql, params_from_iter(count_refs), |row| row.get(0))
        .map_err(|e| e.to_string())?;

    // Then build the data params with limit/offset appended.
    let mut data_params = filter_params;
    data_params.push(Box::new(limit));
    data_params.push(Box::new(offset));
    let data_refs: Vec<&dyn rusqlite::ToSql> = data_params.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&main_sql).map_err(|e| e.to_string())?;
    let rows: Vec<SoundRow> = stmt
        .query_map(params_from_iter(data_refs), row_to_sound)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        let total_in_db: i64 = conn
            .query_row("SELECT COUNT(*) FROM sound_files", [], |row| row.get(0))
            .unwrap_or(0);
        return Ok(format!(
            "No sounds found matching your criteria. Total sounds in database: {}",
            total_in_db
        ));
    }

    let header = if total > limit {
        format!(
            "Found {} sounds (showing {}-{}):\n",
            total,
            offset,
            offset + rows.len() as i64 - 1
        )
    } else {
        format!("Found {} sounds:\n", total)
    };
    let body = rows
        .iter()
        .map(format_sound)
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(format!("{}\n{}", header, body))
}

fn tool_list_directories(conn: &Connection) -> std::result::Result<String, String> {
    let mut stmt = conn
        .prepare("SELECT id, path, file_count, last_synced_at FROM directories ORDER BY path")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(i64, String, i64, Option<i64>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Ok("No directories indexed yet.".to_string());
    }

    let lines: Vec<String> = rows
        .iter()
        .map(|(id, path, count, synced)| {
            let synced_str = synced
                .map(format_unix_iso)
                .unwrap_or_else(|| "never".to_string());
            format!(
                "- **{}** (ID: {})\n  {} files | Last synced: {}",
                path, id, count, synced_str
            )
        })
        .collect();

    Ok(format!("Indexed directories:\n\n{}", lines.join("\n\n")))
}

fn tool_sound_stats(conn: &Connection) -> std::result::Result<String, String> {
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM sound_files", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let total_size: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM sound_files",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let total_duration: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(duration_ms), 0) FROM sound_files",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let avg_duration: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(duration_ms), 0) FROM sound_files WHERE duration_ms IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let dirs: i64 = conn
        .query_row("SELECT COUNT(*) FROM directories", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    let by_ext = collect_pairs::<String>(
        conn,
        "SELECT extension, COUNT(*) FROM sound_files GROUP BY extension ORDER BY COUNT(*) DESC",
    )?;
    let by_codec = collect_pairs::<String>(
        conn,
        "SELECT codec, COUNT(*) FROM sound_files WHERE codec IS NOT NULL GROUP BY codec ORDER BY COUNT(*) DESC",
    )?;
    let by_sr = collect_pairs::<i64>(
        conn,
        "SELECT sample_rate, COUNT(*) FROM sound_files WHERE sample_rate IS NOT NULL GROUP BY sample_rate ORDER BY COUNT(*) DESC",
    )?;
    let by_ch = collect_pairs::<i64>(
        conn,
        "SELECT channels, COUNT(*) FROM sound_files WHERE channels IS NOT NULL GROUP BY channels ORDER BY COUNT(*) DESC",
    )?;

    let mut out = String::new();
    out.push_str("## Sound Library Statistics\n\n");
    out.push_str(&format!("- **Total files:** {}\n", format_int(total)));
    out.push_str(&format!("- **Total size:** {}\n", format_size(total_size)));
    out.push_str(&format!(
        "- **Total duration:** {}\n",
        format_duration(Some(total_duration))
    ));
    out.push_str(&format!(
        "- **Average duration:** {}\n",
        format_duration(Some(avg_duration.round() as i64))
    ));
    out.push_str(&format!("- **Indexed directories:** {}\n\n", dirs));

    out.push_str("### By Extension\n");
    for (k, c) in &by_ext {
        out.push_str(&format!("- {}: {}\n", k, format_int(*c)));
    }
    out.push_str("\n### By Codec\n");
    for (k, c) in &by_codec {
        out.push_str(&format!("- {}: {}\n", k, format_int(*c)));
    }
    out.push_str("\n### By Sample Rate\n");
    for (k, c) in &by_sr {
        out.push_str(&format!("- {}Hz: {}\n", k, format_int(*c)));
    }
    out.push_str("\n### By Channels\n");
    for (k, c) in &by_ch {
        out.push_str(&format!("- {}ch: {}\n", k, format_int(*c)));
    }

    Ok(out)
}

fn collect_pairs<K>(conn: &Connection, sql: &str) -> std::result::Result<Vec<(K, i64)>, String>
where
    K: rusqlite::types::FromSql + std::fmt::Display,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, K>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn tool_get_sound(conn: &Connection, args: &Value) -> std::result::Result<String, String> {
    let id = args.get("id").and_then(|v| v.as_i64());
    let path = args.get("path").and_then(|v| v.as_str());

    if id.is_none() && path.is_none() {
        return Ok("Error: Provide either id or path.".to_string());
    }

    let select = "SELECT sf.id, sf.directory_id, sf.relative_path, sf.filename, \
                  sf.full_path, sf.extension, sf.file_size, sf.modified_at, \
                  sf.duration_ms, sf.sample_rate, sf.channels, sf.bit_rate, \
                  sf.codec, sf.title, sf.artist, sf.album, sf.genre, \
                  sf.comment, sf.indexed_at FROM sound_files sf";

    let row: rusqlite::Result<SoundRow> = if let Some(id) = id {
        conn.query_row(
            &format!("{} WHERE sf.id = ?", select),
            [id],
            row_to_sound,
        )
    } else {
        conn.query_row(
            &format!("{} WHERE sf.full_path = ?", select),
            [path.unwrap()],
            row_to_sound,
        )
    };

    match row {
        Ok(s) => Ok(format_sound(&s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok("Sound not found.".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

// ----------------------------------------------------------------------------
// Formatting helpers
// ----------------------------------------------------------------------------

fn format_duration(ms: Option<i64>) -> String {
    match ms {
        None => "unknown".to_string(),
        Some(ms) => {
            let secs = ms as f64 / 1000.0;
            if secs < 60.0 {
                format!("{:.1}s", secs)
            } else {
                let m = (secs / 60.0).floor() as i64;
                let s = (secs % 60.0).round() as i64;
                format!("{}m{:02}s", m, s)
            }
        }
    }
}

fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_int(n: i64) -> String {
    // Insert thousands separators without bringing a locale crate.
    let s = n.to_string();
    let (sign, digits) = if let Some(rest) = s.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", s.as_str())
    };
    let bytes: Vec<char> = digits.chars().collect();
    let mut out = String::new();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    format!("{}{}", sign, out)
}

fn format_unix_iso(secs: i64) -> String {
    Utc.timestamp_opt(secs, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("{} (unix)", secs))
}

fn format_sound(s: &SoundRow) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(5);
    parts.push(format!("**{}**", s.filename));
    parts.push(format!("  Path: {}", s.full_path));
    parts.push(format!(
        "  Duration: {} | Size: {} | Format: {}",
        format_duration(s.duration_ms),
        format_size(s.file_size),
        s.extension
    ));

    if s.sample_rate.is_some()
        || s.channels.is_some()
        || s.bit_rate.is_some()
        || s.codec.is_some()
    {
        let mut tech: Vec<String> = Vec::new();
        if let Some(c) = &s.codec {
            tech.push(format!("Codec: {}", c));
        }
        if let Some(sr) = s.sample_rate {
            tech.push(format!("{}Hz", sr));
        }
        if let Some(ch) = s.channels {
            tech.push(format!("{}ch", ch));
        }
        if let Some(br) = s.bit_rate {
            tech.push(format!("{}bps", br));
        }
        parts.push(format!("  {}", tech.join(" | ")));
    }

    let mut meta: Vec<String> = Vec::new();
    if let Some(t) = &s.title {
        meta.push(format!("Title: {}", t));
    }
    if let Some(a) = &s.artist {
        meta.push(format!("Artist: {}", a));
    }
    if let Some(a) = &s.album {
        meta.push(format!("Album: {}", a));
    }
    if let Some(g) = &s.genre {
        meta.push(format!("Genre: {}", g));
    }
    if let Some(c) = &s.comment {
        meta.push(format!("Comment: {}", c));
    }
    if !meta.is_empty() {
        parts.push(format!("  {}", meta.join(" | ")));
    }

    // Suppress unused-field warning for `id`; it's part of the row but not
    // emitted in the formatted text.
    let _ = s.id;

    parts.join("\n")
}
