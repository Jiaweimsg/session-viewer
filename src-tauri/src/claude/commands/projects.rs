use std::fs;
use std::io::{BufRead, BufReader};

use crate::claude::models::project::Project;
use crate::claude::models::session::SessionsIndex;
use crate::claude::parser::path_encoder::{
    decode_project_path, get_all_projects_dirs, short_name_from_path,
};

pub fn get_projects() -> Result<Vec<Project>, String> {
    use std::collections::HashMap;

    // 跨多个 home 合并:同一 encoded_name(= 同一 cwd 项目)可能同时出现在
    // 默认 ~/.claude 和额外账号目录下,合并成一个条目、会话数累加。
    let mut merged: HashMap<String, Project> = HashMap::new();

    for projects_dir in get_all_projects_dirs() {
        if !projects_dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(&projects_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let encoded_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Try to read the real cwd from session files first (most accurate),
            // fall back to decoding the encoded directory name
            let display_path =
                read_cwd_from_sessions(&path).unwrap_or_else(|| decode_project_path(&encoded_name));
            let short_name = short_name_from_path(&display_path);

            // Use sessions-index.json if available (consistent with get_sessions)
            let session_count = {
                let index_path = path.join("sessions-index.json");
                if let Ok(content) = fs::read_to_string(&index_path) {
                    if let Ok(index) = serde_json::from_str::<SessionsIndex>(&content) {
                        if !index.entries.is_empty() {
                            index.entries.len()
                        } else {
                            count_jsonl_files(&path)
                        }
                    } else {
                        count_jsonl_files(&path)
                    }
                } else {
                    count_jsonl_files(&path)
                }
            };

            // Get last modified time from the directory
            let last_modified = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                });

            if session_count == 0 {
                continue;
            }

            match merged.get_mut(&encoded_name) {
                Some(existing) => {
                    existing.session_count += session_count;
                    if last_modified > existing.last_modified {
                        existing.last_modified = last_modified;
                    }
                }
                None => {
                    merged.insert(
                        encoded_name.clone(),
                        Project {
                            encoded_name,
                            display_path,
                            short_name,
                            session_count,
                            last_modified,
                        },
                    );
                }
            }
        }
    }

    // Sort by last modified time, most recent first
    let mut projects: Vec<Project> = merged.into_values().collect();
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    Ok(projects)
}

/// Read the real cwd path from the first available session JSONL file in a project directory.
/// This is more accurate than decoding the encoded directory name, which can't handle
/// hyphens in directory names (e.g. "claude-code-hub" gets decoded as "claude/code/hub").
fn read_cwd_from_sessions(project_dir: &std::path::Path) -> Option<String> {
    let dir_entries = fs::read_dir(project_dir).ok()?;

    for entry in dir_entries.flatten() {
        let file_path = entry.path();
        if file_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            if let Some(cwd) = extract_cwd_from_jsonl(&file_path) {
                return Some(cwd);
            }
        }
    }
    None
}

/// Extract the cwd field from the first few lines of a JSONL file
fn extract_cwd_from_jsonl(path: &std::path::Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(10) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("\"cwd\"") {
            continue;
        }

        let v: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
            if !cwd.is_empty() {
                return Some(cwd.to_string());
            }
        }
    }
    None
}

fn count_jsonl_files(dir: &std::path::Path) -> usize {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "jsonl")
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}
