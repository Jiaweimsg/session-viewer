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
                // Initial state snapshot — may contain pre-existing requests
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
                    // Load any requests already present in the initial snapshot
                    if let Some(reqs) = v.get("requests").and_then(|r| r.as_array()) {
                        for req_obj in reqs {
                            let idx = request_order.len();
                            request_order.push(idx);
                            let req_model = req_obj
                                .get("modelId")
                                .and_then(|m| m.as_str())
                                .map(|s| s.to_string());
                            let initial_resp = req_obj
                                .get("response")
                                .and_then(|r| r.as_array())
                                .cloned()
                                .unwrap_or_default();
                            requests_meta.insert(idx, (req_obj.clone(), initial_resp, req_model, 0, 0));
                        }
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
                    // New request appended — initialize with embedded response items
                    if let Some(arr) = v.and_then(|v| v.as_array()) {
                        for req_obj in arr {
                            let idx = request_order.len();
                            request_order.push(idx);
                            let req_model = req_obj
                                .get("modelId")
                                .and_then(|m| m.as_str())
                                .map(|s| s.to_string());
                            let initial_resp = req_obj
                                .get("response")
                                .and_then(|r| r.as_array())
                                .cloned()
                                .unwrap_or_default();
                            requests_meta.insert(idx, (req_obj.clone(), initial_resp, req_model, 0, 0));
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

// ─── Lightweight metadata scan (for session list) ────────────────────────────

/// Minimal session metadata needed for the sessions list page.
/// Much faster than full parsing — skips building response blocks entirely.
pub struct SessionMeta {
    pub session_id: String,
    pub created_ms: u64,
    pub title: Option<String>,
    pub model_id: Option<String>,
    pub first_prompt: Option<String>,
    pub message_count: u32,
}

pub fn scan_session_metadata(path: &PathBuf) -> Result<SessionMeta, String> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => scan_json_metadata(path),
        Some("jsonl") => scan_jsonl_metadata(path),
        _ => Err(format!("Unknown extension: {:?}", path)),
    }
}

fn scan_json_metadata(path: &PathBuf) -> Result<SessionMeta, String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = fs::File::open(path).map_err(|e| format!("open error: {}", e))?;
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    // For small files (<= 512KB) do a full minimal parse
    if file_size <= 512 * 1024 {
        return scan_json_metadata_small(path);
    }

    // For large files: metadata is at the end, first prompt at the beginning.
    // Avoid full parse by reading just the tail (metadata) + head (first prompt).

    // --- Read tail (last 8KB) for session-level metadata ---
    let tail_size = 8192u64.min(file_size);
    file.seek(SeekFrom::End(-(tail_size as i64)))
        .map_err(|e| format!("seek error: {}", e))?;
    let mut tail = vec![0u8; tail_size as usize];
    file.read_exact(&mut tail).map_err(|e| format!("read tail error: {}", e))?;
    let tail_str = String::from_utf8_lossy(&tail);

    let session_id = extract_json_str(&tail_str, "sessionId")
        .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string());
    let created_ms = extract_json_u64(&tail_str, "creationDate").unwrap_or(0);
    let title = extract_json_str(&tail_str, "customTitle");

    // --- Read head (first 4KB) for first prompt ---
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek error: {}", e))?;
    let head_size = 4096u64.min(file_size);
    let mut head = vec![0u8; head_size as usize];
    file.read_exact(&mut head).map_err(|e| format!("read head error: {}", e))?;
    let head_str = String::from_utf8_lossy(&head);

    let first_prompt = extract_json_str(&head_str, "text").map(|t| truncate(&t, 150));

    // --- Count requests by scanning for "requestId" pattern ---
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek error: {}", e))?;
    let mut all = Vec::with_capacity(file_size as usize);
    file.read_to_end(&mut all).map_err(|e| format!("read error: {}", e))?;
    let needle = b"\"requestId\"";
    let message_count = all.windows(needle.len()).filter(|w| *w == needle).count() as u32;

    // Last model_id from tail
    let model_id = extract_json_str(&tail_str, "identifier")
        .or_else(|| extract_json_str(&tail_str, "modelId"));

    Ok(SessionMeta { session_id, created_ms, title, model_id, first_prompt, message_count })
}

/// Tiny JSON string extractor — finds `"key": "value"` in text
fn extract_json_str(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = text.rfind(&needle)?;
    let after = text[start + needle.len()..].trim_start();
    if after.starts_with('"') {
        let inner = &after[1..];
        let end = inner.find('"')?;
        Some(inner[..end].replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\"))
    } else {
        None
    }
}

fn extract_json_u64(text: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\":", key);
    let start = text.rfind(&needle)?;
    let after = text[start + needle.len()..].trim_start();
    let end = after.find(|c: char| !c.is_ascii_digit())?;
    after[..end].parse().ok()
}

