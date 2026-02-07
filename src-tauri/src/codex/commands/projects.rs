use std::collections::HashMap;

use crate::codex::commands::sessions::list_all_sessions;
use crate::codex::models::project::ProjectEntry;

pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    let sessions = list_all_sessions()?;

    let mut project_map: HashMap<String, ProjectEntry> = HashMap::new();

    for session in sessions {
        if session.cwd.is_empty() {
            continue;
        }

        let entry = project_map
            .entry(session.cwd.clone())
            .or_insert_with(|| ProjectEntry {
                cwd: session.cwd.clone(),
                short_name: session.short_name.clone(),
                session_count: 0,
                last_modified: None,
                model_provider: session.model_provider.clone(),
            });

        entry.session_count += 1;

        // Track latest modification time
        if let Some(ref modified) = session.modified {
            if entry
                .last_modified
                .as_ref()
                .map(|m| modified > m)
                .unwrap_or(true)
            {
                entry.last_modified = Some(modified.clone());
            }
        }
    }

    let mut projects: Vec<ProjectEntry> = project_map.into_values().collect();
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    Ok(projects)
}
