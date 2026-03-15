use serde::{Deserialize, Serialize};

/// A GitHub Copilot chat session entry for the frontend list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotSessionEntry {
    /// Session UUID
    pub session_id: String,
    /// Parent workspace hash
    pub workspace_hash: String,
    /// Absolute path to the session file
    pub file_path: String,
    /// Optional user-defined title
    pub title: Option<String>,
    /// First user message text (truncated)
    pub first_prompt: Option<String>,
    /// Number of request/response pairs
    pub message_count: u32,
    /// RFC3339 creation time
    pub created: Option<String>,
    /// RFC3339 last modified time
    pub modified: Option<String>,
    /// Last model used (e.g. "copilot/claude-opus-4.6")
    pub model_id: Option<String>,
}
