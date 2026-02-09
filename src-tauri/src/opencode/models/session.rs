use serde::{Deserialize, Serialize};

/// Session metadata from session JSON file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub id: String,
    pub slug: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "projectID")]
    pub project_id: String,
    pub directory: String,
    #[serde(rename = "parentID")]
    pub parent_id: Option<String>,
    pub title: Option<String>,
    pub permission: Option<Vec<SessionPermission>>,
    pub time: SessionTime,
    pub summary: Option<SessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPermission {
    pub permission: String,
    pub action: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTime {
    pub created: u64,
    pub updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub additions: u32,
    pub deletions: u32,
    pub files: u32,
}

/// Session entry for the frontend list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexEntry {
    pub session_id: String,
    pub project_id: String,
    pub directory: String,
    pub short_name: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub first_prompt: Option<String>,
    pub message_count: u32,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub parent_id: Option<String>,  // 添加 parent_id 字段
}

/// Grouped session with parent and children
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGroup {
    pub root_session: SessionIndexEntry,
    pub sub_sessions: Vec<SessionIndexEntry>,
}
