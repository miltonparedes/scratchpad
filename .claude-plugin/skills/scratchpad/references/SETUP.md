# Scratchpad Setup Guide

Installation, configuration, and project setup for scratchpad (`sp`).

## Installation

### Quick install (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/miltonparedes/scratchpad/main/install.sh | sh
```

The install script detects the platform (macOS arm64/x86_64, Linux x86_64) and
places the `sp` binary in `~/.local/bin`. Ensure `~/.local/bin` is in PATH.

### Manual download

Download the binary for the target platform from
[GitHub Releases](https://github.com/miltonparedes/scratchpad/releases/latest),
extract, and move to a directory in PATH:

```bash
# Example: macOS arm64
tar xzf sp-aarch64-apple-darwin.tar.gz
mv sp ~/.local/bin/
```

### From source (requires Rust toolchain)

```bash
git clone https://github.com/miltonparedes/scratchpad.git
cd scratchpad
cargo install --path scratchpad
```

### Verify installation

```bash
sp --version
```

## Configuration

Run `sp config init` to create the config file at
`~/.config/scratchpad/config.toml`.

Each option can be customized during setup.

### Configuration Options

| Option | Description | Default | Choices |
|--------|-------------|---------|---------|
| `workspace_path` | Where user-global sessions are stored | `~/scratchpad` | Any absolute path |
| `default_agent` | Agent launched by `sp run` | `claude` | `claude`, `codex` |
| `editor` | Editor for `sp edit` / `e` key in TUI | `$EDITOR` or `vi` | e.g. `nvim`, `code --wait`, `zed --wait` |
| `viewer` | Viewer for `sp view` / `v` key in TUI | System default | e.g. `bat --paging=always`, `glow` |
| `name_generator` | How session names are generated | `auto` | `auto` (try LLM then static), `claude`, `codex`, `static` |

### Config Commands

```bash
sp config init       # create default config
sp config show       # display current config
sp config edit       # open config in editor
sp config path       # print config file path
```

### Config File Format (TOML)

```toml
config_version = 1
workspace_path = "/Users/name/scratchpad"
default_agent = "claude"
editor = "nvim"
viewer = "bat --paging=always"
name_generator = "auto"

# Optional sync server
# [server]
# url = "http://localhost:3000"
# token = "your-token"
```

## Project Setup

To scope sessions to a specific repository, initialize a project scratchpad:

```bash
cd /path/to/project
sp init
```

This creates a `.scratchpad/` directory. The command prompts whether to add it to
`.gitignore` or `.git/info/exclude`. Pass `--gitignore` or `--exclude` flags to
skip the prompt.

When `.scratchpad/` exists, `sp` automatically uses project context from that
directory. Use `-u` to force user context, `-p` to force project context.

## Agent Rule

Add a rule so Claude Code always knows to use `sp` for documentation. The rule
template is at `templates/rule.md`. Copy it to the rules directory:

- User-level: `~/.claude/rules/scratchpad.md`
- Project-level: `.claude/rules/scratchpad.md`

```bash
cp templates/rule.md ~/.claude/rules/scratchpad.md
```

Choose user-level for global availability, project-level when the plugin is
scoped to a single project.

## Plugin Installation

Install the scratchpad plugin for the full skill and write hook:

```bash
claude --plugin-dir /path/to/scratchpad
```

The plugin provides:
- **Skill** — full session management reference, loaded on demand
- **Hook** — suggests using sessions when writing loose `.md` files
  (`sp hook check-write`)

The hook requires `sp` to be in PATH.
