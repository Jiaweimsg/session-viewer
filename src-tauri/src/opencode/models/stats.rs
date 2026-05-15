use serde::{Deserialize, Serialize};

/// Token usage summary for OpenCode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSummary {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    /// Subset of `total_output_tokens` spent on internal reasoning when the
    /// underlying model is a thinking model (gpt-5/o-series). Already counted
    /// inside `total_output_tokens` — exposed only so the UI can render a
    /// thinking-share breakdown.
    pub total_reasoning_tokens: u64,
    pub total_tokens: u64,
    pub tokens_by_model: std::collections::HashMap<String, u64>,
    pub daily_tokens: Vec<DailyTokenEntry>,
    pub session_count: usize,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTokenEntry {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}
