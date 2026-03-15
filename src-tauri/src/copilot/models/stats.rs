use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Token and usage statistics for GitHub Copilot sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotTokenSummary {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub tokens_by_model: HashMap<String, u64>,
    pub daily_tokens: Vec<CopilotDailyTokenEntry>,
    pub session_count: usize,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotDailyTokenEntry {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
