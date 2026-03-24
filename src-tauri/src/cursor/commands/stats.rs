use serde::Serialize;

#[derive(Serialize)]
pub struct CursorStats {
    pub total_sessions: usize,
    pub total_projects: usize,
}

pub fn get_stats() -> Result<CursorStats, String> {
    Ok(CursorStats {
        total_sessions: 0,
        total_projects: 0,
    })
}
