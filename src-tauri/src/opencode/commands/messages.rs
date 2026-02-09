use std::fs;
use crate::opencode::parser::json_parser::parse_message;
use crate::opencode::parser::session_scanner::get_message_dir;
use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct MessagePart {
    id: String,
    #[serde(rename = "messageID")]
    message_id: String,
    #[serde(rename = "type")]
    part_type: String,
    text: Option<String>,
}

fn get_part_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(".local/share/opencode/storage/part"))
}

fn read_message_parts(message_id: &str) -> Vec<String> {
    let part_dir = match get_part_dir() {
        Some(dir) => dir.join(message_id),
        None => return vec![],
    };

    if !part_dir.exists() {
        return vec![];
    }

    let mut parts = Vec::new();
    
    if let Ok(entries) = fs::read_dir(&part_dir) {
        let mut part_files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
            .collect();
        
        // Sort by filename to maintain order
        part_files.sort_by_key(|e| e.path());
        
        for entry in part_files {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(part) = serde_json::from_str::<MessagePart>(&content) {
                    if part.part_type == "text" {
                        if let Some(text) = part.text {
                            parts.push(text);
                        }
                    }
                }
            }
        }
    }
    
    parts
}

pub fn get_messages(
    session_id: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let message_dir_base = get_message_dir()
        .ok_or_else(|| "Could not find OpenCode message directory".to_string())?;

    let message_dir = message_dir_base.join(&session_id);

    if !message_dir.exists() {
        return Ok(PaginatedMessages {
            messages: vec![],
            total: 0,
            page,
            page_size,
            has_more: false,
        });
    }

    // Collect all message files
    let mut message_files: Vec<_> = fs::read_dir(&message_dir)
        .map_err(|e| format!("Failed to read message directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();

    // Sort by file metadata (created time)
    message_files.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|m| m.created())
            .ok()
    });

    // First pass: collect all valid messages (with content)
    let mut all_valid_messages = Vec::new();
    for entry in message_files.iter() {
        let path = entry.path();
        if let Ok(msg_meta) = parse_message(&path) {
            let timestamp = Some(
                chrono::DateTime::from_timestamp((msg_meta.time.created / 1000) as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            );

            let mut content_blocks = Vec::new();

            // Read actual message content from part directory
            let parts = read_message_parts(&msg_meta.id);
            
            if !parts.is_empty() {
                // Combine all parts into one text block
                let combined_text = parts.join("\n\n");
                content_blocks.push(DisplayContentBlock::Text {
                    text: combined_text,
                });
            } else {
                // Fallback to summary title if no parts found
                if let Some(ref summary) = msg_meta.summary {
                    if let Some(ref title) = summary.title {
                        if !title.trim().is_empty() {
                            content_blocks.push(DisplayContentBlock::Text {
                                text: title.clone(),
                            });
                        }
                    }
                }
            }

            // Only include messages that have actual content
            if !content_blocks.is_empty() {
                all_valid_messages.push(DisplayMessage {
                    uuid: Some(msg_meta.id.clone()),
                    role: msg_meta.role.clone(),
                    timestamp,
                    content: content_blocks,
                });
            }
        }
    }

    // Now paginate the valid messages
    let total = all_valid_messages.len();
    let start = page * page_size;
    let end = std::cmp::min(start + page_size, total);
    let has_more = end < total;

    let messages = all_valid_messages.into_iter().skip(start).take(page_size).collect();

    Ok(PaginatedMessages {
        messages,
        total,
        page,
        page_size,
        has_more,
    })
}
