use serde::{Deserialize, Serialize};

/// A single Copilot CLI session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotSession {
    /// UUID from workspace.yaml
    pub session_id: String,
    /// Working directory
    pub cwd: String,
    /// Git root (may be absent)
    pub git_root: Option<String>,
    /// Current branch (may be absent)
    pub branch: Option<String>,
    /// AI-generated summary (may be absent)
    pub summary: Option<String>,
    /// RFC3339 creation time
    pub created_at: String,
    /// RFC3339 last update time (may equal created_at)
    pub updated_at: Option<String>,
    /// Approximate message count from events.jsonl
    pub message_count: usize,
    /// First user message (preview)
    pub first_prompt: Option<String>,
}
