use crate::conversation::RoleTag;
use crate::conversation::{ConversationMessage, state::ConversationState};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// 6 CLI-injected prefixes that mark a "user" message as NOT a real user prompt.
pub(crate) const SYSTEM_PREFIXES: &[&str] = &[
    "<local-command-caveat>",
    "<command-name>",
    "<local-command-stdout>",
    "<local-command-stderr>",
    "<system-reminder>",
    "<system-status>",
];

/// Extract the plain-text prompt from a user jsonl line.
/// Returns `None` if the message is a system-injection, tool result, or empty.
pub fn extract_user_text(v: &Value) -> Option<String> {
    if v.get("type")?.as_str()? != "user" {
        return None;
    }
    let content = v.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    let ty = item.get("type")?.as_str()?;
                    if ty == "text" {
                        Some(item.get("text")?.as_str()?.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            if parts.is_empty() {
                return None; // only tool_use/tool_result etc
            }
            parts.join("\n\n")
        }
        _ => return None,
    };

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    for prefix in SYSTEM_PREFIXES {
        if trimmed.starts_with(prefix) {
            return None;
        }
    }
    Some(text)
}

/// Case-insensitive retry patterns. Matched only when text length <= 30 chars.
fn is_retry_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.chars().count() > 30 {
        return false;
    }
    let lower = trimmed.to_lowercase();
    // Chinese patterns
    const ZH: &[&str] = &["再试", "重试", "不对", "继续", "换一个"];
    for p in ZH {
        if trimmed.starts_with(p) {
            return true;
        }
    }
    // English patterns (word-boundary approximation: must be followed by end / whitespace / punct)
    const EN: &[&str] = &["retry", "try again", "again", "no", "continue", "go on"];
    for p in EN {
        if lower.starts_with(p) {
            let rest = &lower[p.len()..];
            if rest.is_empty() || rest.starts_with(|c: char| !c.is_alphanumeric()) {
                return true;
            }
        }
    }
    false
}

/// Classify a user prompt within a freshly-scanned file window.
///
/// `is_fresh_scan` = true means `start_offset == 0` for this file, so we can
/// legitimately mark the first user prompt we see as `First`. On incremental
/// scans (offset > 0), every prompt defaults to `Followup` (or `Retry`) since
/// we cannot tell if the session's real first prompt was already emitted.
pub fn classify_role_tag(text: &str, is_first_in_window: bool, is_fresh_scan: bool) -> RoleTag {
    if is_retry_text(text) {
        return RoleTag::Retry;
    }
    if is_fresh_scan && is_first_in_window {
        return RoleTag::First;
    }
    RoleTag::Followup
}

/// Given a window of jsonl lines ordered by position, find the first
/// `type=assistant` line and return its `message.model`. Skips messages whose
/// model is `<synthetic>` or `unknown` (same rule as claude/commands/report.rs).
/// Returns `None` if no usable assistant is found.
pub fn lookup_following_model(window: &[Value]) -> Option<String> {
    for v in window {
        let Some(ty) = v.get("type").and_then(|x| x.as_str()) else { continue };
        if ty != "assistant" { continue; }
        let Some(model) = v.get("message").and_then(|m| m.get("model")).and_then(|x| x.as_str()) else { continue };
        if model == "<synthetic>" || model == "unknown" || model.is_empty() {
            continue;
        }
        return Some(model.to_string());
    }
    None
}

/// A scanned message annotated with its source file and the byte offset
/// *after* its line (i.e., where the next line would start). Used to advance
/// per-file high-water marks only after the containing batch succeeds.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub file: PathBuf,
    pub line_end: u64,
    pub message: ConversationMessage,
}

