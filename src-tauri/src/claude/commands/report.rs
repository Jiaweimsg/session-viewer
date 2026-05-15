use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::claude::parser::path_encoder::{
    get_projects_dir, list_session_jsonl_files, short_name_from_path,
};
use crate::report::UsageRecord;

/// Scan all Claude sessions and aggregate usage by (date, project, model)
pub fn collect_usage_records() -> Result<Vec<UsageRecord>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    type AggKey = (String, String, String);
    let mut agg: HashMap<AggKey, (u64, u64, u64, u64, u64, u64)> = HashMap::new();
    let mut session_tracker: HashMap<(String, String), std::collections::HashSet<String>> =
        HashMap::new();

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let project_name = resolve_project_name(&path);

        for file_path in list_session_jsonl_files(&path) {
            let session_id = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            scan_session_for_report(
                &file_path,
                &project_name,
                &session_id,
                &mut agg,
                &mut session_tracker,
            );
        }
    }

    let records: Vec<UsageRecord> = agg
        .into_iter()
        .map(|((date, project, model), (input, output, cache_read, cache_creation, msg_count, _))| {
            let session_count = session_tracker
                .get(&(date.clone(), project.clone()))
                .map(|s| s.len() as u64)
                .unwrap_or(0);

            UsageRecord {
                date,
                project,
                model,
                input_tokens: input,
                output_tokens: output,
                cache_read_tokens: cache_read,
                cache_creation_tokens: cache_creation,
                session_count,
                message_count: msg_count,
            }
        })
        .collect();

    Ok(records)
}

#[allow(clippy::type_complexity)]
fn scan_session_for_report(
    path: &Path,
    project_name: &str,
    session_id: &str,
    agg: &mut HashMap<(String, String, String), (u64, u64, u64, u64, u64, u64)>,
    session_tracker: &mut HashMap<(String, String), std::collections::HashSet<String>>,
) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let record_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        let date = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|ts| ts.get(..10))
            .map(|d| d.to_string());

        let date = match date {
            Some(d) => d,
            None => continue,
        };

        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                let model = msg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown");

                if model == "<synthetic>" || model == "unknown" {
                    continue;
                }

                if let Some(usage) = msg.get("usage") {
                    let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_read = usage
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let cache_creation = usage
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    let key = (date.clone(), project_name.to_string(), model.to_string());
                    let entry = agg.entry(key).or_insert((0, 0, 0, 0, 0, 0));
                    entry.0 += input;
                    entry.1 += output;
                    entry.2 += cache_read;
                    entry.3 += cache_creation;
                    entry.4 += 1;
                    entry.5 += 1;

                    session_tracker
                        .entry((date, project_name.to_string()))
                        .or_default()
                        .insert(session_id.to_string());
                }
            }
        }
    }
}

fn resolve_project_name(project_dir: &Path) -> String {
    for file_path in list_session_jsonl_files(project_dir) {
        let file = match fs::File::open(&file_path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let reader = BufReader::new(file);
        for line in reader.lines().take(10) {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
                    if !cwd.is_empty() {
                        return short_name_from_path(cwd);
                    }
                }
            }
        }
    }
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}
