use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use crate::codex::models::message::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

/// Codex JSONL line: {timestamp, type, payload}
/// type can be: "session_meta", "response_item", "event_msg", "turn_context"
/// Parse a Codex JSONL session file and return paginated display messages.
pub fn parse_session_messages(
    path: &Path,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let all_messages = parse_all_messages(path)?;

    let total = all_messages.len();
    let start = page * page_size;
    let end = (start + page_size).min(total);
    let has_more = end < total;

    let page_messages = if start < total {
        all_messages[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(PaginatedMessages {
        messages: page_messages,
        total,
        page,
        page_size,
        has_more,
    })
}

/// Parse all messages from a Codex JSONL file (no pagination, for search)
pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let mut messages: Vec<DisplayMessage> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = row.get("timestamp").and_then(|v| v.as_str()).map(String::from);
        let payload = match row.get("payload") {
            Some(p) => p,
            None => continue,
        };

        if row_type == "response_item" {
            let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match payload_type {
                "message" => {
                    let role = payload
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Skip developer messages (system instructions)
                    if role == "developer" || role == "system" {
                        continue;
                    }

                    if role == "user" || role == "assistant" {
                        let content_blocks = extract_message_content(payload);
                        if !content_blocks.is_empty() {
                            messages.push(DisplayMessage {
                                uuid: None,
                                role: role.to_string(),
                                timestamp: timestamp.clone(),
                                content: content_blocks,
                            });
                        }
                    }
                }
                "function_call" => {
                    let name = payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let arguments = payload
                        .get("arguments")
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                // Try to pretty-print if it's JSON
                                if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                                    serde_json::to_string_pretty(&parsed)
                                        .unwrap_or_else(|_| s.to_string())
                                } else {
                                    s.to_string()
                                }
                            } else {
                                serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
                            }
                        })
                        .unwrap_or_default();
                    let call_id = payload
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    messages.push(DisplayMessage {
                        uuid: None,
                        role: "assistant".to_string(),
                        timestamp: timestamp.clone(),
                        content: vec![DisplayContentBlock::FunctionCall {
                            name,
                            arguments,
                            call_id,
                        }],
                    });
                }
                "function_call_output" => {
                    let call_id = payload
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let output = payload
                        .get("output")
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
                            }
                        })
                        .unwrap_or_default();

                    messages.push(DisplayMessage {
                        uuid: None,
                        role: "tool".to_string(),
                        timestamp: timestamp.clone(),
                        content: vec![DisplayContentBlock::FunctionCallOutput {
                            call_id,
                            output,
                        }],
                    });
                }
                "reasoning" => {
                    let text = payload
                        .get("text")
                        .or_else(|| payload.get("summary").and_then(|s| s.get(0)))
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                // summary is array of {type, text}
                                if let Some(arr) = v.as_array() {
                                    arr.iter()
                                        .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                                        .collect::<Vec<&str>>()
                                        .join("\n")
                                } else {
                                    v.to_string()
                                }
                            }
                        })
                        .unwrap_or_default();

                    if !text.is_empty() {
                        messages.push(DisplayMessage {
                            uuid: None,
                            role: "assistant".to_string(),
                            timestamp: timestamp.clone(),
                            content: vec![DisplayContentBlock::Reasoning { text }],
                        });
                    }
                }
                _ => {}
            }
        }
        // Skip session_meta, event_msg, turn_context for message display
    }

    Ok(messages)
}

