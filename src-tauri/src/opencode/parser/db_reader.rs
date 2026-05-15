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
    #[allow(dead_code)]
    pub session_id: String,
    pub time_created: i64,
    pub data: Value,
}

pub struct PartRow {
    #[allow(dead_code)]
    pub id: String,
    pub message_id: String,
    pub session_id: String,
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
        "SELECT id, project_id, parent_id, title, COALESCE(directory, ''), \
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
        "SELECT id, message_id, session_id, time_created, data \
         FROM part WHERE message_id = ?1 ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![message_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, mid, sid, tc, data_str)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| PartRow {
                        id,
                        message_id: mid,
                        session_id: sid,
                        time_created: tc,
                        data,
                    })
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn query_parts_for_session(conn: &Connection, session_id: &str) -> Vec<PartRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, message_id, session_id, time_created, data \
         FROM part WHERE session_id = ?1 ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, mid, sid, tc, data_str)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| PartRow {
                        id,
                        message_id: mid,
                        session_id: sid,
                        time_created: tc,
                        data,
                    })
            })
            .collect()
    })
    .unwrap_or_default()
}

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

/// Assistant messages joined with their project's worktree path. Used by the
/// report path so we can attribute tokens to the right project basename
/// (sessions live in `session.project_id` → `project.worktree`).
pub fn query_all_assistant_messages_with_worktree(
    conn: &Connection,
) -> Vec<(MessageRow, String)> {
    let mut stmt = match conn.prepare(
        "SELECT m.id, m.session_id, m.time_created, m.data, COALESCE(p.worktree, '') \
         FROM message m \
         JOIN session s ON m.session_id = s.id \
         LEFT JOIN project p ON s.project_id = p.id \
         WHERE json_extract(m.data, '$.role') = 'assistant' \
         ORDER BY m.time_created ASC",
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
            row.get::<_, String>(4)?,
        ))
    })
    .map(|rows| {
        rows.filter_map(|r| r.ok())
            .filter_map(|(id, sid, tc, data_str, worktree)| {
                serde_json::from_str::<Value>(&data_str)
                    .ok()
                    .map(|data| (MessageRow { id, session_id: sid, time_created: tc, data }, worktree))
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn query_all_text_parts(conn: &Connection) -> Vec<(PartRow, String)> {
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
                    .map(|data| (
                        PartRow { id, message_id: mid, session_id: sid, time_created: tc, data },
                        project_id,
                    ))
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
