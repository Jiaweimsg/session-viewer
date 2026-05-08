use std::collections::HashMap;
use std::path::Path;

use crate::cursor::parser::cli_chats;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorCliProject {
    pub cwd: String,
    pub short_name: String,
    pub session_count: usize,
    pub last_modified: Option<String>,
    pub message_count: usize,
}

pub fn get_projects() -> Result<Vec<CursorCliProject>, String> {
    let mut by_cwd: HashMap<String, CursorCliProject> = HashMap::new();

    for session in cli_chats::load_all_sessions() {
        let message_count = session.message_count();
        if message_count == 0 && session.first_prompt().is_none() {
            continue;
        }
        let cwd = session.cwd();
        let modified = session.modified();

        let entry = by_cwd.entry(cwd.clone()).or_insert_with(|| CursorCliProject {
            short_name: Path::new(&cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&cwd)
                .to_string(),
            cwd,
            session_count: 0,
            last_modified: None,
            message_count: 0,
        });
        entry.session_count += 1;
        entry.message_count += message_count;
        if modified > entry.last_modified {
            entry.last_modified = modified;
        }
    }

    let mut projects: Vec<CursorCliProject> = by_cwd.into_values().collect();
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(projects)
}
