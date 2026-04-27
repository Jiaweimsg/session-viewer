use serde::Serialize;
use crate::cursor::parser::project_scanner::{
    read_composer_headers, read_bubbles, epoch_ms_to_rfc3339,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSearchResult {
    pub session_id: String,
    pub project_name: Option<String>,
    pub session_name: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
}

pub fn global_search(query: String, max_results: usize) -> Result<Vec<CursorSearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let q = query.to_lowercase();
    let headers = read_composer_headers();
    let mut results = Vec::new();

    for h in &headers {
        if results.len() >= max_results {
            break;
        }

        let project_name = h
            .workspace_path
            .as_deref()
            .map(crate::shared_models::basename);

        // Search in session name/subtitle first
        if let Some(ref name) = h.name {
            if name.to_lowercase().contains(&q) {
                results.push(CursorSearchResult {
                    session_id: h.composer_id.clone(),
                    project_name: project_name.clone(),
                    session_name: h.name.clone(),
                    matched_text: name.clone(),
                    role: "session".to_string(),
                    timestamp: h.created_at.map(epoch_ms_to_rfc3339),
                });
                if results.len() >= max_results {
                    break;
                }
            }
        }

        // Search in bubble messages
        let bubbles = read_bubbles(&h.composer_id);
        for b in &bubbles {
            if results.len() >= max_results {
                break;
            }
            if let Some(ref text) = b.text {
                if text.to_lowercase().contains(&q) {
                    let role = match b.msg_type {
                        1 => "human",
                        2 => "assistant",
                        _ => "unknown",
                    };
                    // Truncate match context (char-safe: byte slicing would panic
                    // when the 200-byte boundary lands inside a multi-byte UTF-8
                    // sequence — common when the prompt is Chinese).
                    let matched = if text.chars().count() > 200 {
                        let truncated: String = text.chars().take(200).collect();
                        format!("{}...", truncated)
                    } else {
                        text.clone()
                    };
                    results.push(CursorSearchResult {
                        session_id: h.composer_id.clone(),
                        project_name: project_name.clone(),
                        session_name: h.name.clone(),
                        matched_text: matched,
                        role: role.to_string(),
                        timestamp: b.created_at.clone(),
                    });
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    /// Replicates the truncation arithmetic from `global_search`. Guards against
    /// a regression where byte slicing was used and panicked on Chinese prompts
    /// whose 200-byte boundary fell inside a multi-byte sequence.
    fn truncate_for_match(text: &str) -> String {
        if text.chars().count() > 200 {
            let truncated: String = text.chars().take(200).collect();
            format!("{}...", truncated)
        } else {
            text.to_string()
        }
    }

    #[test]
    fn short_ascii_unchanged() {
        assert_eq!(truncate_for_match("hello"), "hello");
    }

    #[test]
    fn short_chinese_unchanged() {
        assert_eq!(truncate_for_match("你好世界"), "你好世界");
    }

    #[test]
    fn long_chinese_does_not_panic_and_truncates_to_chars() {
        // 300 Chinese chars = 900 bytes; pure byte slicing at 200 used to panic.
        let long: String = "中".repeat(300);
        let out = truncate_for_match(&long);
        assert!(out.ends_with("..."));
        // Should be 200 chars + "..."; ensure char count, not byte count.
        let body: String = "中".repeat(200);
        assert_eq!(out, format!("{}...", body));
    }

    #[test]
    fn long_ascii_truncates_to_200_chars_plus_ellipsis() {
        let long = "a".repeat(500);
        let out = truncate_for_match(&long);
        assert_eq!(out.len(), 200 + 3);
    }
}
