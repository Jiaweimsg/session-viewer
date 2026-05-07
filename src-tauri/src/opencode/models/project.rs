use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndexEntry {
    pub id: String,
    pub worktree: String,
    pub short_name: String,
    pub session_count: usize,
    pub last_modified: Option<String>,
}
