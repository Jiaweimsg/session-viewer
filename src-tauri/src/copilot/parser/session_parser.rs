use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

/// A single parsed request/response pair from a Copilot session
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub request_id: String,
    /// Milliseconds since Unix epoch
    pub timestamp_ms: u64,
    /// User message text
    pub user_text: String,
    /// Assembled assistant response text blocks
    pub response_blocks: Vec<ResponseBlock>,
    /// Model identifier (e.g. "copilot/claude-opus-4.6")
    pub model_id: Option<String>,
    /// Token usage from result
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone)]
pub enum ResponseBlock {
    Text(String),
    Thinking(String),
    ToolUse { name: String, input: String },
}

/// The fully parsed state of a Copilot chat session
#[derive(Debug, Clone)]
pub struct ParsedSession {
    pub session_id: String,
    pub created_ms: u64,
    pub title: Option<String>,
    pub model_id: Option<String>,
    pub requests: Vec<ParsedRequest>,
}

// ─── JSON format types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JsonSession {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "creationDate")]
    creation_date: Option<u64>,
    #[serde(rename = "customTitle")]
    custom_title: Option<String>,
    requests: Option<Vec<JsonRequest>>,
}

#[derive(Debug, Deserialize)]
struct JsonRequest {
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    timestamp: Option<u64>,
    message: Option<JsonMessage>,
    response: Option<Vec<serde_json::Value>>,
    #[serde(rename = "modelId")]
    model_id: Option<String>,
    result: Option<JsonResult>,
}

#[derive(Debug, Deserialize)]
struct JsonMessage {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JsonResult {
    usage: Option<JsonUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

// ─── Parsing helpers ─────────────────────────────────────────────────────────

/// Parse response items from a JSON array into ResponseBlock vec
fn parse_response_items(items: &[serde_json::Value]) -> Vec<ResponseBlock> {
    let mut blocks = Vec::new();
    let mut text_acc = String::new();

    for item in items {
        let kind = item.get("kind").and_then(|k| k.as_str());

        match kind {
            Some("thinking") => {
                if !text_acc.trim().is_empty() {
                    blocks.push(ResponseBlock::Text(text_acc.trim().to_string()));
                    text_acc = String::new();
                }
                if let Some(v) = item.get("value").and_then(|v| v.as_str()) {
                    if !v.trim().is_empty() {
                        blocks.push(ResponseBlock::Thinking(v.to_string()));
                    }
                }
            }
            Some("toolInvocationSerialized") => {
                if !text_acc.trim().is_empty() {
                    blocks.push(ResponseBlock::Text(text_acc.trim().to_string()));
                    text_acc = String::new();
                }
                let name = item
                    .get("invocationMessage")
                    .and_then(|m| {
                        // invocationMessage can be a string or object with "value"
                        if let Some(s) = m.as_str() {
                            Some(s.to_string())
                        } else {
                            m.get("value").and_then(|v| v.as_str()).map(|s| s.to_string())
                        }
                    })
                    .unwrap_or_default();
                let input = item
                    .get("toolSpecificData")
                    .map(|d| serde_json::to_string(d).unwrap_or_default())
                    .unwrap_or_default();
                blocks.push(ResponseBlock::ToolUse { name, input });
            }
            Some("markdownContent") => {
                if let Some(v) = item.get("value").and_then(|v| v.as_str()) {
                    text_acc.push_str(v);
                }
            }
            // No "kind" field → plain text value object
            None => {
                if let Some(v) = item.get("value").and_then(|v| v.as_str()) {
                    text_acc.push_str(v);
                }
            }
            _ => {}
        }
    }

    if !text_acc.trim().is_empty() {
        blocks.push(ResponseBlock::Text(text_acc.trim().to_string()));
    }

    blocks
}

// ─── Parse .json (snapshot) format ───────────────────────────────────────────

pub fn parse_json_session(path: &PathBuf) -> Result<ParsedSession, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read session file: {}", e))?;
    let session: JsonSession = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse session JSON: {}", e))?;

    let session_id = session.session_id.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let created_ms = session.creation_date.unwrap_or(0);

    let mut model_id: Option<String> = None;
    let mut requests = Vec::new();

    for req in session.requests.unwrap_or_default() {
        let request_id = req.request_id.unwrap_or_default();
        let timestamp_ms = req.timestamp.unwrap_or(created_ms);
        let user_text = req
            .message
            .and_then(|m| m.text)
            .unwrap_or_default();

        let response_blocks = req
            .response
            .as_deref()
            .map(parse_response_items)
            .unwrap_or_default();

        if let Some(ref m) = req.model_id {
            model_id = Some(m.clone());
        }

        let (input_tokens, output_tokens) = req
            .result
            .and_then(|r| r.usage)
            .map(|u| (u.input_tokens.unwrap_or(0), u.output_tokens.unwrap_or(0)))
            .unwrap_or((0, 0));

        if !user_text.is_empty() || !response_blocks.is_empty() {
            requests.push(ParsedRequest {
                request_id,
                timestamp_ms,
                user_text,
                response_blocks,
                model_id: req.model_id,
                input_tokens,
                output_tokens,
            });
        }
    }

    Ok(ParsedSession {
        session_id,
        created_ms,
        title: session.custom_title,
        model_id,
        requests,
    })
}

// ─── Parse .jsonl (event log) format ─────────────────────────────────────────

pub fn parse_jsonl_session(path: &PathBuf) -> Result<ParsedSession, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read session file: {}", e))?;

