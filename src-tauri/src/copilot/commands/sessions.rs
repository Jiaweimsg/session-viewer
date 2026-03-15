use crate::copilot::models::session::CopilotSessionEntry;
use crate::copilot::parser::session_parser::{parse_session_file, truncate};
use crate::copilot::parser::session_scanner::{get_workspace_storage_dir, scan_session_files};

pub fn get_sessions(workspace_hash: String) -> Result<Vec<CopilotSessionEntry>, String> {
    let storage_dir = get_workspace_storage_dir()
        .ok_or_else(|| "Could not determine VS Code workspace storage directory".to_string())?;

    let session_files = scan_session_files(&storage_dir, &workspace_hash);
    let mut entries = Vec::new();

    for path in session_files {
        let file_path = path.to_string_lossy().to_string();

        // Get file timestamps
        let file_meta = std::fs::metadata(&path).ok();
        let modified = file_meta.as_ref().and_then(|m| {
            m.modified().ok().map(|t| {
                let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            })
        });

        // Parse the session file for metadata (session_id, title, first_prompt, etc.)
        match parse_session_file(&path) {
            Ok(parsed) => {
                let created = if parsed.created_ms > 0 {
                    chrono::DateTime::from_timestamp((parsed.created_ms / 1000) as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                } else {
                    file_meta.as_ref().and_then(|m| {
                        m.created().ok().and_then(|t| {
                            let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                                .map(|dt| dt.to_rfc3339())
                        })
                    })
                };

                let first_prompt = parsed
                    .requests
                    .first()
                    .map(|r| truncate(&r.user_text, 150))
                    .filter(|s| !s.is_empty());

                let message_count = parsed.requests.len() as u32;

                // Use most specific model: last request's model or session-level model
                let model_id = parsed
                    .requests
                    .iter()
                    .rev()
                    .find_map(|r| r.model_id.clone())
                    .or(parsed.model_id);

                entries.push(CopilotSessionEntry {
                    session_id: parsed.session_id,
                    workspace_hash: workspace_hash.clone(),
                    file_path,
                    title: parsed.title,
                    first_prompt,
                    message_count,
                    created,
                    modified,
                    model_id,
                });
            }
            Err(_) => {
                // If we can't parse, add a minimal entry so it still shows up
                let session_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                entries.push(CopilotSessionEntry {
                    session_id,
                    workspace_hash: workspace_hash.clone(),
                    file_path,
                    title: None,
                    first_prompt: None,
                    message_count: 0,
                    created: None,
                    modified,
                    model_id: None,
                });
            }
        }
    }

    // Sort by modified time, most recent first
    entries.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(entries)
}
