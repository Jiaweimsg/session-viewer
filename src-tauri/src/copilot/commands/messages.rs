use std::path::PathBuf;

use crate::copilot::parser::session_parser::{parse_session_file, ResponseBlock};
use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

pub fn get_messages(
    session_key: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let path = PathBuf::from(&session_key);

    if !path.exists() {
        // session_key might be session_id relative to workspace; try to find it
        return Ok(PaginatedMessages {
            messages: vec![],
            total: 0,
            page,
            page_size,
            has_more: false,
        });
    }

    let parsed = parse_session_file(&path)?;

    // Build display messages: each ParsedRequest → one user message + one assistant message
    let mut all_messages: Vec<DisplayMessage> = Vec::new();

    for req in &parsed.requests {
        // User message
        if !req.user_text.is_empty() {
            let timestamp = if req.timestamp_ms > 0 {
                chrono::DateTime::from_timestamp((req.timestamp_ms / 1000) as i64, 0)
                    .map(|dt| dt.to_rfc3339())
            } else {
                None
            };

            all_messages.push(DisplayMessage {
                uuid: Some(req.request_id.clone()),
                role: "user".to_string(),
                timestamp,
                content: vec![DisplayContentBlock::Text {
                    text: req.user_text.clone(),
                }],
            });
        }

        // Assistant message (from response blocks)
        if !req.response_blocks.is_empty() {
            let timestamp = if req.timestamp_ms > 0 {
                chrono::DateTime::from_timestamp((req.timestamp_ms / 1000) as i64, 0)
                    .map(|dt| dt.to_rfc3339())
            } else {
                None
            };

            let content: Vec<DisplayContentBlock> = req
                .response_blocks
                .iter()
                .filter_map(|block| match block {
                    ResponseBlock::Text(t) => {
                        if t.trim().is_empty() {
                            None
                        } else {
                            Some(DisplayContentBlock::Text { text: t.clone() })
                        }
                    }
                    ResponseBlock::Thinking(t) => {
                        if t.trim().is_empty() {
                            None
                        } else {
                            Some(DisplayContentBlock::Thinking { thinking: t.clone() })
                        }
                    }
                    ResponseBlock::ToolUse { name, input } => {
                        Some(DisplayContentBlock::ToolUse {
                            id: String::new(),
                            name: name.clone(),
                            input: input.clone(),
                        })
                    }
                })
                .collect();

            if !content.is_empty() {
                all_messages.push(DisplayMessage {
                    uuid: Some(format!("{}_response", req.request_id)),
                    role: "assistant".to_string(),
                    timestamp,
                    content,
                });
            }
        }
    }

    let total = all_messages.len();
    let start = page * page_size;
    let end = std::cmp::min(start + page_size, total);
    let has_more = end < total;

    let messages = all_messages
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect();

    Ok(PaginatedMessages {
        messages,
        total,
        page,
        page_size,
        has_more,
    })
}
