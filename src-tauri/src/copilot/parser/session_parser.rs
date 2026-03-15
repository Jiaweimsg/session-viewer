use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use crate::copilot::models::message::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

/// Parse the events.jsonl for a Copilot CLI session and return paginated messages
pub fn parse_session_messages(
    events_path: &Path,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let file =
        fs::File::open(events_path).map_err(|e| format!("Failed to open events.jsonl: {}", e))?;
    let reader = BufReader::new(file);

    let mut messages: Vec<DisplayMessage> = Vec::new();
    let mut turn_tool_results: std::collections::HashMap<String, String> = Default::default();

    // First pass: collect tool outputs keyed by toolCallId
    for line in BufReader::new(
        fs::File::open(events_path).map_err(|e| format!("{}", e))?,
    )
    .lines()
    .filter_map(|l| l.ok())
    {
        let trimmed = line.trim();
        if !trimmed.contains("\"type\":\"tool.execution_complete\"") {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            if let (Some(id), Some(output)) = (
                v["data"]["toolCallId"].as_str(),
                v["data"]["output"].as_str(),
            ) {
                turn_tool_results.insert(id.to_string(), output.to_string());
            }
        }
    }

    let mut msg_id_counter = 0usize;

    for line in reader.lines().filter_map(|l| l.ok()) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event_type = if let Some(start) = trimmed.find("\"type\":\"") {
            let rest = &trimmed[start + 8..];
            rest.find('"').map(|end| &rest[..end])
        } else {
            None
        };

        match event_type {
            Some("user.message") => {
                if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                    let content = v["data"]["content"].as_str().unwrap_or("").to_string();
                    let ts = v["timestamp"].as_str().map(|s| s.to_string());
                    if !content.trim().is_empty() {
                        msg_id_counter += 1;
                        messages.push(DisplayMessage {
                            id: format!("user-{}", msg_id_counter),
                            role: "user".to_string(),
                            timestamp: ts,
                            content: vec![DisplayContentBlock {
                                block_type: "text".to_string(),
                                text: Some(content),
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                            }],
                        });
                    }
                }
            }
            Some("assistant.message") => {
                if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                    let text = v["data"]["content"].as_str().unwrap_or("").to_string();
                    let ts = v["timestamp"].as_str().map(|s| s.to_string());
                    let msg_id = v["id"].as_str().unwrap_or("").to_string();

                    let mut blocks: Vec<DisplayContentBlock> = Vec::new();

                    if !text.trim().is_empty() {
                        blocks.push(DisplayContentBlock {
                            block_type: "text".to_string(),
                            text: Some(text),
                            tool_name: None,
                            tool_input: None,
                            tool_output: None,
                        });
                    }

                    if let Some(tool_requests) = v["data"]["toolRequests"].as_array() {
                        for req in tool_requests {
                            let name = req["name"].as_str().unwrap_or("").to_string();
                            let call_id = req["toolCallId"].as_str().unwrap_or("");
                            let args = req["arguments"]
                                .as_object()
                                .map(|o| serde_json::to_string_pretty(o).unwrap_or_default())
                                .or_else(|| req["arguments"].as_str().map(|s| s.to_string()));
                            let output = turn_tool_results.get(call_id).cloned();

                            blocks.push(DisplayContentBlock {
                                block_type: "tool_use".to_string(),
                                text: None,
                                tool_name: Some(name),
                                tool_input: args,
                                tool_output: output,
                            });
                        }
                    }

                    if !blocks.is_empty() {
                        msg_id_counter += 1;
                        messages.push(DisplayMessage {
                            id: if msg_id.is_empty() {
                                format!("asst-{}", msg_id_counter)
                            } else {
                                msg_id
                            },
                            role: "assistant".to_string(),
                            timestamp: ts,
                            content: blocks,
                        });
                    }
                }
            }
            _ => {}
        }
    }

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
