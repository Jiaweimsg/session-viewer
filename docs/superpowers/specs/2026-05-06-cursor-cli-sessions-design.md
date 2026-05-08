# Cursor CLI Sessions Design

## Goal

Extend the existing Cursor tool support so Cursor CLI sessions are first-class data alongside Cursor App sessions. Cursor should continue to appear as one tool in the UI, with projects, sessions, messages, search, stats, and background conversation upload including both data sources.

## Data Source

Cursor App data currently comes from `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb` and `~/.cursor/projects/*/agent-transcripts/*.jsonl`.

Cursor CLI sessions are stored under:

```text
~/.cursor/chats/{project_hash}/{session_uuid}/store.db
```

Each `store.db` has:

- `meta(key TEXT PRIMARY KEY, value TEXT)`: key `0` contains hex-encoded JSON metadata.
- `blobs(id TEXT PRIMARY KEY, data BLOB)`: many rows contain JSON messages, while other rows are binary graph/index blobs.

The metadata JSON includes `agentId`, `latestRootBlobId`, `name`, `mode`, `isRunEverything`, and `createdAt`. User prompts are JSON messages with `role: "user"` and content containing `<user_query>...</user_query>`. System-injected context such as `<user_info>` must not be treated as user prompts.

## Architecture

Add a focused Cursor CLI parser module, `src-tauri/src/cursor/parser/cli_chats.rs`, responsible for discovering CLI store databases, decoding metadata, extracting display messages, extracting real user prompts, inferring workspace paths, and producing normalized session/project records.

Keep the public Tauri command surface unchanged. Existing Cursor commands aggregate two sources internally:

- Cursor App composer/transcript records.
- Cursor CLI chat records.

Cursor CLI session keys are prefixed as `cli:{project_hash}:{session_uuid}` so they cannot collide with App composer IDs. Message loading detects this prefix and routes to the CLI parser.

## User-Facing Behavior

The sidebar still shows one `Cursor` tool. Cursor project lists include projects discovered from Cursor App and Cursor CLI. When the same project path appears in both sources, counts and modified timestamps are merged into one project card.

Cursor session lists include both App sessions and CLI sessions. CLI sessions display their first user prompt when available, otherwise the `meta.name` value. The existing message page, search page, stats page, and Resume/Open button continue to work. Cursor CLI resume/open uses the project directory, matching current Cursor behavior of opening the workspace.

## Background Upload

`conversation::cursor_scanner` gains a CLI branch that scans Cursor CLI store databases. It emits only real user prompts and filters system-injected context. Incremental progress uses `ConversationState.file_offsets` keyed by the `store.db` path and a monotonically increasing blob row position. Successful uploads advance the same offset state used by JSONL scanners.

The upload payload uses:

- `tool`: existing `cursor`
- `session_id`: the CLI session UUID
- `project` and `cwd`: inferred workspace path or stable project hash fallback
- `role_tag`: existing first/followup classification
- `model`: absent unless later CLI metadata exposes a reliable model field

## File Watching

The file watcher should include `~/.cursor/chats` when present. Relevant changes to `store.db`, `store.db-wal`, and `store.db-shm` invalidate Cursor stats and emit the existing `fs-change` event.

## Testing

Add unit tests around pure parser behavior:

- Decode hex-encoded metadata.
- Parse user messages from string and array content.
- Reject `<user_info>` and other non-query user content.
- Infer workspace path from user_info.
- Encode/decode CLI session keys.
- Build display messages from mixed user, assistant, and tool JSON rows.

Add command-level tests where feasible for aggregation helpers without depending on the developer machine's Cursor data. Run Rust tests for Cursor parser/scanner modules and the frontend build/type check.

