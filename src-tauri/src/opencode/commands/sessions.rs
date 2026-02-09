use std::fs;
use std::collections::HashMap;
use crate::opencode::models::session::{SessionIndexEntry, SessionGroup};
use crate::opencode::parser::json_parser::{count_messages, extract_first_prompt, parse_session};
use crate::opencode::parser::session_scanner::{
    get_message_dir, scan_session_files, short_name_from_path,
};

/// Get sessions for a specific project
pub fn get_sessions(project_id: String) -> Result<Vec<SessionIndexEntry>, String> {
    let session_files = scan_session_files(&project_id);
    let message_dir_base = get_message_dir()
        .ok_or_else(|| "Could not find OpenCode message directory".to_string())?;

    let mut entries: Vec<SessionIndexEntry> = Vec::new();

    for session_file in session_files {
        if let Ok(session_meta) = parse_session(&session_file) {
            let session_id = session_meta.id.clone();
            let message_dir = message_dir_base.join(&session_id);

            // Extract first prompt and count messages
            let first_prompt = if message_dir.exists() {
                extract_first_prompt(&message_dir)
            } else {
                None
            };

            let message_count = if message_dir.exists() {
                count_messages(&message_dir)
            } else {
                0
            };

            // Get file timestamps
            let file_meta = fs::metadata(&session_file).ok();
            let modified = file_meta.as_ref().and_then(|m| {
                m.modified().ok().map(|t| {
                    let d = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
            });

            let created = file_meta.as_ref().and_then(|m| {
                m.created().ok().map(|t| {
                    let d = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
            });

            // Try to extract git branch from directory metadata
            // For now, we'll set it to None as it's not in the session metadata
            let git_branch = None;

            entries.push(SessionIndexEntry {
                session_id,
                project_id: session_meta.project_id.clone(),
                directory: session_meta.directory.clone(),
                short_name: short_name_from_path(&session_meta.directory),
                title: session_meta.title.clone(),
                slug: session_meta.slug.clone(),
                first_prompt,
                message_count,
                created,
                modified,
                git_branch,
                parent_id: session_meta.parent_id.clone(),  // 添加 parent_id
            });
        }
    }

    // Sort by modified time, most recent first
    entries.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(entries)
}

/// Get sessions grouped by parent-child relationship
pub fn get_sessions_grouped(project_id: String) -> Result<Vec<SessionGroup>, String> {
    let all_sessions = get_sessions(project_id)?;
    
    let mut root_sessions = Vec::new();
    let mut child_map: HashMap<String, Vec<SessionIndexEntry>> = HashMap::new();
    
    // Separate parent and child sessions
    for session in all_sessions {
        if let Some(ref parent_id) = session.parent_id {
            child_map
                .entry(parent_id.clone())
                .or_insert_with(Vec::new)
                .push(session);
        } else {
            root_sessions.push(session);
        }
    }
    
    // Build grouped structure
    let mut grouped: Vec<SessionGroup> = root_sessions
        .into_iter()
        .map(|root| {
            let mut sub_sessions = child_map
                .remove(&root.session_id)
                .unwrap_or_default();
            
            // Sort sub-sessions by created time
            sub_sessions.sort_by(|a, b| a.created.cmp(&b.created));
            
            SessionGroup {
                root_session: root,
                sub_sessions,
            }
        })
        .collect();
    
    // Sort groups by root session modified time
    grouped.sort_by(|a, b| b.root_session.modified.cmp(&a.root_session.modified));
    
    Ok(grouped)
}
