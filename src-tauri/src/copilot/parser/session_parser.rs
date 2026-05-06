use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

/// Internal tools that are not meaningful to display to the user
const SKIP_TOOLS: &[&str] = &["report_intent", "update_intent"];

/// Parse the events.jsonl for a Copilot CLI session and return paginated messages.
///
/// All assistant turns between two user messages are merged into a single
/// assistant DisplayMessage so the conversation looks natural.
pub fn parse_session_messages(
    events_path: &Path,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    // First pass: collect tool outputs keyed by toolCallId
    let mut tool_outputs: std::collections::HashMap<String, String> = Default::default();
    {
        let file = fs::File::open(events_path)
            .map_err(|e| format!("Failed to open events.jsonl: {}", e))?;
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.contains("\"type\":\"tool.execution_complete\"") {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                if let (Some(id), Some(output)) = (
                    v["data"]["toolCallId"].as_str(),
                    v["data"]["output"].as_str(),
                ) {
                    tool_outputs.insert(id.to_string(), output.to_string());
                }
            }
        }
    }

    // Second pass: build messages, grouping consecutive assistant turns into one
    let file = fs::File::open(events_path)
        .map_err(|e| format!("Failed to open events.jsonl: {}", e))?;

    let mut messages: Vec<DisplayMessage> = Vec::new();
    let mut pending_blocks: Vec<DisplayContentBlock> = Vec::new();
    let mut pending_ts: Option<String> = None;
    let mut msg_id_counter = 0usize;

    macro_rules! flush_assistant {
        () => {
            if !pending_blocks.is_empty() {
                msg_id_counter += 1;
                messages.push(DisplayMessage {
                    uuid: Some(format!("copilot-asst-{}", msg_id_counter)),
                    role: "assistant".to_string(),
                    timestamp: pending_ts.take(),
                    content: std::mem::take(&mut pending_blocks),
                });
            }
        };
    }

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event_type = if let Some(start) = trimmed.find("\"type\":\"") {
            let rest = &trimmed[start + 8..];
            rest.find('"').map(|end| rest[..end].to_string())
        } else {
            None
        };

        match event_type.as_deref() {
            Some("user.message") => {
                flush_assistant!();

                if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                    let content = v["data"]["content"].as_str().unwrap_or("").to_string();
                    let ts = v["timestamp"].as_str().map(|s| s.to_string());
                    // Skip auto-injected resume context messages
                    let is_resume = content.contains("RESUME CONTEXT FOR CONTINUING TASK");
                    if !content.trim().is_empty() && !is_resume {
                        msg_id_counter += 1;
                        messages.push(DisplayMessage {
                            uuid: Some(format!("copilot-user-{}", msg_id_counter)),
                            role: "user".to_string(),
                            timestamp: ts,
                            content: vec![DisplayContentBlock::Text { text: content }],
                        });
                    }
                }
            }

            Some("assistant.message") => {
                if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                    let text = v["data"]["content"].as_str().unwrap_or("").to_string();
                    let ts = v["timestamp"].as_str().map(|s| s.to_string());

                    if pending_ts.is_none() {
                        pending_ts = ts;
                    }

                    if !text.trim().is_empty() {
                        pending_blocks.push(DisplayContentBlock::Text { text });
                    }

                    if let Some(tool_requests) = v["data"]["toolRequests"].as_array() {
                        for req in tool_requests {
                            let name = req["name"].as_str().unwrap_or("").to_string();
                            if SKIP_TOOLS.contains(&name.as_str()) {
                                continue;
                            }
                            let call_id = req["toolCallId"]
                                .as_str()
                                .unwrap_or("")
                                .to_string();
                            let input = req["arguments"]
                                .as_object()
                                .map(|o| serde_json::to_string_pretty(o).unwrap_or_default())
                                .or_else(|| req["arguments"].as_str().map(|s| s.to_string()))
                                .unwrap_or_default();

                            pending_blocks.push(DisplayContentBlock::ToolUse {
                                id: call_id.clone(),
                                name: name.clone(),
                                input,
                            });

                            if let Some(output) = tool_outputs.get(&call_id) {
                                pending_blocks.push(DisplayContentBlock::ToolResult {
                                    tool_use_id: call_id,
                                    content: output.clone(),
                                    is_error: false,
                                });
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    flush_assistant!();

    let total = messages.len();
    let start = page * page_size;
    let end = (start + page_size).min(total);
    let has_more = end < total;
    let page_messages = if start < total {
        messages[start..end].to_vec()
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
