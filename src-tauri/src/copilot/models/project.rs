use serde::{Deserialize, Serialize};

/// A GitHub Copilot workspace (project) entry for the frontend list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotProjectEntry {
    /// VS Code workspace storage hash (used as project key)
    pub workspace_hash: String,
    /// Decoded workspace path (folder or .code-workspace file path)
    pub workspace_path: String,
    /// Short display name (last path component)
    pub short_name: String,
    /// Number of chat sessions in this workspace
    pub session_count: usize,
    /// RFC3339 timestamp of most recently modified session
    pub last_modified: Option<String>,
}
