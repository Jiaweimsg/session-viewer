use std::collections::HashMap;
use crate::opencode::models::project::{ProjectIndexEntry, ProjectMetadata};
use crate::opencode::parser::json_parser::parse_project;
use crate::opencode::parser::session_scanner::{
    get_project_dir, scan_project_hashes, scan_session_files, short_name_from_path,
};

pub fn get_projects() -> Result<Vec<ProjectIndexEntry>, String> {
    let project_dir = get_project_dir()
        .ok_or_else(|| "Could not find OpenCode storage directory".to_string())?;

    if !project_dir.exists() {
        return Ok(vec![]);
    }

    let project_hashes = scan_project_hashes();
    let mut projects = Vec::new();

    for hash in project_hashes {
        let project_file = project_dir.join(format!("{}.json", hash));

        if let Ok(project_meta) = parse_project(&project_file) {
            let session_files = scan_session_files(&hash);
            let session_count = session_files.len();

            // Find last modified session
            let last_modified = find_last_modified_session(&session_files);

            projects.push(ProjectIndexEntry {
                id: project_meta.id.clone(),
                worktree: project_meta.worktree.clone(),
                short_name: short_name_from_path(&project_meta.worktree),
                session_count,
                last_modified,
            });
        }
    }

    // Sort by last modified, most recent first
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    Ok(projects)
}

fn find_last_modified_session(session_files: &[std::path::PathBuf]) -> Option<String> {
    session_files
        .iter()
        .filter_map(|path| {
            std::fs::metadata(path).ok().and_then(|meta| {
                meta.modified().ok().map(|t| {
                    let d = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
            })
        })
        .max()
}