fn scan_json_metadata_small(path: &PathBuf) -> Result<SessionMeta, String> {
    use std::io::BufReader;

    #[derive(Deserialize)]
    struct MetaReq {
        message: Option<MetaMsg>,
        #[serde(rename = "modelId", default)]
        model_id: Option<String>,
    }
    #[derive(Deserialize)]
    struct MetaMsg {
        text: Option<String>,
    }
    #[derive(Deserialize)]
    struct MetaSession {
        #[serde(rename = "sessionId", default)]
        session_id: Option<String>,
        #[serde(rename = "creationDate", default)]
        creation_date: Option<u64>,
        #[serde(rename = "customTitle", default)]
        custom_title: Option<String>,
        #[serde(default)]
        requests: Vec<MetaReq>,
    }

    let file = fs::File::open(path).map_err(|e| format!("open error: {}", e))?;
    let session: MetaSession = serde_json::from_reader(BufReader::new(file))
        .map_err(|e| format!("parse error: {}", e))?;

    let session_id = session.session_id.unwrap_or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
    });
    let message_count = session.requests.len() as u32;
    let first_prompt = session.requests.first()
        .and_then(|r| r.message.as_ref())
        .and_then(|m| m.text.as_deref())
        .map(|t| truncate(t, 150));
    let model_id = session.requests.iter()
        .filter_map(|r| r.model_id.as_deref())
        .last()
        .map(|s| s.to_string());

    Ok(SessionMeta {
        session_id,
        created_ms: session.creation_date.unwrap_or(0),
        title: session.custom_title,
        model_id,
        first_prompt,
        message_count,
    })
}

fn scan_jsonl_metadata(path: &PathBuf) -> Result<SessionMeta, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("read error: {}", e))?;

    let mut session_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
    let mut created_ms: u64 = 0;
    let mut title: Option<String> = None;
    let mut model_id: Option<String> = None;
    let mut first_prompt: Option<String> = None;
    let mut message_count: u32 = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        // Fast-path: skip response append lines without JSON parsing.
        // These are kind=2 events appending to requests[N].response — the largest lines.
        // Pattern: {"kind":2,"k":["requests",<number>,"response"],...}
        if line.starts_with(r#"{"kind":2,"k":["requests","#)
            || (line.starts_with(r#"{"kind":2,"k":["requests"#) && line.contains(r#","response"]"#))
        {
            continue;
        }

        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let kind = obj["kind"].as_i64().unwrap_or(-1);
        match kind {
            0 => {
                let v = &obj["v"];
                if let Some(id) = v["sessionId"].as_str() { session_id = id.to_string(); }
                if let Some(cd) = v["creationDate"].as_u64() { created_ms = cd; }
                if let Some(ct) = v["customTitle"].as_str() { title = Some(ct.to_string()); }
                // Count + first prompt from initial requests snapshot
                if let Some(reqs) = v["requests"].as_array() {
                    message_count += reqs.len() as u32;
                    if first_prompt.is_none() {
                        first_prompt = reqs.first()
                            .and_then(|r| r["message"]["text"].as_str())
                            .map(|t| truncate(t, 150));
                    }
                    // Last model in snapshot
                    if let Some(m) = reqs.iter().filter_map(|r| r["modelId"].as_str()).last() {
                        model_id = Some(m.to_string());
                    }
                }
            }
            1 => {
                // Track model updates
                let k = obj["k"].as_array();
                if let Some(k) = k {
                    if k.len() == 3
                        && k[0].as_str() == Some("inputState")
                        && k[1].as_str() == Some("selectedModel")
                    {
                        if let Some(id) = obj["v"]["identifier"].as_str() {
                            model_id = Some(id.to_string());
                        }
                    } else if k.len() == 3
                        && k[0].as_str() == Some("requests")
                        && k[2].as_str() == Some("modelId")
                    {
                        if let Some(m) = obj["v"].as_str() {
                            model_id = Some(m.to_string());
                        }
                    } else if k.len() == 1 && k[0].as_str() == Some("customTitle") {
                        if let Some(t) = obj["v"].as_str() {
                            title = Some(t.to_string());
                        }
                    }
                }
            }
            2 => {
                let k = obj["k"].as_array();
                // Only count top-level request appends, skip response appends
                if let Some(k) = k {
                    if k.len() == 1 && k[0].as_str() == Some("requests") {
                        if let Some(arr) = obj["v"].as_array() {
                            message_count += arr.len() as u32;
                            // Capture first prompt if not yet set
                            if first_prompt.is_none() {
                                first_prompt = arr.first()
                                    .and_then(|r| r["message"]["text"].as_str())
                                    .map(|t| truncate(t, 150));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(SessionMeta { session_id, created_ms, title, model_id, first_prompt, message_count })
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
