use serde::Serialize;
use std::path::PathBuf;

/// Extract the last path segment as a short name, handling both '/' and '\' separators.
/// Returns "unknown" if the path is empty or has no valid file name.
pub fn basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    PathBuf::from(trimmed)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Fallback: manual split on both separators (handles mixed paths)
            trimmed
                .rsplit(['/', '\\'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("unknown")
                .to_string()
        })
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum DisplayContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        content: String,
        #[serde(rename = "isError")]
        is_error: bool,
    },
    #[serde(rename = "reasoning")]
    Reasoning { text: String },
    #[serde(rename = "function_call")]
    FunctionCall {
        name: String,
        arguments: String,
        #[serde(rename = "callId")]
        call_id: String,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput {
        #[serde(rename = "callId")]
        call_id: String,
        output: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayMessage {
    pub uuid: Option<String>,
    pub role: String,
    pub timestamp: Option<String>,
    pub content: Vec<DisplayContentBlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedMessages {
    pub messages: Vec<DisplayMessage>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_more: bool,
}
