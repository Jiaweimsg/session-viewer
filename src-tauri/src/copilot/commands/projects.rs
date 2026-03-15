use crate::copilot::models::project::CopilotProjectEntry;
use crate::copilot::parser::session_scanner::{
    get_workspace_storage_dir, get_workspace_path, scan_workspace_hashes,
    scan_session_files, short_name_from_path,
};

pub fn get_projects() -> Result<Vec<CopilotProjectEntry>, String> {
    let storage_dir = get_workspace_storage_dir()
        .ok_or_else(|| "Could not determine VS Code workspace storage directory".to_string())?;

    if !storage_dir.exists() {
        return Ok(vec![]);
    }

    let hashes = scan_workspace_hashes(&storage_dir);
    let mut projects = Vec::new();

    for hash in hashes {
        let workspace_path = match get_workspace_path(&storage_dir, &hash) {
            Some(p) => p,
            None => continue,
        };

        let session_files = scan_session_files(&storage_dir, &hash);
        let session_count = session_files.len();

        if session_count == 0 {
            continue;
        }

        let last_modified = session_files
            .iter()
            .filter_map(|p| {
                std::fs::metadata(p).ok().and_then(|m| {
                    m.modified().ok().map(|t| {
                        let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                        chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    })
                })
            })
            .max();

        projects.push(CopilotProjectEntry {
            short_name: short_name_from_path(&workspace_path),
            workspace_hash: hash,
            workspace_path,
            session_count,
            last_modified,
        });
    }

    // Sort by most recently modified
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    Ok(projects)
}
