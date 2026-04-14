use serde::Serialize;
use std::collections::HashSet;
use crate::cursor::parser::project_scanner::{read_composer_headers, count_bubbles};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorStats {
    pub total_sessions: usize,
    pub total_projects: usize,
    pub total_messages: usize,
}

pub fn get_stats() -> Result<CursorStats, String> {
    let headers = read_composer_headers();
    if headers.is_empty() {
        return Ok(CursorStats {
            total_sessions: 0,
            total_projects: 0,
            total_messages: 0,
        });
    }

    let unique_projects: HashSet<_> = headers
        .iter()
        .map(|h| h.workspace_path.as_deref().unwrap_or("(no workspace)"))
        .collect();

    let total_messages: usize = headers
        .iter()
        .map(|h| count_bubbles(&h.composer_id))
        .sum();

    Ok(CursorStats {
        total_sessions: headers.len(),
        total_projects: unique_projects.len(),
        total_messages,
    })
}
