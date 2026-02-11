use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::claude::models::session::{SessionIndexEntry, SessionsIndex};
use crate::claude::parser::jsonl::{extract_first_prompt, extract_session_metadata};
use crate::claude::parser::path_encoder::get_projects_dir;

pub fn get_sessions(encoded_name: String) -> Result<Vec<SessionIndexEntry>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;
    let project_dir = projects_dir.join(&encoded_name);

    if !project_dir.exists() {
        return Err(format!("Project directory not found: {}", encoded_name));
    }

    // Collect all .jsonl files on disk: session_id -> path
    let mut disk_sessions: std::collections::HashMap<String, PathBuf> =
        std::collections::HashMap::new();
    if let Ok(dir_entries) = fs::read_dir(&project_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Some(session_id) = path.file_stem().and_then(|s| s.to_str()) {
                    if !session_id.is_empty() {
                        disk_sessions.insert(session_id.to_string(), path);
                    }
                }
            }
        }
    }

    // Try reading sessions-index.json first
    let index_path = project_dir.join("sessions-index.json");
    if index_path.exists() {
        if let Ok(content) = fs::read_to_string(&index_path) {
            if let Ok(index) = serde_json::from_str::<SessionsIndex>(&content) {
                if !index.entries.is_empty() {
                    let mut entries = index.entries;

                    // Collect indexed session IDs
                    let indexed_ids: HashSet<String> =
                        entries.iter().map(|e| e.session_id.clone()).collect();

                    // Find sessions on disk but missing from index (e.g. Ctrl+C exit)
                    for (session_id, path) in &disk_sessions {
                        if !indexed_ids.contains(session_id) {
                            if let Some(entry) = scan_single_session(path, session_id) {
                                entries.push(entry);
                            }
                        }
                    }

                    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
                    return Ok(entries);
                }
            }
        }
    }

    // Fallback: scan all JSONL files directly
    let mut entries: Vec<SessionIndexEntry> = Vec::new();
    for (session_id, path) in &disk_sessions {
        if let Some(entry) = scan_single_session(path, session_id) {
            entries.push(entry);
        }
    }

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(entries)
}

/// Scan a single .jsonl file and produce a SessionIndexEntry
fn scan_single_session(path: &Path, session_id: &str) -> Option<SessionIndexEntry> {
    let first_prompt = extract_first_prompt(path);
    let metadata = extract_session_metadata(path);
    let (_, git_branch, project_path) = metadata.unwrap_or((String::new(), None, None));

    let message_count = count_messages(path);

    let file_meta = fs::metadata(path).ok();
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

    Some(SessionIndexEntry {
        session_id: session_id.to_string(),
        full_path: Some(path.to_string_lossy().to_string()),
        file_mtime: file_meta.and_then(|m| {
            m.modified().ok().map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            })
        }),
        first_prompt,
        message_count: Some(message_count),
        created,
        modified,
        git_branch,
        project_path,
        is_sidechain: Some(false),
    })
}

fn count_messages(path: &Path) -> u32 {
    use std::io::{BufRead, BufReader};
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let mut count: u32 = 0;
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.contains("\"type\":\"user\"") || trimmed.contains("\"type\":\"assistant\"") {
            count += 1;
        }
    }
    count
}
