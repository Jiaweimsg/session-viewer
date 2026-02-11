use std::collections::HashMap;
use std::fs;

use crate::claude::models::stats::{DailyTokenEntry, StatsCache, TokenUsageSummary};
use crate::claude::parser::path_encoder::get_stats_cache_path;

pub fn get_global_stats() -> Result<StatsCache, String> {
    let path = get_stats_cache_path().ok_or("Could not find stats cache path")?;

    if !path.exists() {
        return Ok(StatsCache {
            version: None,
            last_computed_date: None,
            daily_activity: Vec::new(),
            daily_model_tokens: Vec::new(),
            model_usage: HashMap::new(),
        });
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read stats cache: {}", e))?;

    let stats: StatsCache =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse stats cache: {}", e))?;

    Ok(stats)
}

pub fn get_token_summary() -> Result<TokenUsageSummary, String> {
    let stats = get_global_stats()?;

    let mut total_tokens: u64 = 0;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_tokens: Vec<DailyTokenEntry> = Vec::new();

    // Compute total input/output from modelUsage for ratio calculation
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    for usage in stats.model_usage.values() {
        total_input_tokens +=
            usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens;
        total_output_tokens += usage.output_tokens;
    }

    // Compute input ratio for proportional daily split
    let global_total = total_input_tokens + total_output_tokens;
    let input_ratio = if global_total > 0 {
        total_input_tokens as f64 / global_total as f64
    } else {
        0.5
    };

    for day in &stats.daily_model_tokens {
        let mut day_total: u64 = 0;
        for (model, tokens) in &day.tokens_by_model {
            total_tokens += tokens;
            day_total += tokens;
            *tokens_by_model.entry(model.clone()).or_insert(0) += tokens;
        }

        // Distribute daily total into input/output using global ratio
        let day_input = (day_total as f64 * input_ratio) as u64;
        let day_output = day_total.saturating_sub(day_input);

        daily_tokens.push(DailyTokenEntry {
            date: day.date.clone(),
            input_tokens: day_input,
            output_tokens: day_output,
            total_tokens: day_total,
        });
    }

    Ok(TokenUsageSummary {
        total_input_tokens,
        total_output_tokens,
        total_tokens,
        tokens_by_model,
        daily_tokens,
    })
}
