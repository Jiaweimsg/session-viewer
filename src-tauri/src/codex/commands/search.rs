use rayon::prelude::*;
use serde::Serialize;
use std::fs;

use crate::codex::parser::jsonl::{extract_session_meta, parse_all_messages};
use crate::codex::parser::session_scanner::{scan_all_session_files, short_name_from_path};
use crate::shared_models::DisplayContentBlock;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub cwd: String,
    pub short_name: String,
    pub session_id: String,
    pub first_prompt: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
    pub file_path: String,
}

/// Safely truncate a string to approximately `max_chars` characters
fn safe_truncate(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

/// Extract a context window around a match, operating on characters (not bytes)
fn extract_context(text: &str, query_lower: &str, context_chars: usize) -> String {
    let text_lower = text.to_lowercase();

    // Find match position in character indices
    let text_chars: Vec<char> = text.chars().collect();
    let lower_chars: Vec<char> = text_lower.chars().collect();
    let query_chars: Vec<char> = query_lower.chars().collect();
    let query_len = query_chars.len();

    // Search for the query in character array
    let match_pos = lower_chars
        .windows(query_len)
        .position(|w| w == query_chars.as_slice());

    match match_pos {
        Some(pos) => {
            let start = pos.saturating_sub(context_chars);
            let end = (pos + query_len + context_chars).min(text_chars.len());
            text_chars[start..end].iter().collect()
        }
        None => {
            // Fallback: return first N chars
            safe_truncate(text, context_chars * 2)
        }
    }
}

pub fn global_search(query: String, max_results: usize) -> Result<Vec<SearchResult>, String> {
    let files = scan_all_session_files();

    if files.is_empty() {
        return Ok(Vec::new());
    }

    let query_lower = query.to_lowercase();

    // Parallel search across all files
    let results: Vec<SearchResult> = files
        .par_iter()
        .flat_map(|file_path| {
            let mut file_results: Vec<SearchResult> = Vec::new();

            // Quick pre-check: does the file contain the query at all?
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => return file_results,
            };

            if !content.to_lowercase().contains(&query_lower) {
                return file_results;
            }

            // Get session metadata
            let meta = extract_session_meta(file_path);
            let (session_id, cwd) = match &meta {
                Some(m) => (m.id.clone(), m.cwd.clone()),
                None => {
                    let stem = file_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    (stem, String::new())
                }
            };
            let short_name = short_name_from_path(&cwd);

            // Parse and search through messages
            if let Ok(messages) = parse_all_messages(file_path) {
                let mut first_prompt = None;
                for msg in &messages {
                    if msg.role == "user" && first_prompt.is_none() {
                        for block in &msg.content {
                            if let DisplayContentBlock::Text { text } = block {
                                first_prompt = Some(safe_truncate(text, 100));
                                break;
                            }
                        }
                    }

                    for block in &msg.content {
                        let text = match block {
                            DisplayContentBlock::Text { text } => text,
                            DisplayContentBlock::Reasoning { text } => text,
                            DisplayContentBlock::FunctionCall {
                                arguments, ..
                            } => arguments,
                            DisplayContentBlock::FunctionCallOutput {
                                output, ..
                            } => output,
                            _ => continue,
                        };

                        if text.to_lowercase().contains(&query_lower) {
                            let matched_text = extract_context(text, &query_lower, 50);

                            file_results.push(SearchResult {
                                cwd: cwd.clone(),
                                short_name: short_name.clone(),
                                session_id: session_id.clone(),
                                first_prompt: first_prompt.clone(),
                                matched_text,
                                role: msg.role.clone(),
                                timestamp: msg.timestamp.clone(),
                                file_path: file_path.to_string_lossy().to_string(),
                            });

                            if file_results.len() >= 5 {
                                return file_results;
                            }
                        }
                    }
                }
            }

            file_results
        })
        .collect();

    let mut results = results;
    results.truncate(max_results);

    Ok(results)
}
