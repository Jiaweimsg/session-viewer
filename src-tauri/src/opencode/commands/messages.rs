use crate::opencode::parser::db_reader::{open_db, query_messages, query_parts_for_session};
use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

pub fn get_messages(
    session_id: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => {
            return Ok(PaginatedMessages {
                messages: vec![],
                total: 0,
                page,
                page_size,
                has_more: false,
            })
        }
    };

    let messages = query_messages(&conn, &session_id);

    let all_parts = query_parts_for_session(&conn, &session_id);
    let mut parts_by_message: std::collections::HashMap<String, Vec<_>> =
        std::collections::HashMap::new();
    for part in all_parts {
        parts_by_message.entry(part.message_id.clone()).or_default().push(part);
    }

    let mut all_valid = Vec::new();

    for msg in &messages {
        let role = match msg.data.get("role").and_then(|r| r.as_str()) {
            Some(r) => r.to_string(),
            None => continue,
        };

        let timestamp = Some(
            chrono::DateTime::from_timestamp(msg.time_created / 1000, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
        );

        let empty = vec![];
        let parts = parts_by_message.get(&msg.id).unwrap_or(&empty);
        let mut content_blocks = Vec::new();

        for part in parts {
            let part_type = part.data.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match part_type {
                "text" => {
                    if let Some(text) = part.data.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            content_blocks.push(DisplayContentBlock::Text {
                                text: text.to_string(),
                            });
                        }
                    }
                }
                "reasoning" => {
                    if let Some(text) = part.data.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            content_blocks.push(DisplayContentBlock::Reasoning {
                                text: text.to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        if !content_blocks.is_empty() {
            all_valid.push(DisplayMessage {
                uuid: Some(msg.id.clone()),
                role,
                timestamp,
                content: content_blocks,
            });
        }
    }

    let total = all_valid.len();
    let start = page * page_size;
    let end = std::cmp::min(start + page_size, total);
    let has_more = end < total;
    let messages = all_valid.into_iter().skip(start).take(page_size).collect();

    Ok(PaginatedMessages { messages, total, page, page_size, has_more })
}
