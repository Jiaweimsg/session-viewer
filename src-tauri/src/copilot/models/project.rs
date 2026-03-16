use serde::{Deserialize, Serialize};

/// A GitHub Copilot CLI project (sessions grouped by working directory)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotProject {
    /// Working directory path — used as the project key
    pub cwd: String,
    /// Short display name (last path component)
    pub short_name: String,
    /// Number of sessions in this project
    pub session_count: usize,
    /// RFC3339 most recent session update time
    pub last_modified: Option<String>,
}
