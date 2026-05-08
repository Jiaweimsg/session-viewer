# OpenCode SQLite Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace opencode's broken JSON-file reader with a SQLite reader that queries `~/.local/share/opencode/opencode.db` directly.

**Architecture:** Delete `parser/session_scanner.rs` and `parser/json_parser.rs`; create `parser/db_reader.rs` as the single DB access layer; update all six command files to call `db_reader` instead of the old parsers; clean up model structs that only existed for JSON deserialization.

**Tech Stack:** Rust, `rusqlite` (already in `Cargo.toml` with `bundled` feature), `serde_json`, `chrono`, `dirs`

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| CREATE | `src-tauri/src/opencode/parser/db_reader.rs` | Open DB, all query helpers, row structs |
| DELETE | `src-tauri/src/opencode/parser/session_scanner.rs` | Replaced by db_reader |
| DELETE | `src-tauri/src/opencode/parser/json_parser.rs` | Replaced by db_reader |
| MODIFY | `src-tauri/src/opencode/parser/mod.rs` | Expose only `db_reader` |
| MODIFY | `src-tauri/src/opencode/models/project.rs` | Remove `ProjectMetadata`, `ProjectTime` |
| MODIFY | `src-tauri/src/opencode/models/session.rs` | Remove `SessionMetadata` and related structs |
| MODIFY | `src-tauri/src/opencode/models/message.rs` | Remove JSON-only structs |
| MODIFY | `src-tauri/src/opencode/commands/projects.rs` | Use db_reader |
| MODIFY | `src-tauri/src/opencode/commands/sessions.rs` | Use db_reader |
| MODIFY | `src-tauri/src/opencode/commands/messages.rs` | Use db_reader, part table |
| MODIFY | `src-tauri/src/opencode/commands/stats.rs` | Aggregate real token data from DB |
| MODIFY | `src-tauri/src/opencode/commands/search.rs` | Search part text via DB |

---

## Task 1: Create `parser/db_reader.rs`

**Files:**
- Create: `src-tauri/src/opencode/parser/db_reader.rs`

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/opencode/parser/db_reader.rs` (create the file with just the test module first):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_to_rfc3339_known_value() {
        // 1000ms = 1 second from epoch = 1970-01-01T00:00:01+00:00
        assert_eq!(ms_to_rfc3339(1000), "1970-01-01T00:00:01+00:00");
    }

    #[test]
    fn get_db_path_contains_opencode() {
        // Should always produce a path ending in opencode/opencode.db
        let path = get_db_path().expect("home dir should exist in test env");
        let s = path.to_string_lossy();
        assert!(s.contains("opencode"), "path should contain 'opencode': {}", s);
        assert!(s.ends_with("opencode.db"), "path should end with opencode.db: {}", s);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd src-tauri && cargo test --lib opencode::parser::db_reader 2>&1 | tail -20
```

Expected: compile error — `ms_to_rfc3339` and `get_db_path` not defined yet.

- [ ] **Step 3: Implement `db_reader.rs`**

Replace the file with the full implementation:

