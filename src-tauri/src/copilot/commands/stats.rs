use std::collections::HashMap;

use crate::copilot::models::stats::{CopilotDailyTokenEntry, CopilotTokenSummary};
use crate::copilot::parser::session_parser::parse_session_file;
use crate::copilot::parser::session_scanner::{
    get_workspace_storage_dir, scan_workspace_hashes, scan_session_files,
};

pub fn get_stats() -> Result<CopilotTokenSummary, String> {
    let storage_dir = get_workspace_storage_dir()
        .ok_or_else(|| "Could not determine VS Code workspace storage directory".to_string())?;

    let hashes = scan_workspace_hashes(&storage_dir);

    let mut session_count = 0usize;
    let mut message_count = 0usize;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_map: HashMap<String, (u64, u64)> = HashMap::new();

    for hash in &hashes {
        let session_files = scan_session_files(&storage_dir, hash);
        session_count += session_files.len();

        for path in session_files {
            if let Ok(parsed) = parse_session_file(&path) {
                message_count += parsed.requests.len();

                for req in &parsed.requests {
                    total_input_tokens += req.input_tokens;
                    total_output_tokens += req.output_tokens;
                    let total = req.input_tokens + req.output_tokens;

                    // Attribute tokens to model
                    if let Some(ref mid) = req.model_id {
                        *tokens_by_model.entry(mid.clone()).or_insert(0) += total;
                    }

                    // Daily aggregation using request timestamp
                    if req.timestamp_ms > 0 {
                        if let Some(dt) = chrono::DateTime::from_timestamp((req.timestamp_ms / 1000) as i64, 0) {
                            let date = dt.format("%Y-%m-%d").to_string();
                            let entry = daily_map.entry(date).or_insert((0, 0));
                            entry.0 += req.input_tokens;
                            entry.1 += req.output_tokens;
                        }
                    }
                }
            }
        }
    }

    let mut daily_tokens: Vec<CopilotDailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, (input, output))| CopilotDailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        })
        .collect();

    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(CopilotTokenSummary {
        total_input_tokens,
        total_output_tokens,
        total_tokens: total_input_tokens + total_output_tokens,
        tokens_by_model,
        daily_tokens,
        session_count,
        message_count,
    })
}
