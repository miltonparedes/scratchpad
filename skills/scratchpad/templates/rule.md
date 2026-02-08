When creating documentation (specs, plans, notes, research, decisions),
use `sp` sessions instead of loose markdown files.

Key commands:
- `sp list` — list existing sessions (check before creating new ones)
- `sp new [name]` — create a new session
- `sp write <session> <file>` — write content via stdin pipe
- `sp read <session> [file]` — read session content
- `sp path <session>` — get session directory path
- `sp context` — show active context and workspace

Contexts: use `-p` for project (.scratchpad/), `-u` for user (~/scratchpad).
If `$SP_SESSION` is set, write files directly in the current directory.
