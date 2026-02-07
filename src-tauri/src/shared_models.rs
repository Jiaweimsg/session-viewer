use serde::Serialize;

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
