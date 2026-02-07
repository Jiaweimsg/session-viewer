use std::fs;

use crate::codex::models::session::SessionIndexEntry;
use crate::codex::parser::jsonl::{count_messages, extract_first_prompt, extract_session_meta};
use crate::codex::parser::session_scanner::{scan_all_session_files, short_name_from_path};

/// Internal: scan and return all sessions (used by both get_sessions and get_projects)
pub fn list_all_sessions() -> Result<Vec<SessionIndexEntry>, String> {
    let files = scan_all_session_files();
    let mut entries: Vec<SessionIndexEntry> = Vec::new();

    for file_path in files {
        let meta = extract_session_meta(&file_path);
        let first_prompt = extract_first_prompt(&file_path);
        let message_count = count_messages(&file_path);

        let (session_id, cwd, model_provider, cli_version, git_branch) = match meta {
            Some(m) => (m.id, m.cwd, m.model_provider, m.cli_version, m.git_branch),
            None => {
                let stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                (stem, String::new(), None, None, None)
            }
        };

        let short_name = if cwd.is_empty() {
            "unknown".to_string()
        } else {
            short_name_from_path(&cwd)
        };

        let file_meta = fs::metadata(&file_path).ok();

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

        entries.push(SessionIndexEntry {
            session_id,
            cwd,
            short_name,
            model: None,
            model_provider,
            cli_version,
            first_prompt,
            message_count,
            created,
            modified,
            git_branch,
            file_path: file_path.to_string_lossy().to_string(),
        });
    }

    // Sort by modified time, most recent first
    entries.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(entries)
}

pub fn get_sessions(cwd: Option<String>) -> Result<Vec<SessionIndexEntry>, String> {
    let mut entries = list_all_sessions()?;

    if let Some(ref cwd_filter) = cwd {
        entries.retain(|e| &e.cwd == cwd_filter);
    }

    Ok(entries)
}
