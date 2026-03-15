use rayon::prelude::*;

use crate::copilot::parser::session_parser::{parse_session_file, truncate};
use crate::copilot::parser::session_scanner::{
    get_workspace_storage_dir, scan_workspace_hashes, scan_session_files,
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotSearchResult {
    pub workspace_hash: String,
    pub session_id: String,
    pub file_path: String,
    pub first_prompt: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
}

pub fn global_search(query: String, max_results: usize) -> Result<Vec<CopilotSearchResult>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }

    let storage_dir = get_workspace_storage_dir()
        .ok_or_else(|| "Could not determine VS Code workspace storage directory".to_string())?;

    let query_lower = query.to_lowercase();

    let workspace_hashes = scan_workspace_hashes(&storage_dir);

    // Collect all session files with their workspace hash
    let mut all_files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for hash in &workspace_hashes {
        for file in scan_session_files(&storage_dir, hash) {
            all_files.push((hash.clone(), file));
        }
    }

    let results: Vec<CopilotSearchResult> = all_files
        .par_iter()
        .flat_map(|(workspace_hash, path)| {
            let parsed = match parse_session_file(path) {
                Ok(p) => p,
                Err(_) => return vec![],
            };

            let file_path = path.to_string_lossy().to_string();
            let session_id = parsed.session_id.clone();
            let first_prompt = parsed
                .requests
                .first()
                .map(|r| truncate(&r.user_text, 100))
                .filter(|s| !s.is_empty());

            let mut hits = Vec::new();

            // Search title
            if let Some(ref t) = parsed.title {
                if t.to_lowercase().contains(&query_lower) {
                    hits.push(CopilotSearchResult {
                        workspace_hash: workspace_hash.clone(),
                        session_id: session_id.clone(),
                        file_path: file_path.clone(),
                        first_prompt: first_prompt.clone(),
                        matched_text: extract_context(t, &query, 200),
                        role: "title".to_string(),
                        timestamp: None,
                    });
                }
            }

            // Search each request
            for req in &parsed.requests {
                // User message
                if req.user_text.to_lowercase().contains(&query_lower) {
                    let ts = if req.timestamp_ms > 0 {
                        chrono::DateTime::from_timestamp((req.timestamp_ms / 1000) as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                    } else {
                        None
                    };
                    hits.push(CopilotSearchResult {
                        workspace_hash: workspace_hash.clone(),
                        session_id: session_id.clone(),
                        file_path: file_path.clone(),
                        first_prompt: first_prompt.clone(),
                        matched_text: extract_context(&req.user_text, &query, 200),
                        role: "user".to_string(),
                        timestamp: ts,
                    });
                }

                // Assistant response text blocks
                for block in &req.response_blocks {
                    use crate::copilot::parser::session_parser::ResponseBlock;
                    let text = match block {
                        ResponseBlock::Text(t) => t,
                        _ => continue,
                    };
                    if text.to_lowercase().contains(&query_lower) {
                        let ts = if req.timestamp_ms > 0 {
                            chrono::DateTime::from_timestamp((req.timestamp_ms / 1000) as i64, 0)
                                .map(|dt| dt.to_rfc3339())
                        } else {
                            None
                        };
                        hits.push(CopilotSearchResult {
                            workspace_hash: workspace_hash.clone(),
                            session_id: session_id.clone(),
                            file_path: file_path.clone(),
                            first_prompt: first_prompt.clone(),
                            matched_text: extract_context(text, &query, 200),
                            role: "assistant".to_string(),
                            timestamp: ts,
                        });
                        break; // one hit per request response is enough
                    }
                }
            }

            hits
        })
        .collect();

    Ok(results.into_iter().take(max_results).collect())
}

fn extract_context(text: &str, query: &str, context_len: usize) -> String {
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(pos) = text_lower.find(&query_lower) {
        let start = pos.saturating_sub(context_len / 2);
        let end = std::cmp::min(pos + query_lower.len() + context_len / 2, text.len());

        // Align to char boundaries
        let start = text
            .char_indices()
            .map(|(i, _)| i)
            .filter(|&i| i <= start)
            .last()
            .unwrap_or(0);
        let end = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .filter(|&i| i >= end)
            .next()
            .unwrap_or(text.len());

        let mut snippet = text[start..end].to_string();
        if start > 0 {
            snippet = format!("...{}", snippet);
        }
        if end < text.len() {
            snippet = format!("{}...", snippet);
        }
        snippet
    } else {
        text.chars().take(context_len).collect()
    }
}
