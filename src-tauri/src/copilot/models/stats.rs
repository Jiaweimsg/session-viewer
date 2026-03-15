use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotStats {
    pub total_sessions: usize,
    pub total_projects: usize,
}
