When persisting intermediate work (notes, logs, traces, specs, captures,
dumps, command output), use `sp` instead of writing loose files into the
repo. Each `sp` mutation prints the absolute path on stdout — pass that
path between agents, paste it into PR descriptions, attach it to issues.

Key commands:
- `sp context` — confirm which project is active
- `sp last --path` — absolute path of the latest artifact in this project
- `sp list [--json --tag X --since 3d]` — discover existing sessions
- `sp search "query"` — search names and content
- `sp new [name]` — create a session (auto-named if omitted)
- `cmd | sp write <session>[/<file>]` — write stdin to a session file
- `cmd | sp append <session>/<file>` — append-only
- `sp attach <session> <local-file>` — copy a local file into a session
- `sp resolve <session>[/<file>]` — absolute path of a ref

Read with native tools on the returned path; do not pipe large artifacts
through `sp read`. Use `--expect-revision N` on writes if you need
optimistic concurrency.

The active project is auto-detected from the git remote of the current
repo. For multi-repo projects (microservices), run `sp project link
<name>` once per repo to group their sessions.