/// Scan a single jsonl file from `start_offset` to EOF, returning all
/// user-prompt messages in the window.
pub fn scan_one_file(
    path: &Path,
    start_offset: u64,
    file_size: u64,
) -> std::io::Result<Vec<PendingMessage>> {
    // Defensive: if offset exceeds size (file truncated/rotated), rescan from 0.
    let start_offset = if start_offset > file_size { 0 } else { start_offset };
    let is_fresh_scan = start_offset == 0;

    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start_offset))?;
    let mut reader = BufReader::new(file);

    // First pass: read lines with their post-line byte offset.
    let mut lines: Vec<(u64, Value)> = Vec::new();
    let mut cursor = start_offset;
    loop {
        let mut buf = String::new();
        let n = reader.read_line(&mut buf)?;
        if n == 0 {
            break;
        }
        cursor += n as u64;
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            lines.push((cursor, v));
        }
    }

    // Second pass: project metadata from cwd, tag + model backfill.
    let mut first_emitted = false;
    let mut results = Vec::new();
    for (i, (line_end, v)) in lines.iter().enumerate() {
        let Some(text) = extract_user_text(v) else { continue };

        let uuid = v.get("uuid").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if uuid.is_empty() {
            continue;
        }
        let session_id = v.get("sessionId").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let parent_uuid = v.get("parentUuid").and_then(|x| x.as_str()).map(String::from);
        let timestamp = v.get("timestamp").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let cwd = v.get("cwd").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let git_branch = v.get("gitBranch").and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        let project = cwd
            .rsplit(|c: char| c == '/' || c == '\\')
            .find(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string();

        let tail: Vec<Value> = lines[i + 1..].iter().map(|(_, v)| v.clone()).collect();
        let model = lookup_following_model(&tail);

        let is_first_in_window = !first_emitted;
        let role_tag = classify_role_tag(&text, is_first_in_window, is_fresh_scan);
        if role_tag == RoleTag::First {
            first_emitted = true;
        }

        results.push(PendingMessage {
            file: path.to_path_buf(),
            line_end: *line_end,
            message: ConversationMessage {
                uuid,
                session_id,
                parent_uuid,
                timestamp,
                project,
                cwd,
                git_branch,
                model,
                role_tag,
                text,
            },
        });
    }

    Ok(results)
}

