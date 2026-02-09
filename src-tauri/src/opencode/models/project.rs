use serde::{Deserialize, Serialize};

/// Project metadata for OpenCode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub id: String,
    pub worktree: String,
    pub vcs: Option<String>,
    pub sandboxes: Vec<String>,
    pub time: ProjectTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTime {
    pub created: u64,
    pub updated: u64,
}

/// Project entry for the frontend list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndexEntry {
    pub id: String,
    pub worktree: String,
    pub short_name: String,
    pub session_count: usize,
    pub last_modified: Option<String>,
}
