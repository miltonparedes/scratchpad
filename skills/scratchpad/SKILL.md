---
name: scratchpad
description: |
  This skill should be used when the user asks to "create a session",
  "save logs", "capture a trace", "share an artifact", "look at the last
  scratchpad", "document a spec", "organize research", "set up scratchpad",
  "configure sp", or needs to persist any intermediate artifact (notes,
  logs, dumps, specs, captures) for handoff to another agent. Also trigger
  on mentions of "scratchpad", "session", or "sp".
---

# Scratchpad (`sp`) — Project-aware artifact workspace

`sp` persists artifacts (notes, logs, traces, specs, captures) into
**sessions** scoped to the active **project**. The project is auto-detected
from the current git repo. Each write returns an absolute path, which is the
unit of exchange between agents.

## Core principles

- **Path is the contract.** Every mutation prints the absolute path of what
  was written. Pass that path to other agents or tools — do not synthesize
  paths yourself.
- **Read with your native tools.** Use the path returned by `sp` with your
  built-in Read/Grep. Do not pipe large artifacts through `sp read`.
- **Project is auto-detected.** From inside a git repo, `sp` figures out
  the project (`acme/api`, `host/owner/repo`, or `basename` fallback). No
  flags needed for normal use.
- **Sessions are folders.** Inspectable, scriptable, no opaque database.

## When to reach for `sp`

- The user says "save this for later", "share with the other agent", "look
  at the last log", "make a scratchpad for this task".
- You are about to generate a `.md`, `.log`, `.json`, or dump that would
  otherwise clutter the repo.
- You want a stable reference to attach to a Slack/PR/issue comment.
- A handoff is implied: the user names another agent or asks to continue
  work later.

## Quick reference

```bash
# Discovery
sp context                       # active project + source + workspace path
sp list                          # sessions in the active project
sp list --json                   # for programmatic use
sp list --tag bug --since 3d     # filters
sp last --path                   # absolute path of the latest artifact
sp last --in <session> --path    # latest artifact in a specific session
sp search "stripe"               # name + content search in active project
sp files <session>               # file tree of a session

# Mutation (each prints the absolute path)
sp new <name>                    # new session (auto-named if omitted)
sp new <name> --tag bug          # with a tag
echo "..." | sp write <session>[/<file>]    # default file: notes.md
echo "..." | sp append <session>/<file>     # append-only
sp attach <session> ./local.log --as logs/deploy.log
sp resolve <session>[/<file>]    # absolute path without writing anything

# State
sp tag <session> +urgent -draft
sp archive <session>
sp restore <session>
sp rename <old> <new>

# Sharing
sp link <session>[/<file>]       # absolute path (use --copy for clipboard)

# Projects (multi-repo / microservices)
sp project current               # which project is active and why
sp project list                  # all known projects (disk + aliases)
sp project link <name>           # group the current repo into a named alias
sp project save --as <name>      # persist the auto-detected project under a custom name
```

## How project detection works

`sp` resolves the active project in this order:

1. `--project <name>` flag
2. `SP_PROJECT` env var
3. `git config sp.project` (per-repo escape hatch)
4. `config.toml` alias matching any of the repo's remotes
5. `origin` remote → `owner/repo` for known hosts, `host/owner/repo` for
   self-hosted GitLabs etc.
6. Basename of the git repo root (no remote)
7. `shared` (no git)

`sp context` prints the result + which rule fired.

## Workflow patterns

### "Capture and hand off"

```bash
# Agent A: capture a log and persist it
./repro.sh 2>&1 > /tmp/capture.log
sp new perf-issue
sp attach perf-issue /tmp/capture.log --as capture.log
# stdout: /Users/.../perf-issue/capture.log  ← pass this to the next agent
```

### "Read the latest"

```bash
# User: "look at the last artifact and fix the errors"
LATEST=$(sp last --path)
# Use your native Read tool on $LATEST
```

### "Iterative notes with conflict detection"

```bash
sp new auth-refactor
echo "# Plan" | sp write auth-refactor          # rev 1
echo "# Plan v2" | sp write auth-refactor --expect-revision 1  # rev 2
# Exit 4 if revision moved underneath you
```

### "Cross-repo project (microservices)"

```bash
# From each repo, link once:
cd payments-api && sp project link payments
cd payments-worker && sp project link payments
# From now on, sessions in either repo land in
# ~/.scratchpad/projects/payments/ and list together.
```

## Output contracts

- **stdout** = the value you want to consume (path, content, JSON).
- **stderr** = human-friendly status, never script-parsed.
- **Exit codes**:
  - 0: ok
  - 1: error
  - 2: invalid usage
  - 3: session not found
  - 4: revision conflict (`--expect-revision` mismatch)
  - 5: not in a project (no sessions found, fzf invoked with empty list)

## Environment variables inside `sp run`

When a session is launched via `sp run`, these vars are set for child
processes:

- `SP_SESSION` — current session slug
- `SP_PROJECT` — active project name
- `SP_WORKSPACE` — workspace root (default `~/.scratchpad`)
- `SP_SESSION_DIR` — absolute path to the session directory

If `$SP_SESSION_DIR` is set, write files there directly instead of going
through `sp write`.

## Configuration

```bash
sp config init      # creates ~/.scratchpad/config.toml
sp config show      # current effective config
sp config edit      # open in $EDITOR
sp config path      # print path (default: ~/.scratchpad/config.toml)
```

Key sections in `config.toml`:

```toml
config_version = 2
default_agent = "claude"

[hosts]
short_form = ["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"]

[[projects]]
name = "payments"
repos = ["acme/payments-api", "acme/payments-worker"]

[agents.gemini]
command = "gemini"
args = []
```

## Additional resources

- **`references/SETUP.md`** — installation, configuration options, and
  per-agent rule placement.
- **`templates/rule.md`** — short rule snippet to copy into any agent's
  global instructions (`AGENTS.md`, Codex `~/.codex/AGENTS.md`, Cursor
  rules, etc.).
