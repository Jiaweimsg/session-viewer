use crate::copilot::models::session::CopilotSession;
use crate::copilot::parser::session_scanner::scan_all_sessions;

/// Search sessions by summary or first prompt
pub fn search_sessions(query: String) -> Result<Vec<CopilotSession>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let q = query.to_lowercase();
    let results = scan_all_sessions()
        .into_iter()
        .filter(|s| {
            s.summary.as_deref().unwrap_or("").to_lowercase().contains(&q)
                || s.first_prompt.as_deref().unwrap_or("").to_lowercase().contains(&q)
                || s.cwd.to_lowercase().contains(&q)
        })
        .collect();
    Ok(results)
}
