use std::path::PathBuf;
use std::fs;
use crate::opencode::models::project::ProjectMetadata;
use crate::opencode::models::session::SessionMetadata;
use crate::opencode::models::message::MessageMetadata;

/// Parse project JSON file
pub fn parse_project(path: &PathBuf) -> Result<ProjectMetadata, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read project file: {}", e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse project JSON: {}", e))
}

/// Parse session JSON file
pub fn parse_session(path: &PathBuf) -> Result<SessionMetadata, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read session file: {}", e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse session JSON: {}", e))
}

/// Parse message JSON file
pub fn parse_message(path: &PathBuf) -> Result<MessageMetadata, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read message file: {}", e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse message JSON: {}", e))
}

/// Extract first user message content as first prompt
pub fn extract_first_prompt(message_dir: &PathBuf) -> Option<String> {
    let entries = fs::read_dir(message_dir).ok()?;
    
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(msg) = parse_message(&path) {
                if msg.role == "user" {
                    // Return title from summary, or truncate system prompt
                    if let Some(ref summary) = msg.summary {
                        if let Some(ref title) = summary.title {
                            return Some(title.clone());
                        }
                    }
                    // Fallback: truncate system prompt if available
                    if let Some(ref system) = msg.system {
                        return Some(truncate_text(system, 100));
                    }
                }
            }
        }
    }
    
    None
}

/// Count messages in a message directory
pub fn count_messages(message_dir: &PathBuf) -> u32 {
    fs::read_dir(message_dir)
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
                .count() as u32
        })
        .unwrap_or(0)
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}
