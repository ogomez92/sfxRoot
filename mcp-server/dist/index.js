#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { SSEServerTransport } from "@modelcontextprotocol/sdk/server/sse.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import Database from "better-sqlite3";
import { z } from "zod";
import http from "node:http";
// --- Database path from env or CLI arg ---
const DB_PATH = process.env.SFXROOT_DB_PATH || process.argv[2];
function openDb(path) {
    const db = new Database(path, { readonly: true });
    db.pragma("journal_mode = WAL");
    db.pragma("cache_size = -65536");
    return db;
}
// --- Formatting helpers ---
function formatDuration(ms) {
    if (ms == null)
        return "unknown";
    const secs = ms / 1000;
    if (secs < 60)
        return `${secs.toFixed(1)}s`;
    const m = Math.floor(secs / 60);
    const s = Math.round(secs % 60);
    return `${m}m${s.toString().padStart(2, "0")}s`;
}
function formatSize(bytes) {
    if (bytes < 1024)
        return `${bytes}B`;
    if (bytes < 1024 * 1024)
        return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}
function formatSound(s) {
    const parts = [
        `**${s.filename}**`,
        `  Path: ${s.full_path}`,
        `  Duration: ${formatDuration(s.duration_ms)} | Size: ${formatSize(s.file_size)} | Format: ${s.extension}`,
    ];
    if (s.sample_rate || s.channels || s.bit_rate || s.codec) {
        const tech = [];
        if (s.codec)
            tech.push(`Codec: ${s.codec}`);
        if (s.sample_rate)
            tech.push(`${s.sample_rate}Hz`);
        if (s.channels)
            tech.push(`${s.channels}ch`);
        if (s.bit_rate)
            tech.push(`${s.bit_rate}bps`);
        parts.push(`  ${tech.join(" | ")}`);
    }
    const meta = [];
    if (s.title)
        meta.push(`Title: ${s.title}`);
    if (s.artist)
        meta.push(`Artist: ${s.artist}`);
    if (s.album)
        meta.push(`Album: ${s.album}`);
    if (s.genre)
        meta.push(`Genre: ${s.genre}`);
    if (s.comment)
        meta.push(`Comment: ${s.comment}`);
    if (meta.length > 0)
        parts.push(`  ${meta.join(" | ")}`);
    return parts.join("\n");
}
// --- Build the MCP server ---
const server = new McpServer({
    name: "sfxroot",
    version: "1.0.0",
});
// Tool: search_sounds - the main workhorse
server.tool("search_sounds", `Search for sounds in the SFX Root database. Supports full-text search across filenames, titles, artists, albums, genres, comments, and paths. Can filter by duration range, extension, codec, channels, sample rate, and directory. Returns detailed metadata for each match.`, {
    query: z.string().optional().describe("Full-text search query (searches filename, path, title, artist, album, genre, comment)"),
    extension: z.string().optional().describe("Filter by file extension, e.g. 'mp3', 'wav', 'flac'"),
    codec: z.string().optional().describe("Filter by audio codec, e.g. 'mp3', 'aac', 'flac', 'opus', 'vorbis', 'pcm'"),
    min_duration_ms: z.number().optional().describe("Minimum duration in milliseconds"),
    max_duration_ms: z.number().optional().describe("Maximum duration in milliseconds"),
    min_duration_secs: z.number().optional().describe("Minimum duration in seconds (convenience, overrides min_duration_ms)"),
    max_duration_secs: z.number().optional().describe("Maximum duration in seconds (convenience, overrides max_duration_ms)"),
    channels: z.number().optional().describe("Filter by number of channels (1=mono, 2=stereo)"),
    min_sample_rate: z.number().optional().describe("Minimum sample rate in Hz, e.g. 44100"),
    max_sample_rate: z.number().optional().describe("Maximum sample rate in Hz"),
    directory_id: z.number().optional().describe("Filter to a specific indexed directory by its ID"),
    sort_by: z.enum(["filename", "duration", "size", "modified", "sample_rate"]).optional().describe("Sort results by field (default: filename)"),
    sort_order: z.enum(["asc", "desc"]).optional().describe("Sort order (default: asc)"),
    limit: z.number().optional().describe("Max results to return (default: 50, max: 500)"),
    offset: z.number().optional().describe("Offset for pagination"),
}, async (params) => {
    if (!DB_PATH) {
        return { content: [{ type: "text", text: "Error: No database path configured. Set SFXROOT_DB_PATH env var or pass path as CLI argument." }] };
    }
    let db;
    try {
        db = openDb(DB_PATH);
    }
    catch (e) {
        return { content: [{ type: "text", text: `Error opening database: ${e.message}` }] };
    }
    try {
        const sqlParams = [];
        const conditions = [];
        let usesFts = false;
        // FTS search
        if (params.query?.trim()) {
            usesFts = true;
            const ftsQuery = params.query.trim().split(/\s+/).map((t) => `${t}*`).join(" ");
            sqlParams.push(ftsQuery);
        }
        // Filters
        if (params.directory_id != null) {
            conditions.push("sf.directory_id = ?");
            sqlParams.push(params.directory_id);
        }
        if (params.extension) {
            conditions.push("sf.extension = ?");
            sqlParams.push(params.extension.startsWith(".") ? params.extension : `.${params.extension}`);
        }
        if (params.codec) {
            conditions.push("sf.codec = ?");
            sqlParams.push(params.codec);
        }
        if (params.channels != null) {
            conditions.push("sf.channels = ?");
            sqlParams.push(params.channels);
        }
        // Duration (seconds convenience takes priority)
        const minDur = params.min_duration_secs != null ? params.min_duration_secs * 1000 : params.min_duration_ms;
        const maxDur = params.max_duration_secs != null ? params.max_duration_secs * 1000 : params.max_duration_ms;
        if (minDur != null) {
            conditions.push("sf.duration_ms >= ?");
            sqlParams.push(minDur);
        }
        if (maxDur != null) {
            conditions.push("sf.duration_ms <= ?");
            sqlParams.push(maxDur);
        }
        // Sample rate
        if (params.min_sample_rate != null) {
            conditions.push("sf.sample_rate >= ?");
            sqlParams.push(params.min_sample_rate);
        }
        if (params.max_sample_rate != null) {
            conditions.push("sf.sample_rate <= ?");
            sqlParams.push(params.max_sample_rate);
        }
        // Build SQL
        const selectFields = `sf.id, sf.directory_id, sf.relative_path, sf.filename,
        sf.full_path, sf.extension, sf.file_size, sf.modified_at,
        sf.duration_ms, sf.sample_rate, sf.channels, sf.bit_rate,
        sf.codec, sf.title, sf.artist, sf.album, sf.genre,
        sf.comment, sf.indexed_at`;
        let sql;
        if (usesFts) {
            sql = `SELECT ${selectFields} FROM sound_files sf
          INNER JOIN sound_files_fts fts ON sf.id = fts.rowid
          WHERE fts.sound_files_fts MATCH ?`;
        }
        else {
            sql = `SELECT ${selectFields} FROM sound_files sf WHERE 1=1`;
        }
        if (conditions.length > 0) {
            sql += ` AND ${conditions.join(" AND ")}`;
        }
        // Sort
        const sortCol = {
            filename: "sf.filename_lower",
            duration: "sf.duration_ms",
            size: "sf.file_size",
            modified: "sf.modified_at",
            sample_rate: "sf.sample_rate",
        }[params.sort_by || "filename"] || "sf.filename_lower";
        sql += ` ORDER BY ${sortCol} ${params.sort_order === "desc" ? "DESC" : "ASC"}`;
        // Pagination
        const limit = Math.min(params.limit || 50, 500);
        sql += ` LIMIT ?`;
        sqlParams.push(limit);
        if (params.offset != null) {
            sql += ` OFFSET ?`;
            sqlParams.push(params.offset);
        }
        // Count query
        let countSql;
        const countParams = sqlParams.slice(0, sqlParams.length - (params.offset != null ? 2 : 1));
        if (usesFts) {
            countSql = `SELECT COUNT(*) as cnt FROM sound_files sf
          INNER JOIN sound_files_fts fts ON sf.id = fts.rowid
          WHERE fts.sound_files_fts MATCH ?`;
        }
        else {
            countSql = `SELECT COUNT(*) as cnt FROM sound_files sf WHERE 1=1`;
        }
        if (conditions.length > 0) {
            countSql += ` AND ${conditions.join(" AND ")}`;
        }
        const countRow = db.prepare(countSql).get(...countParams);
        const totalCount = countRow?.cnt ?? 0;
        const rows = db.prepare(sql).all(...sqlParams);
        if (rows.length === 0) {
            return { content: [{ type: "text", text: `No sounds found matching your criteria. Total sounds in database: ${db.prepare("SELECT COUNT(*) as cnt FROM sound_files").get()?.cnt ?? 0}` }] };
        }
        const header = `Found ${totalCount} sounds${totalCount > limit ? ` (showing ${params.offset || 0}-${(params.offset || 0) + rows.length - 1})` : ""}:\n`;
        const formatted = rows.map((r) => formatSound(r)).join("\n\n");
        db.close();
        return { content: [{ type: "text", text: header + "\n" + formatted }] };
    }
    catch (e) {
        db.close();
        return { content: [{ type: "text", text: `Error: ${e.message}` }] };
    }
});
// Tool: list_directories - show indexed directories
server.tool("list_directories", "List all indexed directories in the SFX Root database, with file counts and last sync times.", {}, async () => {
    if (!DB_PATH) {
        return { content: [{ type: "text", text: "Error: No database path configured." }] };
    }
    let db;
    try {
        db = openDb(DB_PATH);
    }
    catch (e) {
        return { content: [{ type: "text", text: `Error opening database: ${e.message}` }] };
    }
    try {
        const rows = db.prepare("SELECT * FROM directories ORDER BY path").all();
        if (rows.length === 0) {
            db.close();
            return { content: [{ type: "text", text: "No directories indexed yet." }] };
        }
        const lines = rows.map((d) => {
            const synced = d.last_synced_at ? new Date(d.last_synced_at * 1000).toISOString() : "never";
            return `- **${d.path}** (ID: ${d.id})\n  ${d.file_count} files | Last synced: ${synced}`;
        });
        db.close();
        return { content: [{ type: "text", text: `Indexed directories:\n\n${lines.join("\n\n")}` }] };
    }
    catch (e) {
        db.close();
        return { content: [{ type: "text", text: `Error: ${e.message}` }] };
    }
});
// Tool: sound_stats - database statistics
server.tool("sound_stats", "Get statistics about the sound library: total files, total duration, format breakdown, codec breakdown, sample rate distribution, etc.", {}, async () => {
    if (!DB_PATH) {
        return { content: [{ type: "text", text: "Error: No database path configured." }] };
    }
    let db;
    try {
        db = openDb(DB_PATH);
    }
    catch (e) {
        return { content: [{ type: "text", text: `Error opening database: ${e.message}` }] };
    }
    try {
        const total = db.prepare("SELECT COUNT(*) as cnt FROM sound_files").get().cnt;
        const totalSize = db.prepare("SELECT COALESCE(SUM(file_size), 0) as s FROM sound_files").get().s;
        const totalDuration = db.prepare("SELECT COALESCE(SUM(duration_ms), 0) as d FROM sound_files").get().d;
        const avgDuration = db.prepare("SELECT COALESCE(AVG(duration_ms), 0) as d FROM sound_files WHERE duration_ms IS NOT NULL").get().d;
        const dirs = db.prepare("SELECT COUNT(*) as cnt FROM directories").get().cnt;
        const byExt = db.prepare("SELECT extension, COUNT(*) as cnt FROM sound_files GROUP BY extension ORDER BY cnt DESC").all();
        const byCodec = db.prepare("SELECT codec, COUNT(*) as cnt FROM sound_files WHERE codec IS NOT NULL GROUP BY codec ORDER BY cnt DESC").all();
        const bySampleRate = db.prepare("SELECT sample_rate, COUNT(*) as cnt FROM sound_files WHERE sample_rate IS NOT NULL GROUP BY sample_rate ORDER BY cnt DESC").all();
        const byChannels = db.prepare("SELECT channels, COUNT(*) as cnt FROM sound_files WHERE channels IS NOT NULL GROUP BY channels ORDER BY cnt DESC").all();
        const lines = [
            `## Sound Library Statistics`,
            ``,
            `- **Total files:** ${total.toLocaleString()}`,
            `- **Total size:** ${formatSize(totalSize)}`,
            `- **Total duration:** ${formatDuration(totalDuration)}`,
            `- **Average duration:** ${formatDuration(Math.round(avgDuration))}`,
            `- **Indexed directories:** ${dirs}`,
            ``,
            `### By Extension`,
            ...byExt.map((r) => `- ${r.extension}: ${r.cnt.toLocaleString()}`),
            ``,
            `### By Codec`,
            ...byCodec.map((r) => `- ${r.codec}: ${r.cnt.toLocaleString()}`),
            ``,
            `### By Sample Rate`,
            ...bySampleRate.map((r) => `- ${r.sample_rate}Hz: ${r.cnt.toLocaleString()}`),
            ``,
            `### By Channels`,
            ...byChannels.map((r) => `- ${r.channels}ch: ${r.cnt.toLocaleString()}`),
        ];
        db.close();
        return { content: [{ type: "text", text: lines.join("\n") }] };
    }
    catch (e) {
        db.close();
        return { content: [{ type: "text", text: `Error: ${e.message}` }] };
    }
});
// Tool: get_sound - get full details for a single sound by ID or path
server.tool("get_sound", "Get full details for a specific sound file by its database ID or file path.", {
    id: z.number().optional().describe("Sound file database ID"),
    path: z.string().optional().describe("Full file path to look up"),
}, async (params) => {
    if (!DB_PATH) {
        return { content: [{ type: "text", text: "Error: No database path configured." }] };
    }
    if (params.id == null && !params.path) {
        return { content: [{ type: "text", text: "Error: Provide either id or path." }] };
    }
    let db;
    try {
        db = openDb(DB_PATH);
    }
    catch (e) {
        return { content: [{ type: "text", text: `Error opening database: ${e.message}` }] };
    }
    try {
        let row;
        if (params.id != null) {
            row = db.prepare("SELECT * FROM sound_files WHERE id = ?").get(params.id);
        }
        else {
            row = db.prepare("SELECT * FROM sound_files WHERE full_path = ?").get(params.path);
        }
        db.close();
        if (!row) {
            return { content: [{ type: "text", text: "Sound not found." }] };
        }
        return { content: [{ type: "text", text: formatSound(row) }] };
    }
    catch (e) {
        db.close();
        return { content: [{ type: "text", text: `Error: ${e.message}` }] };
    }
});
// --- Start server ---
async function main() {
    const mode = process.env.MCP_TRANSPORT || "stdio";
    if (mode === "http" || mode === "sse") {
        const port = parseInt(process.env.MCP_PORT || "3839", 10);
        if (mode === "sse") {
            // SSE transport for remote access
            const sessions = new Map();
            const httpServer = http.createServer(async (req, res) => {
                // CORS for remote access
                res.setHeader("Access-Control-Allow-Origin", "*");
                res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
                res.setHeader("Access-Control-Allow-Headers", "Content-Type");
                if (req.method === "OPTIONS") {
                    res.writeHead(204);
                    res.end();
                    return;
                }
                const url = new URL(req.url || "/", `http://localhost:${port}`);
                if (url.pathname === "/sse" && req.method === "GET") {
                    const transport = new SSEServerTransport("/messages", res);
                    sessions.set(transport.sessionId, transport);
                    res.on("close", () => sessions.delete(transport.sessionId));
                    await server.connect(transport);
                }
                else if (url.pathname === "/messages" && req.method === "POST") {
                    const sessionId = url.searchParams.get("sessionId");
                    const transport = sessionId ? sessions.get(sessionId) : undefined;
                    if (transport) {
                        let body = "";
                        req.on("data", (chunk) => (body += chunk));
                        req.on("end", async () => {
                            try {
                                await transport.handlePostMessage(req, res, body);
                            }
                            catch {
                                res.writeHead(500);
                                res.end("Internal error");
                            }
                        });
                    }
                    else {
                        res.writeHead(404);
                        res.end("Session not found");
                    }
                }
                else {
                    res.writeHead(200, { "Content-Type": "text/plain" });
                    res.end(`SFX Root MCP Server running. Connect via SSE at /sse\nDB: ${DB_PATH || "not configured"}`);
                }
            });
            httpServer.listen(port, "0.0.0.0", () => {
                console.error(`SFX Root MCP server (SSE) listening on http://0.0.0.0:${port}`);
                console.error(`Connect via: http://localhost:${port}/sse`);
                if (DB_PATH)
                    console.error(`Database: ${DB_PATH}`);
            });
        }
        else {
            // Streamable HTTP transport
            const httpServer = http.createServer(async (req, res) => {
                res.setHeader("Access-Control-Allow-Origin", "*");
                res.setHeader("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS");
                res.setHeader("Access-Control-Allow-Headers", "Content-Type");
                if (req.method === "OPTIONS") {
                    res.writeHead(204);
                    res.end();
                    return;
                }
                if (req.url === "/mcp" || req.url?.startsWith("/mcp?")) {
                    const transport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined });
                    await server.connect(transport);
                    await transport.handleRequest(req, res);
                }
                else {
                    res.writeHead(200, { "Content-Type": "text/plain" });
                    res.end(`SFX Root MCP Server running. POST to /mcp\nDB: ${DB_PATH || "not configured"}`);
                }
            });
            httpServer.listen(port, "0.0.0.0", () => {
                console.error(`SFX Root MCP server (HTTP) listening on http://0.0.0.0:${port}`);
                console.error(`Endpoint: http://localhost:${port}/mcp`);
                if (DB_PATH)
                    console.error(`Database: ${DB_PATH}`);
            });
        }
    }
    else {
        // Default: stdio transport (for local Claude Code)
        const transport = new StdioServerTransport();
        await server.connect(transport);
        console.error("SFX Root MCP server running on stdio");
        if (DB_PATH)
            console.error(`Database: ${DB_PATH}`);
    }
}
main().catch((e) => {
    console.error("Fatal:", e);
    process.exit(1);
});
