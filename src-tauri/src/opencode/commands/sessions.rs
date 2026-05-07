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