/// Extract content blocks from a message payload
fn extract_message_content(payload: &Value) -> Vec<DisplayContentBlock> {
    let mut blocks = Vec::new();

    if let Some(content) = payload.get("content") {
        if let Some(arr) = content.as_array() {
            for item in arr {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match item_type {
                    "input_text" | "output_text" | "text" => {
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !text.trim().is_empty() {
                            blocks.push(DisplayContentBlock::Text {
                                text: text.to_string(),
                            });
                        }
                    }
                    "reasoning" => {
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !text.trim().is_empty() {
                            blocks.push(DisplayContentBlock::Reasoning {
                                text: text.to_string(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        } else if let Some(s) = content.as_str() {
            if !s.trim().is_empty() {
                blocks.push(DisplayContentBlock::Text {
                    text: s.to_string(),
                });
            }
        }
    }

    blocks
}

/// Extract session metadata from the first line (session_meta)
pub struct SessionMeta {
    pub id: String,
    pub cwd: String,
    pub cli_version: Option<String>,
    pub model_provider: Option<String>,
    pub git_branch: Option<String>,
}

pub fn extract_session_meta(path: &Path) -> Option<SessionMeta> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(5) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if row_type == "session_meta" {
            if let Some(payload) = row.get("payload") {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let cwd = payload
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let cli_version = payload
                    .get("cli_version")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let model_provider = payload
                    .get("model_provider")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let git_branch = payload
                    .get("git")
                    .and_then(|g| g.get("branch"))
                    .and_then(|v| v.as_str())
                    .map(String::from);

                return Some(SessionMeta {
                    id,
                    cwd,
                    cli_version,
                    model_provider,
                    git_branch,
                });
            }
        }
    }
    None
}

/// Extract the first user prompt from a Codex JSONL file
pub fn extract_first_prompt(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Quick pre-filter
        if !trimmed.contains("\"role\"") || !trimmed.contains("\"user\"") {
            continue;
        }

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if row_type != "response_item" {
            continue;
        }

        if let Some(payload) = row.get("payload") {
            let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if payload_type != "message" {
                continue;
            }
            let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "user" {
                continue;
            }

            // Extract text from content array
            if let Some(content) = payload.get("content").and_then(|c| c.as_array()) {
                for item in content {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if item_type == "input_text" || item_type == "text" {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                return Some(truncate_string(text, 200));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Count messages (user and assistant) in a JSONL file
pub fn count_messages(path: &Path) -> u32 {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let mut count: u32 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();

        // Count response_item lines with message type (user/assistant)
        if trimmed.contains("\"response_item\"") && trimmed.contains("\"message\"") {
            // Skip developer messages
            if !trimmed.contains("\"developer\"") && !trimmed.contains("\"system\"") {
                count += 1;
            }
        }
    }
    count
}

/// Extract token counts from event_msg lines.
///
/// Codex semantics (differ from Anthropic):
/// - `total_token_usage.input_tokens` is **cumulative session-wide** and
///   **already includes `cached_input_tokens`** (the cached prefix is counted
///   inside input_tokens, not as a separate bucket).
/// - We surface `cached_input_tokens` so callers can split cached vs. fresh
///   input and align semantics with Anthropic (`input_tokens` = fresh only).
pub struct TokenInfo {
    pub input_tokens: u64,         // cumulative, includes cached
    #[allow(dead_code)]
    pub cached_input_tokens: u64,  // cumulative cached portion of input_tokens
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Per-day delta computed from adjacent cumulative `total_token_usage` snapshots.
/// `date` is taken from the event line's own `timestamp`, so a session that
/// spans midnight splits correctly.
#[derive(Debug, Clone)]
pub struct TokenDelta {
    pub date: String,            // YYYY-MM-DD
    pub input_fresh: u64,        // input_tokens delta minus cached delta
    pub cached: u64,             // cached_input_tokens delta
    pub output: u64,             // output_tokens delta
}

fn parse_token_usage(info: &Value) -> Option<(u64, u64, u64, u64)> {
    let usage = info.get("total_token_usage")?;
    let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let cached = usage
        .get("cached_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(input + output);
    Some((input, cached, output, total))
}

/// Session-wide cumulative totals (last snapshot wins). Used by stats page.
pub fn extract_token_info(path: &Path) -> Option<TokenInfo> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last: Option<TokenInfo> = None;

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("\"token_count\"") {
            continue;
        }
        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if row.get("type").and_then(|v| v.as_str()) != Some("event_msg") {
            continue;
        }
        let payload = match row.get("payload") {
            Some(p) => p,
            None => continue,
        };
        if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
            continue;
        }
        let info = match payload.get("info") {
            Some(i) => i,
            None => continue,
        };
        if let Some((input, cached, output, total)) = parse_token_usage(info) {
            last = Some(TokenInfo {
                input_tokens: input,
                cached_input_tokens: cached,
                output_tokens: output,
                total_tokens: total,
            });
        }
    }
    last
}

/// Per-day deltas derived from adjacent cumulative snapshots. Used by reports
/// so cross-midnight sessions credit each day correctly.
pub fn extract_token_deltas(path: &Path) -> Vec<TokenDelta> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);

    // Aggregate deltas by date; keys stay ordered by insertion for predictable output.
    let mut by_date: std::collections::HashMap<String, (u64, u64, u64)> = std::collections::HashMap::new();
    let mut prev_input: u64 = 0;
    let mut prev_cached: u64 = 0;
    let mut prev_output: u64 = 0;

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("\"token_count\"") {
            continue;
        }
        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if row.get("type").and_then(|v| v.as_str()) != Some("event_msg") {
            continue;
        }
        let payload = match row.get("payload") {
            Some(p) => p,
            None => continue,
        };
        if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
            continue;
        }
        let info = match payload.get("info") {
            Some(i) => i,
            None => continue,
        };
        let (input, cached, output, _total) = match parse_token_usage(info) {
            Some(v) => v,
            None => continue,
        };

        // Cumulative totals should never decrease; if they do (corrupt log),
        // treat as no-op to avoid negative deltas.
        let d_input = input.saturating_sub(prev_input);
        let d_cached = cached.saturating_sub(prev_cached);
        let d_output = output.saturating_sub(prev_output);
        if d_input == 0 && d_cached == 0 && d_output == 0 {
            continue; // duplicate / rate-limit-only re-emit
        }

        let date = row
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|ts| ts.get(..10))
            .unwrap_or("")
            .to_string();
        if date.is_empty() {
            continue;
        }

        // input_fresh = total input delta - cached delta, so we never double-count
        // the cached portion when the server sums input + cache_read.
        let input_fresh = d_input.saturating_sub(d_cached);

        let entry = by_date.entry(date).or_insert((0, 0, 0));
        entry.0 += input_fresh;
        entry.1 += d_cached;
        entry.2 += d_output;

        prev_input = input;
        prev_cached = cached;
        prev_output = output;
    }

    by_date
        .into_iter()
        .map(|(date, (input_fresh, cached, output))| TokenDelta {
            date,
            input_fresh,
            cached,
            output,
        })
        .collect()
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}
