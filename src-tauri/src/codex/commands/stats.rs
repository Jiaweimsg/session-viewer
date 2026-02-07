use std::collections::HashMap;

use crate::codex::models::stats::{DailyTokenEntry, TokenUsageSummary};
use crate::codex::parser::jsonl::{count_messages, extract_session_meta, extract_token_info};
use crate::codex::parser::session_scanner::{extract_date_from_path, scan_all_session_files};

pub fn get_stats() -> Result<TokenUsageSummary, String> {
    let files = scan_all_session_files();

    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_map: HashMap<String, (u64, u64, u64)> = HashMap::new();
    let mut session_count: u64 = 0;
    let mut message_count: u64 = 0;

    for file_path in &files {
        session_count += 1;
        message_count += count_messages(file_path) as u64;

        // Get model info
        let model_provider = extract_session_meta(file_path)
            .and_then(|m| m.model_provider)
            .unwrap_or_else(|| "unknown".to_string());

        // Get token info
        if let Some(token_info) = extract_token_info(file_path) {
            total_input_tokens += token_info.input_tokens;
            total_output_tokens += token_info.output_tokens;
            total_tokens += token_info.total_tokens;

            *tokens_by_model.entry(model_provider).or_insert(0) += token_info.total_tokens;

            // Aggregate by date
            if let Some(date) = extract_date_from_path(file_path) {
                let entry = daily_map.entry(date).or_insert((0, 0, 0));
                entry.0 += token_info.input_tokens;
                entry.1 += token_info.output_tokens;
                entry.2 += token_info.total_tokens;
            }
        }
    }

    // Convert daily map to sorted vec
    let mut daily_tokens: Vec<DailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, (input, output, total))| DailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(TokenUsageSummary {
        total_input_tokens,
        total_output_tokens,
        total_tokens,
        tokens_by_model,
        daily_tokens,
        session_count,
        message_count,
    })
}
