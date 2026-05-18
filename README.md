# Scratchpad

Scratchpad (`sp`) is a small CLI/TUI for organizing AI-agent work sessions.
Each session is a folder with Markdown notes, so the data stays inspectable and
easy to move between tools.

The workspace contains two binaries:

- `sp`: the CLI/TUI used to create, browse, edit, and launch agent sessions.
- `sp-server`: an optional Axum + SQLite sync relay, still in development.

## Requirements

- Rust stable with `clippy` and `rustfmt`
- Optional CLI tools: `fzf` for interactive session picking, `glow` for richer
  Markdown previews, `claude` or `codex` for generated session names

## Build And Test

This repo uses `just` for common tasks:

```bash
just build
just test
just check
just ci
just run list
```

Equivalent Cargo commands:

```bash
cargo build --workspace
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Usage

```bash
sp new "investigate sync"
sp quick "notes from a one-off debugging session"
sp list
sp open
sp read investigate-sync
sp edit investigate-sync
sp run investigate-sync --agent codex
```

Run `sp` with no subcommand to open the TUI.

## Contexts

Scratchpad supports two storage contexts:

- User context: global sessions under `~/scratchpad` by default.
- Project context: sessions under a project-local `.scratchpad/` directory.

Use `sp init` inside a repository to create project-local storage. The CLI
prefers project context when a `.scratchpad/` directory exists in the current
directory or an ancestor. Use `--user` or `--project` to force a context.

## Configuration

User config lives at `~/.config/scratchpad/config.toml`.

```bash
sp config init
sp config show
sp config edit
```

Important fields:

- `workspace_path`: user-context session directory.
- `default_agent`: `claude` or `codex`.
- `editor` / `viewer`: commands for opening notes.
- `name_generator`: `auto`, `claude`, `codex`, or `static`.
- `server`: optional sync server URL and token for future sync support.

## Sync Server

Start the development server with:

```bash
just serve
```

Defaults are intentionally local:

- `HOST=127.0.0.1`
- `PORT=3000`
- `DATABASE_PATH=scratchpad-server.db`

Set `SERVER_TOKEN` to require either `Authorization: Bearer <token>` or
`x-scratchpad-token: <token>` on API and WebSocket requests. CORS is disabled by
default; use `CORS_ORIGIN=https://example.com` for a single allowed origin or
`CORS_ALLOW_ANY=true` for explicit open CORS during local development.

The `sp sync` client command is not implemented yet.

## Install From Source

```bash
cargo install --locked --path scratchpad
cargo install --locked --path server
```
