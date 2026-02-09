use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

use crate::opencode::parser::json_parser::parse_message;
use crate::opencode::parser::session_scanner::{get_message_dir, scan_all_session_files};

/// Search result for OpenCode
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeSearchResult {
    pub project_id: String,
    pub session_id: String,
    pub first_prompt: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
    pub message_id: String,
}

pub fn global_search(query: String, max_results: usize) -> Result<Vec<OpencodeSearchResult>, String> {
    let message_dir_base = get_message_dir()
        .ok_or_else(|| "Could not find OpenCode message directory".to_string())?;

    if query.is_empty() {
        return Ok(vec![]);
    }

    let query_lower = query.to_lowercase();

    // Get all session directories
    let session_dirs: Vec<PathBuf> = fs::read_dir(&message_dir_base)
        .map_err(|e| format!("Failed to read message directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path())
        .collect();

    // Parallel search across all message files
    let results: Vec<OpencodeSearchResult> = session_dirs
        .par_iter()
        .flat_map(|session_dir| {
            let session_id = session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let message_files: Vec<PathBuf> = fs::read_dir(session_dir)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
                        .map(|e| e.path())
                        .collect()
                })
                .unwrap_or_default();

            message_files
                .into_iter()
                .filter_map(|path| {
                    let msg_meta = parse_message(&path).ok()?;

                    // Search in title, system prompt
                    let mut matched_text = None;

                    if let Some(ref summary) = msg_meta.summary {
                        if let Some(ref title) = summary.title {
                            if title.to_lowercase().contains(&query_lower) {
                                matched_text = Some(extract_match_context(title, &query, 200));
                            }
                        }
                    }

                    if matched_text.is_none() {
                        if let Some(ref system) = msg_meta.system {
                            if system.to_lowercase().contains(&query_lower) {
                                matched_text = Some(extract_match_context(system, &query, 200));
                            }
                        }
                    }

                    matched_text.map(|text| {
                        let timestamp = Some(
                            chrono::DateTime::from_timestamp((msg_meta.time.created / 1000) as i64, 0)
                                .map(|dt| dt.to_rfc3339())
                                .unwrap_or_default(),
                        );

                        OpencodeSearchResult {
                            project_id: "".to_string(), // Will be filled later if needed
                            session_id: session_id.clone(),
                            first_prompt: None,
                            matched_text: text,
                            role: msg_meta.role.clone(),
                            timestamp,
                            message_id: msg_meta.id.clone(),
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    // Take only the first max_results
    let results = results.into_iter().take(max_results).collect();

    Ok(results)
}

fn extract_match_context(text: &str, query: &str, context_len: usize) -> String {
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(pos) = text_lower.find(&query_lower) {
        let start = pos.saturating_sub(context_len / 2);
        let end = std::cmp::min(pos + query_lower.len() + context_len / 2, text.len());

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
