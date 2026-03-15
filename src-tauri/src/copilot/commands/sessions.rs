use rayon::prelude::*;

use crate::copilot::models::session::CopilotSessionEntry;
use crate::copilot::parser::session_parser::scan_session_metadata;
use crate::copilot::parser::session_scanner::{get_workspace_storage_dir, scan_session_files};

pub fn get_sessions(workspace_hash: String) -> Result<Vec<CopilotSessionEntry>, String> {
    let storage_dir = get_workspace_storage_dir()
        .ok_or_else(|| "Could not determine VS Code workspace storage directory".to_string())?;

    let session_files = scan_session_files(&storage_dir, &workspace_hash);

    // Parallel lightweight metadata scan — no response block parsing
    let mut entries: Vec<CopilotSessionEntry> = session_files
        .par_iter()
        .map(|path| {
            let file_path = path.to_string_lossy().to_string();

            let file_meta = std::fs::metadata(path).ok();
            let modified = file_meta.as_ref().and_then(|m| {
                m.modified().ok().map(|t| {
                    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
            });

            match scan_session_metadata(path) {
                Ok(meta) => {
                    let created = if meta.created_ms > 0 {
                        chrono::DateTime::from_timestamp((meta.created_ms / 1000) as i64, 0)
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

                    CopilotSessionEntry {
                        session_id: meta.session_id,
                        workspace_hash: workspace_hash.clone(),
                        file_path,
                        title: meta.title,
                        first_prompt: meta.first_prompt.filter(|s| !s.is_empty()),
                        message_count: meta.message_count,
                        created,
                        modified,
                        model_id: meta.model_id,
                    }
                }
                Err(_) => {
                    let session_id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    CopilotSessionEntry {
                        session_id,
                        workspace_hash: workspace_hash.clone(),
                        file_path,
                        title: None,
                        first_prompt: None,
                        message_count: 0,
                        created: None,
                        modified,
                        model_id: None,
                    }
                }
            }
        })
        .collect();

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(entries)
}
