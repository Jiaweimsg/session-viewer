use std::collections::HashMap;
use std::path::Path;

use crate::copilot::models::project::CopilotProject;
use crate::copilot::parser::session_scanner::{get_session_state_dir, scan_all_sessions};

/// List all Copilot CLI projects (sessions grouped by cwd)
pub fn get_projects() -> Result<Vec<CopilotProject>, String> {
    let state_dir =
        get_session_state_dir().ok_or("Could not find ~/.copilot/session-state directory")?;
    if !state_dir.exists() {
        return Ok(Vec::new());
    }

    let sessions = scan_all_sessions();

    // Group by cwd
    let mut by_cwd: HashMap<String, Vec<_>> = HashMap::new();
    for session in sessions {
        by_cwd.entry(session.cwd.clone()).or_default().push(session);
    }

    let mut projects: Vec<CopilotProject> = by_cwd
        .into_iter()
        .map(|(cwd, sessions)| {
            let short_name = Path::new(&cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&cwd)
                .to_string();

            let last_modified = sessions
                .iter()
                .map(|s| s.updated_at.as_deref().unwrap_or(&s.created_at).to_string())
                .max();

            CopilotProject {
                cwd,
                short_name,
                session_count: sessions.len(),
                last_modified,
            }
        })
        .collect();

    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(projects)
}
