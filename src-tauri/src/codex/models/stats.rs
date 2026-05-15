use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageSummary {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    /// Subset of `total_output_tokens` spent on internal reasoning by
    /// thinking-capable models (gpt-5/o-series). Surfaced so the UI can show
    /// "X% of output was thinking" — already counted inside output_tokens.
    pub total_reasoning_tokens: u64,
    pub total_tokens: u64,
    pub tokens_by_model: HashMap<String, u64>,
    pub daily_tokens: Vec<DailyTokenEntry>,
    pub session_count: u64,
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTokenEntry {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}
