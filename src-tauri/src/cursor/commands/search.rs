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
                    // Truncate match context
                    let matched = if text.len() > 200 {
                        format!("{}...", &text[..200])
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
