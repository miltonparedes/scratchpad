# Scratchpad Setup Guide

Installation, configuration, and per-agent rule placement for `sp`.

## Installation

### Quick install (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/miltonparedes/scratchpad/main/install.sh | sh
```

Drops the `sp` binary into `~/.local/bin`. Make sure `~/.local/bin` is on
`PATH`.

### From source

```bash
git clone https://github.com/miltonparedes/scratchpad.git
cd scratchpad
cargo install --path scratchpad
```

### Verify

```bash
sp --version
sp context        # shows the project that would be used here
```

## Configuration

`sp config init` writes `~/.scratchpad/config.toml`. All fields are
optional — defaults work without any config at all.

| Field | Default | Notes |
|---|---|---|
| `workspace_path` | `~/.scratchpad` | Absolute path to workspace root |
| `default_agent` | `claude` | `claude` or `codex` |
| `editor` | `$EDITOR` or `vi` | `nvim`, `code --wait`, `zed --wait` |
| `viewer` | system default | `bat --paging=always`, `glow` |
| `name_generator` | `auto` | `auto`, `claude`, `codex`, `static` |
| `hosts.short_form` | github/gitlab/bitbucket/codeberg | Hosts that emit short slug |
| `projects` | empty | Multi-repo aliases |
| `agents` | empty | Custom agent bindings for `sp run` |

### Multi-repo projects (microservices)

```toml
[[projects]]
name = "payments"
repos = ["acme/payments-api", "acme/payments-worker", "acme/payments-web"]
```

Equivalent: run `sp project link payments` inside each repo once.

### Custom agents for `sp run`

```toml
[agents.gemini]
command = "gemini"
args = []

[agents.opencode]
command = "opencode"
```

Invoke with `sp run <session> --agent gemini`.

## Per-agent rule placement

Copy `templates/rule.md` into the location each agent reads.

### Codex

Append to either the user-level or repo-level `AGENTS.md`:

```bash
cat templates/rule.md >> ~/.codex/AGENTS.md
# or
cat templates/rule.md >> ./AGENTS.md
```

### Claude Code

Append to `CLAUDE.md` at the repo root, or to a system prompt that's
loaded for every conversation. The plugin under `.claude-plugin/` already
adds a `PreToolUse` hook for the legacy "warn on loose .md" behavior.

### Cursor

Add `templates/rule.md` content to `.cursor/rules/` or to the global
rules in Cursor settings.

### Other agents

Identify where the agent loads global instructions (often `AGENTS.md`,
`INSTRUCTIONS.md`, or a settings file) and append the rule content there.
The rule is agent-agnostic.

## Project layout on disk

```
~/.scratchpad/
├── config.toml
├── projects/
│   ├── acme/
│   │   ├── api/
│   │   │   ├── auth-refactor/
│   │   │   │   ├── .sp/meta.toml
│   │   │   │   ├── notes.md
│   │   │   │   └── capture.log
│   │   │   └── perf-issue/
│   │   └── worker/
│   └── payments/                   # multi-repo alias
│       └── refund-bug/
└── shared/                         # sessions created outside a git repo
    └── random-notes/
```

`projects/<owner>/<repo>/` for short-form hosts (github, gitlab, etc.).
`projects/<host>/<owner>/<repo>/` for self-hosted servers.
`projects/<alias>/` when an alias matches (multi-repo).

## Troubleshooting

- **"No sessions found in project 'shared'"** — you're not inside a git
  repo and there are no sessions in `~/.scratchpad/shared/`. Either `cd`
  into a repo or pass `--project <name>` explicitly.
- **Wrong project detected** — run `sp context` to see why. Override with
  `git config sp.project my-name` inside the repo, or `--project my-name`
  per command.
- **Revision conflict (exit 4)** — another writer (you, the TUI, another
  agent) bumped the revision since you read. Read the latest and decide.
