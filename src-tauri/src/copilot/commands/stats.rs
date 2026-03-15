use crate::copilot::models::stats::CopilotStats;
use crate::copilot::parser::session_scanner::{get_session_state_dir, scan_all_sessions};

pub fn get_stats() -> Result<CopilotStats, String> {
    let state_dir =
        get_session_state_dir().ok_or("Could not find ~/.copilot/session-state directory")?;
    if !state_dir.exists() {
        return Ok(CopilotStats {
            total_sessions: 0,
            total_projects: 0,
        });
    }

    let sessions = scan_all_sessions();
    let unique_cwds: std::collections::HashSet<_> = sessions.iter().map(|s| &s.cwd).collect();

    Ok(CopilotStats {
        total_sessions: sessions.len(),
        total_projects: unique_cwds.len(),
    })
}
