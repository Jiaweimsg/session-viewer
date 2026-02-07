use std::fs;

use crate::claude::models::project::Project;
use crate::claude::models::session::SessionsIndex;
use crate::claude::parser::path_encoder::{decode_project_path, get_projects_dir, short_name_from_path};

pub fn get_projects() -> Result<Vec<Project>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects: Vec<Project> = Vec::new();

    let entries =
        fs::read_dir(&projects_dir).map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let encoded_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let display_path = decode_project_path(&encoded_name);
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
                let duration = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });

        if session_count > 0 {
            projects.push(Project {
                encoded_name,
                display_path,
                short_name,
                session_count,
                last_modified,
            });
        }
    }

    // Sort by last modified time, most recent first
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    Ok(projects)
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
