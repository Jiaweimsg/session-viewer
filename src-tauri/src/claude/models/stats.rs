use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The stats-cache.json file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsCache {
    pub version: Option<u32>,
    pub last_computed_date: Option<String>,
    #[serde(default)]
    pub daily_activity: Vec<DailyActivity>,
    #[serde(default)]
    pub daily_model_tokens: Vec<DailyModelTokens>,
    #[serde(default)]
    pub model_usage: HashMap<String, ModelUsageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub date: String,
    pub message_count: u64,
    pub session_count: u64,
    pub tool_call_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyModelTokens {
    pub date: String,
    pub tokens_by_model: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageEntry {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageSummary {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub tokens_by_model: HashMap<String, u64>,
    pub daily_tokens: Vec<DailyTokenEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTokenEntry {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

// ============ Advanced Stats ============

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedStats {
    pub project_token_ranking: Vec<ProjectTokenEntry>,
    pub tool_call_ranking: Vec<ToolCallEntry>,
    #[serde(default)]
    pub efficiency: SessionEfficiency,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTokenEntry {
    pub project_name: String,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallEntry {
    pub tool_name: String,
    pub call_count: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionEfficiency {
    pub avg_messages_per_session: f64,
    pub avg_tokens_per_session: f64,
    pub total_sessions: u64,
    pub total_messages: u64,
    pub distribution: Vec<EfficiencyBucket>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyBucket {
    pub label: String,
    pub count: u64,
}
