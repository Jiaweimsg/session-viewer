use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use crate::copilot::models::message::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

/// Parse the events.jsonl for a Copilot CLI session and return paginated messages.
///
/// Messages are grouped so that all assistant turns between two user messages are
/// combined into a single assistant DisplayMessage (matching the logical conversation flow).
pub fn parse_session_messages(
    events_path: &Path,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    // First pass: collect tool outputs keyed by toolCallId
    let mut tool_outputs: std::collections::HashMap<String, String> = Default::default();
    {
        let file =
            fs::File::open(events_path).map_err(|e| format!("Failed to open events.jsonl: {}", e))?;
        for line in BufReader::new(file).lines().filter_map(|l| l.ok()) {
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
    let file =
        fs::File::open(events_path).map_err(|e| format!("Failed to open events.jsonl: {}", e))?;

    let mut messages: Vec<DisplayMessage> = Vec::new();
    // Accumulated blocks for the current assistant response (across multiple turns)
    let mut pending_asst_blocks: Vec<DisplayContentBlock> = Vec::new();
    let mut pending_asst_ts: Option<String> = None;
    let mut msg_id_counter = 0usize;

    let flush_assistant = |blocks: &mut Vec<DisplayContentBlock>,
                           ts: &mut Option<String>,
                           counter: &mut usize,
                           msgs: &mut Vec<DisplayMessage>| {
        if !blocks.is_empty() {
            *counter += 1;
            msgs.push(DisplayMessage {
                id: format!("asst-{}", counter),
                role: "assistant".to_string(),
                timestamp: ts.take(),
                content: std::mem::take(blocks),
            });
        }
    };

    for line in BufReader::new(file).lines().filter_map(|l| l.ok()) {
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
                // Flush any pending assistant blocks before emitting user message
                flush_assistant(
                    &mut pending_asst_blocks,
                    &mut pending_asst_ts,
                    &mut msg_id_counter,
                    &mut messages,
                );

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

                    // Record timestamp of the first block in this response
                    if pending_asst_ts.is_none() {
                        pending_asst_ts = ts;
                    }

                    if !text.trim().is_empty() {
                        pending_asst_blocks.push(DisplayContentBlock {
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
                            // Skip internal/cosmetic tools from display
                            if name == "report_intent" || name == "update_intent" {
                                continue;
                            }
                            let call_id = req["toolCallId"].as_str().unwrap_or("");
                            let args = req["arguments"]
                                .as_object()
                                .map(|o| serde_json::to_string_pretty(o).unwrap_or_default())
                                .or_else(|| req["arguments"].as_str().map(|s| s.to_string()));
                            let output = tool_outputs.get(call_id).cloned();

                            pending_asst_blocks.push(DisplayContentBlock {
                                block_type: "tool_use".to_string(),
                                text: None,
                                tool_name: Some(name),
                                tool_input: args,
                                tool_output: output,
                            });
                        }
                    }
                }
            }

            _ => {}
        }
    }

    // Flush any trailing assistant blocks
    flush_assistant(
        &mut pending_asst_blocks,
        &mut pending_asst_ts,
        &mut msg_id_counter,
        &mut messages,
    );

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