/// Walk `~/.claude/projects/**/*.jsonl` and scan each incrementally.
pub fn scan_all(state: &ConversationState) -> Vec<PendingMessage> {
    let Some(projects_dir) = crate::claude::parser::path_encoder::get_projects_dir() else {
        return Vec::new();
    };
    if !projects_dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return Vec::new() };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&project_dir) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let start = state.offset_for(&p);
                let Ok(meta) = std::fs::metadata(&p) else { continue };
                let size = meta.len();
                if start >= size {
                    // Nothing new; skip without opening the file.
                    continue;
                }
                match scan_one_file(&p, start, size) {
                    Ok(mut v) => out.append(&mut v),
                    Err(e) => eprintln!("[Conversation] scan failed for {:?}: {}", p, e),
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod text_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_string_content() {
        let v = json!({"type": "user", "message": {"content": "hello"}});
        assert_eq!(extract_user_text(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn array_text_segments_joined() {
        let v = json!({
            "type": "user",
            "message": {"content": [
                {"type": "text", "text": "part1"},
                {"type": "text", "text": "part2"}
            ]}
        });
        assert_eq!(extract_user_text(&v).as_deref(), Some("part1\n\npart2"));
    }

    #[test]
    fn array_with_only_tool_result_returns_none() {
        let v = json!({
            "type": "user",
            "message": {"content": [
                {"type": "tool_result", "tool_use_id": "x", "content": "ok"}
            ]}
        });
        assert_eq!(extract_user_text(&v), None);
    }

    #[test]
    fn six_system_prefixes_filtered() {
        for prefix in SYSTEM_PREFIXES {
            let text = format!("{}something", prefix);
            let v = json!({"type": "user", "message": {"content": text}});
            assert_eq!(extract_user_text(&v), None, "should filter prefix: {}", prefix);
        }
    }

    #[test]
    fn empty_whitespace_returns_none() {
        let v = json!({"type": "user", "message": {"content": "   \n  "}});
        assert_eq!(extract_user_text(&v), None);
    }

    #[test]
    fn non_user_type_returns_none() {
        let v = json!({"type": "assistant", "message": {"content": "hi"}});
        assert_eq!(extract_user_text(&v), None);
    }

    #[test]
    fn missing_content_returns_none() {
        let v = json!({"type": "user", "message": {}});
        assert_eq!(extract_user_text(&v), None);
    }
}

#[cfg(test)]
mod role_tests {
    use super::*;

    #[test]
    fn retry_chinese() {
        assert!(is_retry_text("再试一下"));
        assert!(is_retry_text("重试"));
        assert!(is_retry_text("不对"));
        assert!(is_retry_text("继续"));
        assert!(is_retry_text("换一个方案"));
    }

    #[test]
    fn retry_english_case_insensitive() {
        assert!(is_retry_text("Retry"));
        assert!(is_retry_text("try again please"));
        assert!(is_retry_text("no"));
        assert!(is_retry_text("NO, try X"));
        assert!(is_retry_text("continue"));
        assert!(is_retry_text("go on"));
    }

    #[test]
    fn not_retry_when_too_long() {
        let long = "retry ".repeat(10); // 60 chars
        assert!(!is_retry_text(&long));
    }

    #[test]
    fn not_retry_mid_sentence() {
        // Word starts must match at beginning of text. "please retry" starts with "please".
        assert!(!is_retry_text("please retry"));
    }

    #[test]
    fn not_retry_false_prefix_match() {
        // "again" should match but "against" should not (word boundary).
        assert!(is_retry_text("again!"));
        assert!(!is_retry_text("against the wall"));
    }

    #[test]
    fn first_only_on_fresh_scan() {
        assert_eq!(classify_role_tag("how do I do X", true, true), RoleTag::First);
        assert_eq!(classify_role_tag("how do I do X", true, false), RoleTag::Followup);
        assert_eq!(classify_role_tag("how do I do X", false, true), RoleTag::Followup);
    }

    #[test]
    fn retry_beats_first() {
        assert_eq!(classify_role_tag("重试", true, true), RoleTag::Retry);
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_first_assistant_model() {
        let w = vec![
            json!({"type": "user", "message": {"content": "x"}}),
            json!({"type": "assistant", "message": {"model": "claude-opus-4-6"}}),
        ];
        assert_eq!(lookup_following_model(&w).as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn skips_synthetic() {
        let w = vec![
            json!({"type": "assistant", "message": {"model": "<synthetic>"}}),
            json!({"type": "assistant", "message": {"model": "claude-sonnet-4-5"}}),
        ];
        assert_eq!(lookup_following_model(&w).as_deref(), Some("claude-sonnet-4-5"));
    }

    #[test]
    fn skips_unknown() {
        let w = vec![
            json!({"type": "assistant", "message": {"model": "unknown"}}),
            json!({"type": "assistant", "message": {"model": "claude-opus-4-6"}}),
        ];
        assert_eq!(lookup_following_model(&w).as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn returns_none_when_no_assistant() {
        let w = vec![
            json!({"type": "user", "message": {"content": "x"}}),
            json!({"type": "user", "message": {"content": "y"}}),
        ];
        assert_eq!(lookup_following_model(&w), None);
    }

    #[test]
    fn returns_none_when_only_synthetic() {
        let w = vec![
            json!({"type": "assistant", "message": {"model": "<synthetic>"}}),
        ];
        assert_eq!(lookup_following_model(&w), None);
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for l in lines {
            writeln!(f, "{}", l).unwrap();
        }
        f.flush().unwrap();
        f
    }

    #[test]
    fn fresh_scan_marks_first() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"hello"}}"#,
            r#"{"type":"assistant","uuid":"a1","message":{"model":"claude-opus-4-6"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"more"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].message.role_tag, RoleTag::First);
        assert_eq!(result[0].message.project, "proj");
        assert_eq!(result[0].message.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(result[1].message.role_tag, RoleTag::Followup);
    }

    #[test]
    fn incremental_scan_no_first() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"hello"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"more"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        // Scan from the start of the 2nd line by asking scanner to resume at size/2
        let result = scan_one_file(f.path(), size / 2, size).unwrap();
        assert!(!result.is_empty());
        // No `First` should be emitted because this is not a fresh scan.
        assert!(result.iter().all(|m| m.message.role_tag != RoleTag::First));
    }

    #[test]
    fn system_messages_filtered() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"<system-reminder>ignore me"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"real prompt"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.uuid, "u2");
    }

    #[test]
    fn truncated_file_resets_offset() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"hi"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        // Pretend we had offset larger than current size (file was shrunk).
        let result = scan_one_file(f.path(), size + 9999, size).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.role_tag, RoleTag::First);
    }

    #[test]
    fn line_end_offset_monotonic() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"one"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"two"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].line_end < result[1].line_end);
        assert_eq!(result[1].line_end, size);
    }
}
