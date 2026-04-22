use serde::Serialize;
use std::collections::{HashMap, HashSet};
use crate::cursor::parser::project_scanner::{
    read_composer_headers, read_bubbles, epoch_ms_to_rfc3339,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorStats {
    pub total_sessions: usize,
    pub total_projects: usize,
    pub total_messages: usize,
    pub total_requests: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub daily_activity: Vec<CursorDailyActivity>,
    pub daily_tokens: Vec<CursorDailyTokenEntry>,
    pub mode_distribution: Vec<ModeEntry>,
    pub model_usage: Vec<ModelUsageEntry>,
    pub project_ranking: Vec<CursorProjectTokenEntry>,
    pub efficiency: CursorSessionEfficiency,
    pub active_sessions: usize,
    pub archived_sessions: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorDailyActivity {
    pub date: String,
    pub message_count: u64,
    pub session_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorDailyTokenEntry {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeEntry {
    pub mode: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageEntry {
    pub model: String,
    pub request_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorProjectTokenEntry {
    pub project_name: String,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub session_count: usize,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSessionEfficiency {
    pub avg_messages_per_session: f64,
    pub avg_tokens_per_session: f64,
    pub total_sessions: u64,
    pub total_messages: u64,
    pub distribution: Vec<EfficiencyBucket>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyBucket {
    pub label: String,
    pub count: u64,
}

/// Extract date (YYYY-MM-DD) from epoch milliseconds
fn date_from_epoch_ms(ms: u64) -> Option<String> {
    let rfc = epoch_ms_to_rfc3339(ms);
    if rfc.len() >= 10 {
        Some(rfc[..10].to_string())
    } else {
        None
    }
}

/// Extract short project name from workspace path
fn short_name(path: &str) -> String {
    path.rsplit('/').next()
        .or_else(|| path.rsplit('\\').next())
        .unwrap_or(path)
        .to_string()
}

pub fn get_stats() -> Result<CursorStats, String> {
    let headers = read_composer_headers();
    if headers.is_empty() {
        return Ok(CursorStats {
            total_sessions: 0,
            total_projects: 0,
            total_messages: 0,
            total_requests: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            daily_activity: Vec::new(),
            daily_tokens: Vec::new(),
            mode_distribution: Vec::new(),
            model_usage: Vec::new(),
            project_ranking: Vec::new(),
            efficiency: CursorSessionEfficiency {
                avg_messages_per_session: 0.0,
                avg_tokens_per_session: 0.0,
                total_sessions: 0,
                total_messages: 0,
                distribution: Vec::new(),
            },
            active_sessions: 0,
            archived_sessions: 0,
        });
    }

    let unique_projects: HashSet<_> = headers
        .iter()
        .map(|h| h.workspace_path.as_deref().unwrap_or("(no workspace)"))
        .collect();

    // Mode distribution
    let mut mode_counts: HashMap<String, usize> = HashMap::new();
    for h in &headers {
        let mode = h.unified_mode.as_deref().unwrap_or("unknown").to_string();
        *mode_counts.entry(mode).or_insert(0) += 1;
    }
    let mut mode_distribution: Vec<ModeEntry> = mode_counts
        .into_iter()
        .map(|(mode, count)| ModeEntry { mode, count })
        .collect();
    mode_distribution.sort_by(|a, b| b.count.cmp(&a.count));

    // Archived vs active
    let archived_sessions = headers.iter().filter(|h| h.is_archived).count();
    let active_sessions = headers.len() - archived_sessions;

    // Per-session scanning: tokens, daily activity, project stats
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut total_messages: usize = 0;
    let mut total_requests: usize = 0;

    // model_name -> request_count (from user messages)
    let mut model_counts: HashMap<String, usize> = HashMap::new();

    // daily_date -> (messages, sessions_set)
    let mut daily_messages: HashMap<String, u64> = HashMap::new();
    let mut daily_sessions: HashMap<String, HashSet<String>> = HashMap::new();
    // daily_date -> (input, output)
    let mut daily_token_map: HashMap<String, (u64, u64)> = HashMap::new();
    // project -> (input, output, session_count, message_count)
    let mut project_stats: HashMap<String, (u64, u64, usize, usize)> = HashMap::new();
    // per-session message counts for efficiency
    let mut session_msg_counts: Vec<u64> = Vec::new();
    let mut session_token_totals: Vec<u64> = Vec::new();

    // Sessions that also have a transcript file; those are authoritative for
    // prompts so we skip message/session counting from the bubble side to
    // avoid double-counting. Token aggregation still runs from bubbles.
    let transcript_session_ids =
        crate::cursor::parser::agent_transcripts::collect_transcript_session_ids();
    let bubble_session_count = headers.len();
    let overlap = headers
        .iter()
        .filter(|h| transcript_session_ids.contains(&h.composer_id))
        .count();

    for header in &headers {
        let has_transcript = transcript_session_ids.contains(&header.composer_id);
        let bubbles = read_bubbles(&header.composer_id);
        let msg_count = bubbles.len();
        if !has_transcript {
            total_messages += msg_count;
        }

        let mut session_input: u64 = 0;
        let mut session_output: u64 = 0;

        // Session date from header
        let session_date = header.created_at.and_then(|ms| date_from_epoch_ms(ms));

        // Register session in daily activity (skip overlap — transcript loop handles it)
        if !has_transcript {
            if let Some(ref date) = session_date {
                daily_sessions
                    .entry(date.clone())
                    .or_default()
                    .insert(header.composer_id.clone());
            }
        }

        for bubble in &bubbles {
            // Token aggregation (always — token_count lives only in bubbles)
            if let Some(ref tc) = bubble.token_count {
                session_input += tc.input_tokens;
                session_output += tc.output_tokens;

                // Try to get date from bubble, fallback to session date
                let bubble_date = bubble.created_at.as_deref()
                    .and_then(|s| if s.len() >= 10 { Some(s[..10].to_string()) } else { None })
                    .or_else(|| session_date.clone());

                if let Some(date) = bubble_date {
                    let entry = daily_token_map.entry(date).or_insert((0, 0));
                    entry.0 += tc.input_tokens;
                    entry.1 += tc.output_tokens;
                }
            }

            // Daily message count - try bubble date, fallback to session date
            // Skip when transcript is authoritative.
            if !has_transcript {
                let msg_date = bubble.created_at.as_deref()
                    .and_then(|s| if s.len() >= 10 { Some(s[..10].to_string()) } else { None })
                    .or_else(|| session_date.clone());

                if let Some(date) = msg_date {
                    *daily_messages.entry(date).or_insert(0) += 1;
                }
            }

            // Count user requests and model usage (type 1 = user message = one request)
            if bubble.msg_type == 1 {
                total_requests += 1;
                if let Some(ref model) = bubble.model_name {
                    if !model.is_empty() {
                        *model_counts.entry(model.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        total_input_tokens += session_input;
        total_output_tokens += session_output;

        // Project stats — tokens always, but session/message counts skip when
        // transcript side will handle them.
        let project_key = header.workspace_path.as_deref().unwrap_or("(no workspace)").to_string();
        let proj = project_stats.entry(project_key).or_insert((0, 0, 0, 0));
        proj.0 += session_input;
        proj.1 += session_output;
        if !has_transcript {
            proj.2 += 1;
            proj.3 += msg_count;
        }

        // Efficiency tracking — only non-overlap bubbles (transcript loop
        // appends its own msg_count for overlapping sessions).
        if !has_transcript {
            session_msg_counts.push(msg_count as u64);
        }
        session_token_totals.push(session_input + session_output);
    }

    // ── Also ingest Cursor Agent transcripts (new jsonl format) ──
    // These don't carry token counts, so we only bump message/session counts.
    let transcript_files = crate::cursor::parser::agent_transcripts::scan_all_transcript_files();
    for tpath in &transcript_files {
        let Some(tmeta) = crate::cursor::parser::agent_transcripts::extract_transcript_meta(tpath) else { continue };
        let msg_count = crate::cursor::parser::agent_transcripts::count_user_messages(tpath);
        if msg_count == 0 { continue; }

        total_messages += msg_count as usize;
        let date = crate::cursor::parser::agent_transcripts::date_from_epoch_ms(tmeta.file_mtime_ms);
        if let Some(date) = date {
            *daily_messages.entry(date.clone()).or_insert(0) += msg_count;
            daily_sessions.entry(date).or_default().insert(tmeta.session_id.clone());
        }

        // Project aggregation — workspace_encoded serves as the project name.
        let project = tmeta.workspace_encoded.clone();
        let entry = project_stats.entry(project).or_insert((0, 0, 0, 0));
        entry.2 += 1;            // session_count
        entry.3 += msg_count as usize; // message_count

        session_msg_counts.push(msg_count);
    }
    let transcript_session_count = transcript_files.len();

    // Build daily activity (sorted)
    let mut all_dates: HashSet<String> = HashSet::new();
    all_dates.extend(daily_messages.keys().cloned());
    all_dates.extend(daily_sessions.keys().cloned());

    let mut daily_activity: Vec<CursorDailyActivity> = all_dates
        .iter()
        .map(|date| CursorDailyActivity {
            date: date.clone(),
            message_count: daily_messages.get(date).copied().unwrap_or(0),
            session_count: daily_sessions.get(date).map(|s| s.len() as u64).unwrap_or(0),
        })
        .collect();
    daily_activity.sort_by(|a, b| a.date.cmp(&b.date));

    // Build daily tokens (sorted)
    let mut daily_tokens: Vec<CursorDailyTokenEntry> = daily_token_map
        .into_iter()
        .map(|(date, (input, output))| CursorDailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    // Build project ranking (top 10, sorted by total tokens)
    let mut project_ranking: Vec<CursorProjectTokenEntry> = project_stats
        .into_iter()
        .map(|(path, (input, output, sc, mc))| CursorProjectTokenEntry {
            project_name: short_name(&path),
            total_tokens: input + output,
            input_tokens: input,
            output_tokens: output,
            session_count: sc,
            message_count: mc,
        })
        .collect();
    project_ranking.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    project_ranking.truncate(10);

    // Build session efficiency
    let total_sess = session_msg_counts.len() as u64;
    let total_msgs: u64 = session_msg_counts.iter().sum();
    let total_tok: u64 = session_token_totals.iter().sum();
    let avg_messages = if total_sess > 0 {
        (total_msgs as f64 / total_sess as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };
    let avg_tokens = if total_sess > 0 {
        (total_tok as f64 / total_sess as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let buckets = [
        ("1-5", 1u64, 5u64),
        ("6-10", 6, 10),
        ("11-20", 11, 20),
        ("21-50", 21, 50),
        ("50+", 51, u64::MAX),
    ];
    let distribution: Vec<EfficiencyBucket> = buckets
        .iter()
        .map(|(label, lo, hi)| {
            let count = session_msg_counts
                .iter()
                .filter(|&&c| c >= *lo && c <= *hi)
                .count() as u64;
            EfficiencyBucket {
                label: label.to_string(),
                count,
            }
        })
        .collect();

    // Build model usage (sorted by count desc)
    let mut model_usage: Vec<ModelUsageEntry> = model_counts
        .into_iter()
        .map(|(model, request_count)| ModelUsageEntry { model, request_count })
        .collect();
    model_usage.sort_by(|a, b| b.request_count.cmp(&a.request_count));

    Ok(CursorStats {
        total_sessions: bubble_session_count - overlap + transcript_session_count,
        total_projects: unique_projects.len(),
        total_messages,
        total_requests,
        total_input_tokens,
        total_output_tokens,
        total_tokens: total_input_tokens + total_output_tokens,
        daily_activity,
        daily_tokens,
        mode_distribution,
        model_usage,
        project_ranking,
        efficiency: CursorSessionEfficiency {
            avg_messages_per_session: avg_messages,
            avg_tokens_per_session: avg_tokens,
            total_sessions: total_sess,
            total_messages: total_msgs,
            distribution,
        },
        active_sessions,
        archived_sessions,
    })
}
