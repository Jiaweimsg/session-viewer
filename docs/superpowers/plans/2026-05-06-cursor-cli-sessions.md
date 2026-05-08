# Cursor CLI Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full Cursor CLI session support to the existing Cursor tool.

**Architecture:** Introduce a focused Cursor CLI parser for `~/.cursor/chats/*/*/store.db`, then aggregate its normalized records into the existing Cursor commands. Keep the Tauri API and frontend routes unchanged by using prefixed CLI session keys.

**Tech Stack:** Rust, Tauri commands, rusqlite, serde_json, existing React/Zustand frontend.

---

### Task 1: Cursor CLI Parser

**Files:**
- Create: `src-tauri/src/cursor/parser/cli_chats.rs`
- Modify: `src-tauri/src/cursor/parser/mod.rs`

- [ ] Write failing unit tests for metadata decoding, user-query extraction, workspace path extraction, session key encoding, and display-message conversion.
- [ ] Run `cargo test cursor::parser::cli_chats --lib` and verify the module is missing or tests fail.
- [ ] Implement `cli_chats.rs` with pure helpers first, then filesystem/database discovery.
- [ ] Run `cargo test cursor::parser::cli_chats --lib` and verify tests pass.

### Task 2: Cursor Command Aggregation

**Files:**
- Modify: `src-tauri/src/cursor/commands/projects.rs`
- Modify: `src-tauri/src/cursor/commands/sessions.rs`
- Modify: `src-tauri/src/cursor/commands/messages.rs`
- Modify: `src-tauri/src/cursor/commands/search.rs`
- Modify: `src-tauri/src/cursor/commands/stats.rs`

- [ ] Add failing tests for merge/session-key helpers where possible without local Cursor data.
- [ ] Route `cli:{project_hash}:{session_uuid}` message requests to the CLI parser.
- [ ] Merge CLI projects with existing Cursor projects by `cwd`.
- [ ] Append CLI sessions to the relevant project session list and sort by modified time.
- [ ] Include CLI messages in Cursor search and stats.
- [ ] Run targeted Rust tests and fix compile errors.

### Task 3: Conversation Upload And Watcher

**Files:**
- Modify: `src-tauri/src/conversation/cursor_scanner.rs`
- Modify: `src-tauri/src/watcher/fs_watcher.rs`

- [ ] Add failing tests for converting CLI user prompts into `PendingMessage` rows using offsets.
- [ ] Implement CLI scanning in `conversation::cursor_scanner`.
- [ ] Add `~/.cursor/chats` to the watcher and classify changes as `cursor`.
- [ ] Run targeted Rust tests and fix failures.

### Task 4: Verification

**Files:**
- No new files expected.

- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml cursor --lib`.
- [ ] Run `npm run build`.
- [ ] Check `git status --short` and confirm only intended files changed besides the pre-existing `package.json` and `package-lock.json`.