    let mut session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut created_ms: u64 = 0;
    let mut title: Option<String> = None;
    let mut model_id: Option<String> = None;

    // request_idx -> (request_obj, Vec<response_items>, model_id, usage)
    let mut requests_meta: HashMap<usize, (serde_json::Value, Vec<serde_json::Value>, Option<String>, u64, u64)> = HashMap::new();
    let mut request_order: Vec<usize> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let kind = obj.get("kind").and_then(|k| k.as_i64()).unwrap_or(-1);
        let k = obj.get("k").and_then(|k| k.as_array()).cloned().unwrap_or_default();
        let v = obj.get("v");

        match kind {
            0 => {
                // Initial state
                if let Some(v) = v {
                    if let Some(id) = v.get("sessionId").and_then(|s| s.as_str()) {
                        session_id = id.to_string();
                    }
                    if let Some(cd) = v.get("creationDate").and_then(|d| d.as_u64()) {
                        created_ms = cd;
                    }
                    if let Some(ct) = v.get("customTitle").and_then(|t| t.as_str()) {
                        title = Some(ct.to_string());
                    }
                }
            }
            1 => {
                // Set field
                if k == [serde_json::json!("customTitle")] {
                    if let Some(t) = v.and_then(|v| v.as_str()) {
                        title = Some(t.to_string());
                    }
                } else if k.len() == 3
                    && k[0].as_str() == Some("inputState")
                    && k[1].as_str() == Some("selectedModel")
                {
                    // model update from selectedModel
                    if let Some(id) = v.and_then(|v| v.get("identifier")).and_then(|i| i.as_str()) {
                        model_id = Some(id.to_string());
                    }
                } else if k.len() == 3
                    && k[0].as_str() == Some("requests")
                    && k[2].as_str() == Some("result")
                {
                    if let Some(idx) = k[1].as_u64() {
                        let idx = idx as usize;
                        let usage = v
                            .and_then(|v| v.get("usage"))
                            .map(|u| {
                                (
                                    u.get("inputTokens").and_then(|t| t.as_u64()).unwrap_or(0),
                                    u.get("outputTokens").and_then(|t| t.as_u64()).unwrap_or(0),
                                )
                            })
                            .unwrap_or((0, 0));
                        if let Some(entry) = requests_meta.get_mut(&idx) {
                            entry.3 = usage.0;
                            entry.4 = usage.1;
                        }
                    }
                } else if k.len() == 3
                    && k[0].as_str() == Some("requests")
                    && k[2].as_str() == Some("modelId")
                {
                    if let Some(idx) = k[1].as_u64() {
                        let idx = idx as usize;
                        if let Some(mid) = v.and_then(|v| v.as_str()) {
                            if let Some(entry) = requests_meta.get_mut(&idx) {
                                entry.2 = Some(mid.to_string());
                            }
                        }
                    }
                }
            }
            2 => {
                // Append to array
                if k == [serde_json::json!("requests")] {
                    // New request appended
                    if let Some(arr) = v.and_then(|v| v.as_array()) {
                        for req_obj in arr {
                            let idx = request_order.len();
                            request_order.push(idx);
                            let req_model = req_obj
                                .get("modelId")
                                .and_then(|m| m.as_str())
                                .map(|s| s.to_string());
                            requests_meta.insert(idx, (req_obj.clone(), Vec::new(), req_model, 0, 0));
                        }
                    }
                } else if k.len() == 3
                    && k[0].as_str() == Some("requests")
                    && k[2].as_str() == Some("response")
                {
                    if let Some(idx) = k[1].as_u64() {
                        let idx = idx as usize;
                        if let Some(items) = v.and_then(|v| v.as_array()) {
                            if let Some(entry) = requests_meta.get_mut(&idx) {
                                entry.1.extend(items.iter().cloned());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Build ParsedRequest list in order
    let mut parsed_requests = Vec::new();
    for idx in request_order {
        if let Some((req_obj, resp_items, req_model, in_tok, out_tok)) = requests_meta.remove(&idx) {
            let request_id = req_obj
                .get("requestId")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            let timestamp_ms = req_obj
                .get("timestamp")
                .and_then(|t| t.as_u64())
                .unwrap_or(created_ms);
            let user_text = req_obj
                .get("message")
                .and_then(|m| m.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            let response_blocks = parse_response_items(&resp_items);

            // Update session-level model
            if let Some(ref m) = req_model {
                model_id = Some(m.clone());
            }

            if !user_text.is_empty() || !response_blocks.is_empty() {
                parsed_requests.push(ParsedRequest {
                    request_id,
                    timestamp_ms,
                    user_text,
                    response_blocks,
                    model_id: req_model,
                    input_tokens: in_tok,
                    output_tokens: out_tok,
                });
            }
        }
    }

    Ok(ParsedSession {
        session_id,
        created_ms,
        title,
        model_id,
        requests: parsed_requests,
    })
}

/// Parse any session file (auto-detect format by extension)
pub fn parse_session_file(path: &PathBuf) -> Result<ParsedSession, String> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => parse_json_session(path),
        Some("jsonl") => parse_jsonl_session(path),
        _ => Err(format!("Unknown session file extension: {:?}", path)),
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
