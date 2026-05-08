use std::collections::{HashMap, HashSet};

use crate::cursor::commands::stats::{
    CursorDailyActivity, CursorDailyTokenEntry, CursorProjectTokenEntry,
    CursorSessionEfficiency, CursorStats, EfficiencyBucket, ModeEntry, ModelUsageEntry,
};
use crate::cursor::parser::cli_chats;

pub fn get_stats() -> Result<CursorStats, String> {
    let sessions = cli_chats::load_all_sessions();

    if sessions.is_empty() {
        return Ok(empty_stats());
    }

    let mut total_messages: usize = 0;
    let mut total_requests: usize = 0;
    let mut visible_sessions: usize = 0;
    let mut unique_projects: HashSet<String> = HashSet::new();

    let mut mode_counts: HashMap<String, usize> = HashMap::new();
    let mut daily_messages: HashMap<String, u64> = HashMap::new();
    let mut daily_sessions: HashMap<String, HashSet<String>> = HashMap::new();
    // project -> (input, output, session_count, message_count) — tokens always 0 for CLI
    let mut project_stats: HashMap<String, (u64, u64, usize, usize)> = HashMap::new();
    let mut session_msg_counts: Vec<u64> = Vec::new();

    for session in &sessions {
        let display_count = session.message_count();
        let prompt_rows = session.user_prompt_rows_after(0);
        if display_count == 0 && prompt_rows.is_empty() {
            continue;
        }
        visible_sessions += 1;

        total_messages += display_count;
        total_requests += prompt_rows.len();

        let cwd = session.cwd();
        unique_projects.insert(cwd.clone());

        let mode = session.meta.mode.as_deref().unwrap_or("unknown").to_string();
        *mode_counts.entry(mode).or_insert(0) += 1;

        if let Some(date) = session.modified().and_then(|s| {
            if s.len() >= 10 {
                Some(s[..10].to_string())
            } else {
                None
            }
        }) {
            *daily_messages.entry(date.clone()).or_insert(0) += display_count as u64;
            daily_sessions
                .entry(date)
                .or_default()
                .insert(session.session_id.clone());
        }

        let proj = project_stats.entry(cwd).or_insert((0, 0, 0, 0));
        proj.2 += 1;
        proj.3 += display_count;
        session_msg_counts.push(display_count as u64);
    }

    // Mode distribution
    let mut mode_distribution: Vec<ModeEntry> = mode_counts
        .into_iter()
        .map(|(mode, count)| ModeEntry { mode, count })
        .collect();
    mode_distribution.sort_by_key(|e| std::cmp::Reverse(e.count));

    // Daily activity (sorted)
    let mut all_dates: HashSet<String> = HashSet::new();
    all_dates.extend(daily_messages.keys().cloned());
    all_dates.extend(daily_sessions.keys().cloned());
    let mut date_list: Vec<String> = all_dates.into_iter().collect();
    date_list.sort();

    let daily_activity: Vec<CursorDailyActivity> = date_list
        .iter()
        .map(|date| CursorDailyActivity {
            date: date.clone(),
            message_count: daily_messages.get(date).copied().unwrap_or(0),
            session_count: daily_sessions.get(date).map(|s| s.len() as u64).unwrap_or(0),
        })
        .collect();

    // Project ranking — sorted by message_count desc since tokens are 0
    let mut project_ranking: Vec<CursorProjectTokenEntry> = project_stats
        .into_iter()
        .map(|(project_path, (input, output, session_count, message_count))| {
            CursorProjectTokenEntry {
                project_name: crate::shared_models::basename(&project_path),
                total_tokens: input + output,
                input_tokens: input,
                output_tokens: output,
                session_count,
                message_count,
            }
        })
        .collect();
    project_ranking.sort_by(|a, b| b.message_count.cmp(&a.message_count));

    // Efficiency buckets (by message count per session)
    let total_sess = session_msg_counts.len() as u64;
    let total_msgs: u64 = session_msg_counts.iter().sum();
    let avg_messages = if total_sess == 0 {
        0.0
    } else {
        total_msgs as f64 / total_sess as f64
    };
    let mut buckets = vec![
        EfficiencyBucket { label: "0".to_string(), count: 0 },
        EfficiencyBucket { label: "1-5".to_string(), count: 0 },
        EfficiencyBucket { label: "6-20".to_string(), count: 0 },
        EfficiencyBucket { label: "21-50".to_string(), count: 0 },
        EfficiencyBucket { label: "50+".to_string(), count: 0 },
    ];
    for n in &session_msg_counts {
        let idx = match *n {
            0 => 0,
            1..=5 => 1,
            6..=20 => 2,
            21..=50 => 3,
            _ => 4,
        };
        buckets[idx].count += 1;
    }

    Ok(CursorStats {
        total_sessions: visible_sessions,
        total_projects: unique_projects.len(),
        total_messages,
        total_requests,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_tokens: 0,
        daily_activity,
        daily_tokens: Vec::<CursorDailyTokenEntry>::new(),
        mode_distribution,
        model_usage: Vec::<ModelUsageEntry>::new(),
        project_ranking,
        efficiency: CursorSessionEfficiency {
            avg_messages_per_session: avg_messages,
            avg_tokens_per_session: 0.0,
            total_sessions: total_sess,
            total_messages: total_msgs,
            distribution: buckets,
        },
        active_sessions: visible_sessions,
        archived_sessions: 0,
    })
}

fn empty_stats() -> CursorStats {
    CursorStats {
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
    }
}
