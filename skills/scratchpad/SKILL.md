---
name: scratchpad
description: |
  This skill should be used when the user asks to "create a session",
  "write notes", "document a spec", "organize research", "set up scratchpad",
  "configure sp", "install scratchpad", or needs to create documentation,
  specs, plans, notes, research, or decisions. Also trigger on mentions of
  "scratchpad", "session", or "sp".
---

# Scratchpad (`sp`) — Session Management

Sessions are the canonical location for documentation artifacts. Each session is
a directory with markdown files, managed by the `sp` CLI.

## Core Principle

**Always use sessions for documentation artifacts.** Instead of creating loose
.md files in the project, organize them in `sp` sessions. This keeps work
discoverable and structured.

## Commands

### Discovery

```bash
sp context              # show active context (user/project) and workspace path
sp list                 # list all sessions (most recent first)
sp read <session>       # read session entry point
sp read <session> <file> # read specific file in session
sp files <session>      # show file tree for a session
```

### Creating & Writing

```bash
sp new [name]           # create session (auto-generates name if omitted)
sp write <session> [file] # write stdin to session file (default: notes.md)

# Examples:
sp new auth-refactor
echo "# Auth Spec" | sp write auth-refactor spec.md
cat design.md | sp write auth-refactor
```

### Working Inside a Session

```bash
cd "$(sp path <session>)"   # enter session directory
sp path <session>            # get directory path (for scripting)
```

## Workflow

1. Check context: `sp context` — identify where sessions will be created
2. Search first: `sp list` — avoid duplicating existing sessions
3. Create or reuse: `sp new <name>` or work with existing session
4. Write content: pipe via `sp write` or write files directly in session dir

## When to Create vs Reuse

- **New session**: distinct task, new feature, separate concern
- **Reuse existing**: continuation of prior work, updates to existing docs
- Check with `sp list` and `sp read <session>` before creating

## Contexts

- **Project** (`-p`): `.scratchpad/` in repo — for project-specific docs
- **User** (`-u`): `~/scratchpad` — for cross-project or personal notes
- Auto-detected from cwd (project context preferred if `.scratchpad/` exists)

## Environment Variables

When launched via `sp run`, the following env vars are available:

- `$SP_SESSION` — current session slug
- `$SP_CONTEXT` — "user" or "project"
- `$SP_WORKSPACE` — workspace directory path

If `$SP_SESSION` is set, the process is already inside a session — write files
directly instead of using `sp write`.

Session names support prefix matching: `sp read quant` matches `quantum-reactor`.

## Configuration

Run `sp config init` to create the config file. Key options:

| Option | Default | Description |
|--------|---------|-------------|
| `workspace_path` | `~/scratchpad` | Where user-global sessions are stored |
| `default_agent` | `claude` | Agent launched by `sp run` (`claude` or `codex`) |
| `editor` | `$EDITOR` / `vi` | Editor for `sp edit` |
| `viewer` | system default | Viewer for `sp view` |
| `name_generator` | `auto` | Name strategy: `auto`, `claude`, `codex`, `static` |

Initialize a project scratchpad with `sp init` in any repo root.

## Additional Resources

### Reference Files

- **`references/SETUP.md`** — Installation, configuration options, and project setup guide. Load when the user needs to install `sp`, configure preferences, or initialize a project scratchpad.

### Templates

- **`templates/rule.md`** — Agent rule template. Copy to the agent's rules/instructions directory so it always knows to use `sp` for documentation. See SETUP.md for per-agent locations.
