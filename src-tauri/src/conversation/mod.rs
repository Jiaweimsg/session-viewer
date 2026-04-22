//! Conversation collection module.
//!
//! Scans Claude Code JSONL sessions, extracts user prompts (filtering
//! CLI-injected system messages), tags each with first/followup/retry,
//! and uploads in <=10MB batches to the server's /api/conversations endpoint.
//!
//! State (per-file byte offsets) is persisted to disk so scans are incremental
//! and resumable after a crash or partial upload.

pub mod scanner;
pub mod state;
pub mod uploader;

use serde::{Deserialize, Serialize};

/// A single user prompt collected from a Claude Code session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMessage {
    pub uuid: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_uuid: Option<String>,
    pub timestamp: String,
    pub project: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub role_tag: RoleTag,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoleTag {
    First,
    Followup,
    Retry,
}

pub const MAX_BATCH_BYTES: usize = 10 * 1024 * 1024; // 10MB
