---
name: scratchpad
description: |
  Use this skill when creating documentation, specs, plans, notes, research,
  or decisions — any structured content that should be organized in sessions
  rather than created as loose markdown files. Also trigger on mentions of
  "scratchpad", "session", or "sp".

  <example>
  Context: User wants to document a feature design
  user: "Create a spec for the new auth module"
  assistant: "I'll create a scratchpad session for this spec"
  </example>

  <example>
  Context: User wants to organize research notes
  user: "I need to take notes on this API"
  assistant: "Let me create a scratchpad session for your research"
  </example>
---

# Scratchpad (`sp`) — Session Management

Sessions are the canonical location for documentation artifacts. Each session is
a directory with markdown files, managed by the `sp` CLI.

## Core Principle

**Always use sessions for documentation artifacts.** Instead of creating loose
.md files in the project, use `sp` to organize them in sessions. This keeps
work discoverable and structured.

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

1. Check context: `sp context` — know where sessions will be created
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

When launched via `sp run`, these are available:
- `$SP_SESSION` — current session slug
- `$SP_CONTEXT` — "user" or "project"
- `$SP_WORKSPACE` — workspace directory path

If `$SP_SESSION` is set, you're already inside a session — write files directly.

Session names support prefix matching: `sp read quant` matches `quantum-reactor`.
