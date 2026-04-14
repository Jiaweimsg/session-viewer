use std::collections::HashMap;
use std::path::Path;
use serde::Serialize;
use crate::cursor::parser::project_scanner::{
    read_composer_headers, count_bubbles, epoch_ms_to_rfc3339,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorProject {
    pub cwd: String,
    pub short_name: String,
    pub session_count: usize,
    pub last_modified: Option<String>,
    pub message_count: usize,
}

pub fn get_projects() -> Result<Vec<CursorProject>, String> {
    let headers = read_composer_headers();
    if headers.is_empty() {
        return Ok(Vec::new());
    }

    // Group by workspace path
    let mut by_ws: HashMap<String, Vec<_>> = HashMap::new();
    for h in &headers {
        let ws = h.workspace_path.clone().unwrap_or_else(|| "(no workspace)".to_string());
        by_ws.entry(ws).or_default().push(h);
    }

    let mut projects: Vec<CursorProject> = by_ws
        .into_iter()
        .map(|(ws, sessions)| {
            let short_name = Path::new(&ws)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&ws)
                .to_string();

            let last_modified = sessions
                .iter()
                .filter_map(|s| s.last_updated_at.or(s.created_at))
                .max()
                .map(epoch_ms_to_rfc3339);

            let message_count: usize = sessions
                .iter()
                .map(|s| count_bubbles(&s.composer_id))
                .sum();

            CursorProject {
                cwd: ws,
                short_name,
                session_count: sessions.len(),
                last_modified,
                message_count,
            }
        })
        .collect();

    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(projects)
}
