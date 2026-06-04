use std::collections::{HashMap, HashSet};
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
    // Global dedup of (message.id, requestId) across ALL session files. Claude
    // Code splits one API response into multiple JSONL lines (one per content
    // block), each carrying the SAME cumulative `usage`; without this the same
    // request's tokens get summed once per block (~2x over-count). Global (not
    // per-file) so resume/history-copy duplicates across files also collapse.
    let mut seen: HashSet<(String, String)> = HashSet::new();

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
                &mut seen,
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
    seen: &mut HashSet<(String, String)>,
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
                    // Dedup by (message.id, requestId): Claude Code writes one
                    // line per content block of a single response, each with the
                    // same cumulative usage. Count each request once (keep-first).
                    // Lines missing either id are counted as-is (can't dedup).
                    let msg_id = msg.get("id").and_then(|x| x.as_str());
                    let req_id = v.get("requestId").and_then(|x| x.as_str());
                    if let (Some(mid), Some(rid)) = (msg_id, req_id) {
                        if !seen.insert((mid.to_string(), rid.to_string())) {
                            continue;
                        }
                    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn dedup_collapses_duplicate_content_block_lines() {
        // Three lines = one API response split across content blocks, all sharing
        // (message.id, requestId) and the SAME cumulative usage — exactly how
        // Claude Code writes a tool-using turn. A fourth line is a distinct request.
        let dup = r#"{"type":"assistant","timestamp":"2026-06-01T01:00:00.000Z","requestId":"req_1","message":{"id":"msg_1","model":"claude-opus-4-8","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":100,"cache_creation_input_tokens":20}}}"#;
        let other = r#"{"type":"assistant","timestamp":"2026-06-01T02:00:00.000Z","requestId":"req_2","message":{"id":"msg_2","model":"claude-opus-4-8","usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":2,"cache_creation_input_tokens":3}}}"#;

        let path = std::env::temp_dir().join(format!("sv_dedup_report_{}.jsonl", std::process::id()));
        {
            let mut f = fs::File::create(&path).unwrap();
            writeln!(f, "{}", dup).unwrap();
            writeln!(f, "{}", dup).unwrap();
            writeln!(f, "{}", dup).unwrap();
            writeln!(f, "{}", other).unwrap();
        }

        let mut agg = HashMap::new();
        let mut tracker = HashMap::new();
        let mut seen = HashSet::new();
        scan_session_for_report(&path, "proj", "sess1", &mut agg, &mut tracker, &mut seen);
        let _ = fs::remove_file(&path);

        let key = (
            "2026-06-01".to_string(),
            "proj".to_string(),
            "claude-opus-4-8".to_string(),
        );
        let v = agg.get(&key).expect("agg entry present");
        // req_1 counted once (10/5/100/20) + req_2 (1/1/2/3); NOT 3x req_1.
        assert_eq!(v.0, 11, "input deduped");
        assert_eq!(v.1, 6, "output deduped");
        assert_eq!(v.2, 102, "cache_read deduped");
        assert_eq!(v.3, 23, "cache_creation deduped");
        assert_eq!(v.4, 2, "message_count counts distinct requests, not block-lines");
    }
}
