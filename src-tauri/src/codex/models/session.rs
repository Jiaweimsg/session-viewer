use serde::{Deserialize, Serialize};

/// A session entry for the frontend list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexEntry {
    pub session_id: String,
    pub cwd: String,
    pub short_name: String,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub cli_version: Option<String>,
    pub first_prompt: Option<String>,
    pub message_count: u32,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub file_path: String,
}
