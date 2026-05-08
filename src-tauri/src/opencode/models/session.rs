use serde::{Deserialize, Serialize};

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
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGroup {
    pub root_session: SessionIndexEntry,
    pub sub_sessions: Vec<SessionIndexEntry>,
}
