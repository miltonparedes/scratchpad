# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Scratchpad (`sp`) is a lightweight workspace manager for AI agent sessions. The core idea: each "session" is a directory with markdown files where you organize work with AI agents (Claude, Codex). The TUI lets you create sessions, preview notes, and launch agents directly into a session's directory. It supports both a global user workspace (`~/scratchpad`) and per-project workspaces (`.scratchpad/`), so sessions can be scoped to a repo or shared across projects.

Cargo workspace with two crates:

- **scratchpad** (binary: `sp`) — CLI + TUI for creating, browsing, and managing sessions
- **server** (binary: `sp-server`) — Axum-based relay server with SQLite for session sync (in development)

### Roadmap / Backlog (post-MVP)

Full-text search, session templates, export to Markdown/JSON, session migration between user and project contexts, cross-context merge. See `.scratchpad/SPEC.md` for details.

## Build & Run Commands

El proyecto usa un **Justfile**. Correr `just` para ver todas las recetas.

```bash
just build              # Build workspace
just test               # Run all tests
just test-one <name>    # Run a single test by name
just check              # Lint (clippy) + format check
just fmt                # Auto-format
just install            # Install sp binary from local source
just install-server     # Install sp-server
just run <args>         # Launch TUI / run subcommands (e.g. just run list)
just serve              # Start sync server
just release            # Build in release mode
just clean              # Clean artifacts
```

## Rust Edition

Both crates use **Rust edition 2024** — be aware of edition-specific syntax changes (e.g., `gen` is a reserved keyword).

## Architecture

### Dual Context System

Sessions live in one of two contexts, resolved at startup:

- **User context**: global workspace at `~/scratchpad` (configurable via `~/.config/scratchpad/config.toml`)
- **Project context**: local `.scratchpad/` directory, found by walking up from CWD

CLI flags `--user` / `--project` force a context. Without flags, project context is preferred if a `.scratchpad/` directory exists in any ancestor. The TUI supports switching between contexts with `g`.

### Session Storage Model

Sessions are **directories** inside the workspace, not database entries. Each session directory contains markdown files. Metadata (timestamps) comes from filesystem metadata — there's no manifest or metadata file.

Entry point resolution priority: `main.md` > `notes.md` > `readme.md` > `README.md` > first `.md` alphabetically. If no markdown file exists, the TUI shows a file listing instead.

Session names support prefix matching throughout the codebase (CLI and TUI).

### Name Generation

`names.rs` generates session names via a cascade: LLM CLI tools (claude `--print` / codex `--quiet`) → static adjective-noun combos. A name cache at `~/.config/scratchpad/name-cache.txt` (last 10 names) prevents reuse. Config field `name_generator` controls the strategy: `auto` (try claude then codex), `claude`, `codex`, or `static`.

### TUI Architecture

The TUI (`tui/`) uses ratatui + crossterm with a modal architecture:

- **Modes**: Normal, Search, NewSession, QuickSession, Help — each has its own key handler in `app.rs`
- **Focus**: List or Detail panel — `Tab` switches, border color indicates active focus
- **Actions**: `handle_key()` returns an `Action` enum. The event loop in `tui/mod.rs` matches on these to perform side effects (run agent, open editor, etc.)
- External editors/agents temporarily exit the TUI (disable raw mode, leave alternate screen), then re-enter after the process exits

### Markdown Rendering

`markdown.rs` prefers shelling out to `glow` CLI for rendering. Falls back to a basic built-in renderer. The `glow` output (ANSI) is converted via `ansi-to-tui`, which produces `ratatui-core` types that must be manually converted to `ratatui` types (the `convert_*` functions at the bottom of the file). This is a version compatibility shim.

### Server (server crate)

Axum HTTP server with SQLite (rusqlite, bundled). Routes under `/api/` for ops and snapshots, plus `/ws` for WebSocket. Database uses `Mutex<Connection>` for thread safety. Schema: `ops` table (append-only operation log) and `snapshots` table. Configured via env vars: `DATABASE_PATH`, `PORT`, `RUST_LOG`.

## Configuration

User config at `~/.config/scratchpad/config.toml`:
- `workspace_path` — where user-context sessions live (default: `~/scratchpad`)
- `default_agent` — `claude` or `codex`
- `editor` / `viewer` — override for edit/view commands (falls back to `EDITOR`/`VISUAL` env vars, then `vi`)
- `name_generator` — `auto`, `claude`, `codex`, or `static`
- `server` — optional `{ url, token }` for sync