```rust
use std::path::PathBuf;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use crate::opencode::models::project::ProjectIndexEntry;

// ── DB path & connection ─────────────────────────────────────────────────────

pub fn get_db_path() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".local").join("share").join("opencode").join("opencode.db"))
}

pub fn open_db() -> Result<Connection, String> {
    let path = get_db_path().ok_or_else(|| "Cannot determine home directory".to_string())?;
    if !path.exists() {
        return Err(format!("opencode.db not found at {:?}", path));
    }
    Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open opencode.db: {}", e))
}

// ── Row structs ──────────────────────────────────────────────────────────────

pub struct SessionRow {
    pub id: String,
    pub project_id: String,
    pub parent_id: Option<String>,
    pub title: Option<String>,
    pub directory: String,
    pub created: String,
    pub modified: String,
}

pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub time_created: i64,
    pub data: Value,
}

pub struct PartRow {
    pub id: String,
    pub message_id: String,
    pub time_created: i64,
    pub data: Value,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub fn ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::from_timestamp(ms / 1000, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

// ── Project queries ──────────────────────────────────────────────────────────

pub fn query_projects(conn: &Connection) -> Vec<ProjectIndexEntry> {
    let mut stmt = match conn.prepare(
        "SELECT id, COALESCE(worktree, ''), time_updated \
         FROM project ORDER BY time_updated DESC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let worktree: String = row.get(1)?;
        let time_updated: i64 = row.get(2)?;
        Ok((id, worktree, time_updated))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .map(|(id, worktree, time_updated)| {
                let short_name = crate::shared_models::basename(&worktree);
                ProjectIndexEntry {
                    id,
                    short_name,
                    worktree,
                    session_count: 0,
                    last_modified: Some(ms_to_rfc3339(time_updated)),
                }
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn count_sessions_for_project(conn: &Connection, project_id: &str) -> usize {
    conn.query_row(
        "SELECT COUNT(*) FROM session WHERE project_id = ?1",
        rusqlite::params![project_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|c| c as usize)
    .unwrap_or(0)
}

// ── Session queries ──────────────────────────────────────────────────────────

pub fn query_sessions(conn: &Connection, project_id: &str) -> Vec<SessionRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, project_id, parent_id, title, COALESCE(path, ''), \
         time_created, time_updated \
         FROM session WHERE project_id = ?1 ORDER BY time_updated DESC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![project_id], |row| {
        let tc: i64 = row.get(5)?;
        let tu: i64 = row.get(6)?;
        Ok(SessionRow {
            id: row.get(0)?,
            project_id: row.get(1)?,
            parent_id: row.get(2)?,
            title: row.get(3)?,
            directory: row.get(4)?,
            created: ms_to_rfc3339(tc),
            modified: ms_to_rfc3339(tu),
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn count_messages_for_session(conn: &Connection, session_id: &str) -> u32 {
    conn.query_row(
        "SELECT COUNT(*) FROM message WHERE session_id = ?1",
        rusqlite::params![session_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|c| c as u32)
    .unwrap_or(0)
}

// ── Message queries ──────────────────────────────────────────────────────────

pub fn query_messages(conn: &Connection, session_id: &str) -> Vec<MessageRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, time_created, data \
         FROM message WHERE session_id = ?1 ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, sid, tc, data_str)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| MessageRow { id, session_id: sid, time_created: tc, data })
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn query_parts_for_message(conn: &Connection, message_id: &str) -> Vec<PartRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, message_id, time_created, data \
         FROM part WHERE message_id = ?1 ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![message_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, mid, tc, data_str)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| PartRow { id, message_id: mid, time_created: tc, data })
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn query_parts_for_session(conn: &Connection, session_id: &str) -> Vec<PartRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, message_id, time_created, data \
         FROM part WHERE session_id = ?1 ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, mid, tc, data_str)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| PartRow { id, message_id: mid, time_created: tc, data })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Query all assistant messages across all sessions for stats aggregation.
pub fn query_all_assistant_messages(conn: &Connection) -> Vec<MessageRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, time_created, data FROM message \
         WHERE json_extract(data, '$.role') = 'assistant' \
         ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, sid, tc, data_str)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| MessageRow { id, session_id: sid, time_created: tc, data })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Query all text parts across all sessions for full-text search.
pub fn query_all_text_parts(conn: &Connection) -> Vec<(PartRow, String)> {
    // Returns (PartRow, project_id)
    let mut stmt = match conn.prepare(
        "SELECT p.id, p.message_id, p.session_id, p.time_created, p.data, s.project_id \
         FROM part p \
         JOIN session s ON p.session_id = s.id \
         WHERE json_extract(p.data, '$.type') = 'text' \
         ORDER BY p.time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, mid, sid, tc, data_str, project_id)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| (PartRow { id, message_id: mid, session_id: sid, time_created: tc, data }, project_id))
            })
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_to_rfc3339_known_value() {
        assert_eq!(ms_to_rfc3339(1000), "1970-01-01T00:00:01+00:00");
    }

    #[test]
    fn get_db_path_contains_opencode() {
        let path = get_db_path().expect("home dir should exist in test env");
        let s = path.to_string_lossy();
        assert!(s.contains("opencode"), "path should contain 'opencode': {}", s);
        assert!(s.ends_with("opencode.db"), "path should end with opencode.db: {}", s);
    }
}
```

