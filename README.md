# SFX Root

A desktop sound effect library manager. Index your audio collections, search and browse them instantly, and expose your library to AI coding assistants via a built-in MCP server.

Built with [Tauri 2](https://tauri.app/), [SvelteKit](https://kit.svelte.dev/), and [Rust](https://www.rust-lang.org/).

## Features

- **Index audio libraries** — Point SFX Root at directories containing audio files (WAV, MP3, FLAC, OGG, AIFF, M4A, Opus, AAC). It scans them in parallel, extracts metadata (duration, sample rate, channels, codec, ID3/Vorbis tags), and stores everything in a local SQLite database.
- **Search and browse** — Full-text search across filenames, paths, titles, artists, albums, genres, and comments. Filter by format, codec, duration, sample rate, and channels. Sort by filename, duration, size, date modified, or sample rate.
- **MCP server** — Start a built-in [Model Context Protocol](https://modelcontextprotocol.io/) server that lets AI assistants (Claude Code, Cursor, etc.) search your sound library, get file details, and copy sounds into projects.
- **Claude Code skill** — One-click install of a Claude Code skill file so Claude automatically knows how to use your sound library when you ask about sounds or SFX.
- **Cross-platform** — Runs on Windows, macOS, and Linux. No external dependencies like FFmpeg — audio metadata extraction is pure Rust (symphonia + lofty).

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform

## Getting Started

```bash
# Clone the repository
git clone https://github.com/ogomez92/sfxroot.git
cd sfxroot

# Install frontend dependencies
pnpm install

# Build the MCP server
cd mcp-server
pnpm install
pnpm build
cd ..

# Run in development mode
pnpm tauri dev
```

## Building for Production

```bash
pnpm tauri build
```

The installer/bundle will be output to `src-tauri/target/release/bundle/`.

## Usage

### Indexing

1. Open or create a database file (`.db`) — this is where your indexed metadata is stored.
2. Switch to the **Indexing** tab and add one or more directories containing audio files.
3. Click **Start Indexing** to scan. Progress is shown in real-time. You can cancel and resume at any time.
4. Use **Resync** to pick up new or changed files without re-scanning everything.

### Viewer

Switch to the **Viewer** tab to browse and search your indexed sounds. You can:

- Type to search across all metadata fields
- Filter by format, codec, duration, channels, and sample rate
- Sort results by various fields
- Copy file paths to the clipboard
- Open files in your system file explorer

### MCP Server

The **MCP** tab lets you start a local MCP server that exposes your sound library to AI assistants.

1. Make sure a database is open with indexed sounds.
2. Click **Start** to launch the MCP server (default port 3839).
3. Configure your AI assistant to connect to the MCP server at `http://localhost:3839/sse`.
4. Optionally, click **Save Skill** to install a Claude Code skill file into any project, so Claude knows how to use your sound library automatically.

#### Available MCP Tools

| Tool | Description |
|------|-------------|
| `search_sounds` | Full-text search with filters for format, duration, channels, sample rate, and more |
| `list_directories` | List all indexed directories with file counts |
| `sound_stats` | Aggregate statistics: total files, size, duration, breakdowns by codec/format |
| `get_sound` | Full details for a single sound by ID or path |

## Project Structure

```
sfxroot/
├── src/                  # SvelteKit frontend (Svelte 5 + Tailwind CSS)
│   ├── lib/components/   # UI components
│   └── routes/           # App pages
├── src-tauri/            # Tauri + Rust backend
│   └── src/
│       ├── commands/     # Tauri command handlers
│       ├── db/           # SQLite database layer
│       └── indexing/     # File scanning & metadata extraction
└── mcp-server/           # Node.js MCP server
    └── src/              # TypeScript source
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop framework | Tauri 2 |
| Frontend | SvelteKit, Svelte 5, Tailwind CSS 4 |
| Backend | Rust |
| Database | SQLite (rusqlite) |
| Audio metadata | symphonia, lofty |
| MCP server | @modelcontextprotocol/sdk, better-sqlite3 |

## License

[MIT](LICENSE)