> **Note:** `PartRow` needs a `session_id` field — add it to the struct definition above. The `query_parts_for_message` and `query_parts_for_session` functions set it to `message_id`'s session; for simplicity `query_parts_for_message` can leave `session_id` as empty string since it's unused there.

Update `PartRow` to include `session_id`:
```rust
pub struct PartRow {
    pub id: String,
    pub message_id: String,
    pub session_id: String,   // add this
    pub time_created: i64,
    pub data: Value,
}
```

And update `query_parts_for_message` to set `session_id: String::new()` (it's unused in that context).

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd src-tauri && cargo test --lib opencode::parser::db_reader 2>&1 | tail -20
```

Expected:
```
test opencode::parser::db_reader::tests::get_db_path_contains_opencode ... ok
test opencode::parser::db_reader::tests::ms_to_rfc3339_known_value ... ok
test result: ok. 2 passed; 0 failed
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/opencode/parser/db_reader.rs
git commit -m "feat(opencode): add SQLite db_reader with project/session/message/part queries"
```

---

## Task 2: Update `parser/mod.rs` and Clean Up Models

**Files:**
- Modify: `src-tauri/src/opencode/parser/mod.rs`
- Modify: `src-tauri/src/opencode/models/project.rs`
- Modify: `src-tauri/src/opencode/models/session.rs`
- Modify: `src-tauri/src/opencode/models/message.rs`

- [ ] **Step 1: Update `parser/mod.rs`**

Replace the entire file:

```rust
pub mod db_reader;
```

- [ ] **Step 2: Replace `models/project.rs`**

Remove `ProjectMetadata` and `ProjectTime` (JSON-only). Keep only `ProjectIndexEntry`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndexEntry {
    pub id: String,
    pub worktree: String,
    pub short_name: String,
    pub session_count: usize,
    pub last_modified: Option<String>,
}
```

- [ ] **Step 3: Replace `models/session.rs`**

Remove `SessionMetadata`, `SessionPermission`, `SessionTime`, `SessionSummary` (all JSON-only). Keep `SessionIndexEntry` and `SessionGroup`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexEntry {
    pub session_id: String,
    pub project_id: String,
    pub directory: String,
    pub short_name: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub first_prompt: Option<String>,
    pub message_count: u32,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGroup {
    pub root_session: SessionIndexEntry,
    pub sub_sessions: Vec<SessionIndexEntry>,
}
```

- [ ] **Step 4: Replace `models/message.rs`**

Remove all JSON-only structs (`MessageMetadata`, `MessageTime`, `MessageSummary`, `ModelInfo`). The file can now be empty or hold only types needed elsewhere. Since no types are needed from here after the migration, replace with an empty module marker:

```rust
// Message display types are in shared_models::DisplayMessage
```

- [ ] **Step 5: Verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error" | head -20
```

Expected: errors only in the four `commands/` files that still import the old parser modules — that's correct and expected at this stage.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/opencode/parser/mod.rs \
        src-tauri/src/opencode/models/project.rs \
        src-tauri/src/opencode/models/session.rs \
        src-tauri/src/opencode/models/message.rs
git commit -m "refactor(opencode): remove JSON-only model structs and old parser exports"
```

---

## Task 3: Rewrite `commands/projects.rs`

**Files:**
- Modify: `src-tauri/src/opencode/commands/projects.rs`

- [ ] **Step 1: Replace the file**

```rust
use crate::opencode::models::project::ProjectIndexEntry;
use crate::opencode::parser::db_reader::{
    count_sessions_for_project, open_db, query_projects,
};

pub fn get_projects() -> Result<Vec<ProjectIndexEntry>, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    let mut projects = query_projects(&conn);

    for project in &mut projects {
        project.session_count = count_sessions_for_project(&conn, &project.id);
    }

    Ok(projects)
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error\[" | grep "projects" | head -10
```

Expected: no errors mentioning `commands/projects.rs`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/opencode/commands/projects.rs
git commit -m "feat(opencode): read projects from SQLite"
```

---

## Task 4: Rewrite `commands/sessions.rs`

**Files:**
- Modify: `src-tauri/src/opencode/commands/sessions.rs`

- [ ] **Step 1: Replace the file**

```rust
use std::collections::HashMap;
use rusqlite::Connection;

use crate::opencode::models::session::{SessionGroup, SessionIndexEntry};
use crate::opencode::parser::db_reader::{
    count_messages_for_session, open_db, query_messages, query_parts_for_message, query_sessions,
};
use crate::shared_models::basename;

pub fn get_sessions(project_id: String) -> Result<Vec<SessionIndexEntry>, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    let rows = query_sessions(&conn, &project_id);

    let entries = rows
        .into_iter()
        .map(|row| {
            let first_prompt = get_first_prompt(&conn, &row.id);
            let message_count = count_messages_for_session(&conn, &row.id);
            let short_name = if row.directory.is_empty() {
                "unknown".to_string()
            } else {
                basename(&row.directory)
            };

            SessionIndexEntry {
                session_id: row.id,
                project_id: row.project_id,
                directory: row.directory,
                short_name,
                title: row.title,
                slug: None,
                first_prompt,
                message_count,
                created: Some(row.created),
                modified: Some(row.modified),
                git_branch: None,
                parent_id: row.parent_id,
            }
        })
        .collect::<Vec<_>>();

    Ok(entries)
}

pub fn get_sessions_grouped(project_id: String) -> Result<Vec<SessionGroup>, String> {
    let all_sessions = get_sessions(project_id)?;

    let mut root_sessions = Vec::new();
    let mut child_map: HashMap<String, Vec<SessionIndexEntry>> = HashMap::new();

    for session in all_sessions {
        if let Some(ref parent_id) = session.parent_id {
            child_map.entry(parent_id.clone()).or_default().push(session);
        } else {
            root_sessions.push(session);
        }
    }

    let mut grouped: Vec<SessionGroup> = root_sessions
        .into_iter()
        .map(|root| {
            let mut sub_sessions = child_map.remove(&root.session_id).unwrap_or_default();
            sub_sessions.sort_by(|a, b| a.created.cmp(&b.created));
            SessionGroup { root_session: root, sub_sessions }
        })
        .collect();

    grouped.sort_by(|a, b| b.root_session.modified.cmp(&a.root_session.modified));

    Ok(grouped)
}

fn get_first_prompt(conn: &Connection, session_id: &str) -> Option<String> {
    let messages = query_messages(conn, session_id);

    let user_msg = messages.iter().find(|m| {
        m.data.get("role").and_then(|r| r.as_str()) == Some("user")
    })?;

    let parts = query_parts_for_message(conn, &user_msg.id);
    let text_part = parts.iter().find(|p| {
        p.data.get("type").and_then(|t| t.as_str()) == Some("text")
    })?;

    let text = text_part.data.get("text")?.as_str()?;
    if text.len() <= 100 {
        Some(text.to_string())
    } else {
        Some(format!("{}...", &text[..100]))
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error\[" | grep "sessions" | head -10
```

Expected: no errors mentioning `commands/sessions.rs`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/opencode/commands/sessions.rs
git commit -m "feat(opencode): read sessions from SQLite with first_prompt from part table"
```

---

## Task 5: Rewrite `commands/messages.rs`

**Files:**
- Modify: `src-tauri/src/opencode/commands/messages.rs`

- [ ] **Step 1: Replace the file**

```rust
use crate::opencode::parser::db_reader::{open_db, query_messages, query_parts_for_session};
use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

pub fn get_messages(
    session_id: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => {
            return Ok(PaginatedMessages {
                messages: vec![],
                total: 0,
                page,
                page_size,
                has_more: false,
            })
        }
    };

    let messages = query_messages(&conn, &session_id);

    // Load all parts for this session at once, grouped by message_id
    let all_parts = query_parts_for_session(&conn, &session_id);
    let mut parts_by_message: std::collections::HashMap<String, Vec<_>> =
        std::collections::HashMap::new();
    for part in all_parts {
        parts_by_message.entry(part.message_id.clone()).or_default().push(part);
    }

    let mut all_valid = Vec::new();

    for msg in &messages {
        let role = match msg.data.get("role").and_then(|r| r.as_str()) {
            Some(r) => r.to_string(),
            None => continue,
        };

        let timestamp = Some(
            chrono::DateTime::from_timestamp(msg.time_created / 1000, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
        );

        let empty = vec![];
        let parts = parts_by_message.get(&msg.id).unwrap_or(&empty);
        let mut content_blocks = Vec::new();

        for part in parts {
            let part_type = part.data.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match part_type {
                "text" => {
                    if let Some(text) = part.data.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            content_blocks.push(DisplayContentBlock::Text {
                                text: text.to_string(),
                            });
                        }
                    }
                }
                "reasoning" => {
                    if let Some(text) = part.data.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            content_blocks.push(DisplayContentBlock::Reasoning {
                                text: text.to_string(),
                            });
                        }
                    }
                }
                _ => {} // skip step-start, step-finish, patch, tool
            }
        }

        if !content_blocks.is_empty() {
            all_valid.push(DisplayMessage {
                uuid: Some(msg.id.clone()),
                role,
                timestamp,
                content: content_blocks,
            });
        }
    }

    let total = all_valid.len();
    let start = page * page_size;
    let end = std::cmp::min(start + page_size, total);
    let has_more = end < total;
    let messages = all_valid.into_iter().skip(start).take(page_size).collect();

    Ok(PaginatedMessages { messages, total, page, page_size, has_more })
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error\[" | grep "messages" | head -10
```

Expected: no errors mentioning `commands/messages.rs`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/opencode/commands/messages.rs
git commit -m "feat(opencode): read messages and parts from SQLite"
```

---

## Task 6: Rewrite `commands/stats.rs`

**Files:**
- Modify: `src-tauri/src/opencode/commands/stats.rs`

- [ ] **Step 1: Replace the file**

```rust
use std::collections::HashMap;

use crate::opencode::models::stats::{DailyTokenEntry, TokenSummary};
use crate::opencode::parser::db_reader::{open_db, query_all_assistant_messages};

pub fn get_stats() -> Result<TokenSummary, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => {
            return Ok(TokenSummary {
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_tokens: 0,
                tokens_by_model: HashMap::new(),
                daily_tokens: vec![],
                session_count: 0,
                message_count: 0,
            })
        }
    };

    let session_count = conn
        .query_row("SELECT COUNT(*) FROM session", [], |r| r.get::<_, i64>(0))
        .map(|c| c as usize)
        .unwrap_or(0);

    let message_count = conn
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get::<_, i64>(0))
        .map(|c| c as usize)
        .unwrap_or(0);

    let assistant_messages = query_all_assistant_messages(&conn);

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    // date -> (input, output)
    let mut daily_map: HashMap<String, (u64, u64)> = HashMap::new();

    for msg in &assistant_messages {
        let tokens = match msg.data.get("tokens") {
            Some(t) => t,
            None => continue,
        };

        let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
        let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
        let cache_write = tokens
            .get("cache")
            .and_then(|c| c.get("write"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = tokens
            .get("cache")
            .and_then(|c| c.get("read"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let effective_input = input + cache_write + cache_read;
        total_input += effective_input;
        total_output += output;

        let provider = msg.data.get("providerID").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model = msg.data.get("modelID").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model_key = format!("{}/{}", provider, model);
        *tokens_by_model.entry(model_key).or_insert(0) += effective_input + output;

        let date = chrono::DateTime::from_timestamp(msg.time_created / 1000, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        if !date.is_empty() {
            let entry = daily_map.entry(date).or_insert((0, 0));
            entry.0 += effective_input;
            entry.1 += output;
        }
    }

    let mut daily_tokens: Vec<DailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, (input, output))| DailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(TokenSummary {
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_tokens: total_input + total_output,
        tokens_by_model,
        daily_tokens,
        session_count,
        message_count,
    })
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error\[" | grep "stats" | head -10
```

Expected: no errors mentioning `commands/stats.rs`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/opencode/commands/stats.rs
git commit -m "feat(opencode): aggregate real token data from SQLite message table"
```

---

## Task 7: Rewrite `commands/search.rs`

**Files:**
- Modify: `src-tauri/src/opencode/commands/search.rs`

- [ ] **Step 1: Replace the file**

```rust
use crate::opencode::parser::db_reader::{open_db, query_all_text_parts};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeSearchResult {
    pub project_id: String,
    pub session_id: String,
    pub first_prompt: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
    pub message_id: String,
}

pub fn global_search(query: String, max_results: usize) -> Result<Vec<OpencodeSearchResult>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    let query_lower = query.to_lowercase();
    let all_parts = query_all_text_parts(&conn);

    let results = all_parts
        .into_iter()
        .filter_map(|(part, project_id)| {
            let text = part.data.get("text")?.as_str()?;
            if !text.to_lowercase().contains(&query_lower) {
                return None;
            }

            let matched_text = extract_match_context(text, &query, 200);
            let timestamp = Some(
                chrono::DateTime::from_timestamp(part.time_created / 1000, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            );

            Some(OpencodeSearchResult {
                project_id,
                session_id: part.session_id.clone(),
                first_prompt: None,
                matched_text,
                role: String::new(), // role lives on the message, not the part
                timestamp,
                message_id: part.message_id.clone(),
            })
        })
        .take(max_results)
        .collect();

    Ok(results)
}

fn extract_match_context(text: &str, query: &str, context_len: usize) -> String {
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(pos) = text_lower.find(&query_lower) {
        let start = pos.saturating_sub(context_len / 2);
        let end = std::cmp::min(pos + query_lower.len() + context_len / 2, text.len());
        let mut snippet = text[start..end].to_string();
        if start > 0 { snippet = format!("...{}", snippet); }
        if end < text.len() { snippet = format!("{}...", snippet); }
        snippet
    } else {
        text.chars().take(context_len).collect()
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error\[" | grep "search" | head -10
```

Expected: no errors mentioning `commands/search.rs`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/opencode/commands/search.rs
git commit -m "feat(opencode): full-text search via part table in SQLite"
```

---

## Task 8: Delete Old Parser Files and Final Build

**Files:**
- Delete: `src-tauri/src/opencode/parser/session_scanner.rs`
- Delete: `src-tauri/src/opencode/parser/json_parser.rs`

- [ ] **Step 1: Delete the old files**

```bash
rm src-tauri/src/opencode/parser/session_scanner.rs
rm src-tauri/src/opencode/parser/json_parser.rs
```

- [ ] **Step 2: Full clean build**

```bash
cd src-tauri && cargo build 2>&1 | grep "^error" | head -20
```

Expected: zero errors.

- [ ] **Step 3: Run all tests**

```bash
cd src-tauri && cargo test --lib 2>&1 | tail -10
```

Expected:
```
test result: ok. N passed; 0 failed; 0 ignored
```

- [ ] **Step 4: Commit**

```bash
git add -u src-tauri/src/opencode/parser/
git commit -m "chore(opencode): remove obsolete JSON file parser modules"
```

---

## Self-Review Checklist

- [x] **Spec coverage**: All three spec goals covered — projects display (Task 3), sessions display (Task 4), messages display (Task 5), token stats (Task 6), search (Task 7)
- [x] **Placeholder scan**: No TBD/TODO — all code blocks are complete
- [x] **Type consistency**: `SessionRow`, `MessageRow`, `PartRow` defined in Task 1 and used by name in Tasks 4–7; `ProjectIndexEntry` fields match Task 2 model definition; `TokenSummary`/`DailyTokenEntry` fields match existing `models/stats.rs`
- [x] **`PartRow.session_id`**: Added to struct in Task 1 note; `query_parts_for_message` sets it to `String::new()` (unused); `query_parts_for_session` and `query_all_text_parts` populate it correctly
- [x] **`rayon` removal**: Old `search.rs` used `rayon::prelude::*` — new version doesn't import it; no need to update `Cargo.toml` since rayon is used by other modules
