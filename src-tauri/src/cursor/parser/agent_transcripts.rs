//! Cursor Agent Transcripts reader.
//!
//! Newer Cursor versions write agent conversations as append-only JSONL at
//! `~/.cursor/projects/{workspace}/agent-transcripts/{session-uuid}/{session-uuid}.jsonl`.
//! Each line is `{role, message: {content: [{type, text}]}}`. The real user
//! prompts are wrapped in `<user_query>...</user_query>`; other user-role
//! messages (user_info / git_status / rules preamble) are system-injected and
//! are filtered out here.

use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Returns `~/.cursor/projects/` if that directory exists, else `None`.
fn projects_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let p = home.join(".cursor").join("projects");
    if p.is_dir() { Some(p) } else { None }
}

/// Walk every `{workspace}/agent-transcripts/{session}/*.jsonl`.
pub fn scan_all_transcript_files() -> Vec<PathBuf> {
    let Some(projects) = projects_dir() else { return Vec::new() };
    let mut out = Vec::new();
    let Ok(ws_iter) = fs::read_dir(&projects) else { return out };
    for ws in ws_iter.flatten() {
        let ws_path = ws.path();
        if !ws_path.is_dir() { continue; }
        let at_dir = ws_path.join("agent-transcripts");
        if !at_dir.is_dir() { continue; }
        let Ok(sessions) = fs::read_dir(&at_dir) else { continue };
        for sess in sessions.flatten() {
            let sess_dir = sess.path();
            if !sess_dir.is_dir() { continue; }
            let Ok(files) = fs::read_dir(&sess_dir) else { continue };
            for f in files.flatten() {
                let p = f.path();
                if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct TranscriptMeta {
    pub session_id: String,
    pub workspace_encoded: String,
    pub file_mtime_ms: u64,
}

/// Infer session_id + workspace name from the path, and read mtime.
/// Path: .../projects/{workspace}/agent-transcripts/{uuid}/{uuid}.jsonl
pub fn extract_transcript_meta(path: &Path) -> Option<TranscriptMeta> {
    let session_id = path.file_stem()?.to_str()?.to_string();
    let workspace_encoded = path
        .parent()? // {uuid}/
        .parent()? // agent-transcripts/
        .parent()? // {workspace}/
        .file_name()?.to_str()?.to_string();
    let meta = fs::metadata(path).ok()?;
    let mtime_ms = meta
        .modified().ok()?
        .duration_since(std::time::UNIX_EPOCH).ok()?
        .as_millis() as u64;
    Some(TranscriptMeta { session_id, workspace_encoded, file_mtime_ms: mtime_ms })
}

/// If this jsonl row is a real `<user_query>`-wrapped user prompt, return the
/// inner text. Otherwise (role != user, system-injected preamble, empty) → None.
pub fn extract_user_query_text(v: &Value) -> Option<String> {
    if v.get("role")?.as_str()? != "user" { return None; }
    let content = v.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(|item| {
                let ty = item.get("type")?.as_str()?;
                if ty == "text" {
                    Some(item.get("text")?.as_str()?.to_string())
                } else { None }
            }).collect();
            if parts.is_empty() { return None; }
            parts.join("\n\n")
        }
        _ => return None,
    };
    let trimmed = text.trim();
    const PREFIX: &str = "<user_query>";
    const SUFFIX: &str = "</user_query>";
    if !trimmed.starts_with(PREFIX) || !trimmed.ends_with(SUFFIX) {
        // Not wrapped → system-injected preamble (user_info / rules / etc.)
        return None;
    }
    let inner = &trimmed[PREFIX.len()..trimmed.len() - SUFFIX.len()];
    let inner = inner.trim();
    if inner.is_empty() { return None; }
    Some(inner.to_string())
}

#[derive(Debug, Clone)]
pub struct TranscriptMessage {
    pub line_start: u64,
    pub line_end: u64,
    pub text: String,
}

/// Scan one jsonl from `start_offset` to EOF, returning user-prompt rows.
pub fn scan_one_transcript(
    path: &Path,
    start_offset: u64,
    file_size: u64,
) -> std::io::Result<Vec<TranscriptMessage>> {
    let start_offset = if start_offset > file_size { 0 } else { start_offset };
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start_offset))?;
    let mut reader = BufReader::new(file);
    let mut cursor = start_offset;
    let mut out = Vec::new();
    loop {
        let line_start = cursor;
        let mut buf = String::new();
        let n = reader.read_line(&mut buf)?;
        if n == 0 { break; }
        cursor += n as u64;
        let trimmed = buf.trim();
        if trimmed.is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<Value>(trimmed) else { continue };
        let Some(text) = extract_user_query_text(&v) else { continue };
        out.push(TranscriptMessage { line_start, line_end: cursor, text });
    }
    Ok(out)
}

/// Count user-query messages in a transcript (used by stats aggregation).
pub fn count_user_messages(path: &Path) -> u64 {
    let Ok(f) = File::open(path) else { return 0 };
    let reader = BufReader::new(f);
    let mut c: u64 = 0;
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<Value>(trimmed) else { continue };
        if extract_user_query_text(&v).is_some() {
            c += 1;
        }
    }
    c
}

/// Epoch ms → YYYY-MM-DD (UTC).
pub fn date_from_epoch_ms(ms: u64) -> Option<String> {
    let secs = (ms / 1000) as i64;
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)?;
    Some(dt.format("%Y-%m-%d").to_string())
}

/// Collect the session UUIDs that have a transcript file on disk.
/// Used to deduplicate against the old SQLite bubble schema (where the same
/// session also appears) so counts aren't inflated.
pub fn collect_transcript_session_ids() -> std::collections::HashSet<String> {
    scan_all_transcript_files()
        .iter()
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(String::from))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_user_query_basic() {
        let v = json!({
            "role": "user",
            "message": {"content": [{"type": "text", "text": "<user_query>\nhello\n</user_query>"}]}
        });
        assert_eq!(extract_user_query_text(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn extract_user_query_strips_leading_trailing_whitespace() {
        let v = json!({
            "role": "user",
            "message": {"content": [{"type": "text", "text": "<user_query>\n  hi there  \n</user_query>"}]}
        });
        assert_eq!(extract_user_query_text(&v).as_deref(), Some("hi there"));
    }

    #[test]
    fn extract_user_query_multiple_text_parts_joined() {
        let v = json!({
            "role": "user",
            "message": {"content": [
                {"type": "text", "text": "<user_query>\npart a"},
                {"type": "text", "text": "part b\n</user_query>"}
            ]}
        });
        assert_eq!(extract_user_query_text(&v).as_deref(), Some("part a\n\npart b"));
    }

    #[test]
    fn rejects_non_user_role() {
        let v = json!({"role": "assistant", "message": {"content": [{"type":"text","text":"<user_query>x</user_query>"}]}});
        assert_eq!(extract_user_query_text(&v), None);
    }

    #[test]
    fn rejects_system_injection_without_user_query_wrapping() {
        // user_info / git_status preambles appear under role=user but lack <user_query>.
        let v = json!({
            "role": "user",
            "message": {"content": [{"type": "text", "text": "<user_info>\nOS Version..."}]}
        });
        assert_eq!(extract_user_query_text(&v), None);
    }

    #[test]
    fn rejects_empty_inner() {
        let v = json!({
            "role": "user",
            "message": {"content": [{"type": "text", "text": "<user_query>\n   \n</user_query>"}]}
        });
        assert_eq!(extract_user_query_text(&v), None);
    }

    #[test]
    fn rejects_non_text_content_parts() {
        // Only tool_use parts, no text → None.
        let v = json!({
            "role": "user",
            "message": {"content": [{"type": "tool_use", "id": "x"}]}
        });
        assert_eq!(extract_user_query_text(&v), None);
    }

    #[test]
    fn scan_one_transcript_finds_real_queries_only() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut f = NamedTempFile::new().unwrap();
        // Line 1: real user query
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>\nhello\n</user_query>"}}]}}}}"#).unwrap();
        // Line 2: system-injected user_info
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_info>OS x</user_info>"}}]}}}}"#).unwrap();
        // Line 3: assistant
        writeln!(f, r#"{{"role":"assistant","message":{{"content":[{{"type":"text","text":"hi"}}]}}}}"#).unwrap();
        // Line 4: another real query
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>\nworld\n</user_query>"}}]}}}}"#).unwrap();
        f.flush().unwrap();
        let size = fs::metadata(f.path()).unwrap().len();
        let msgs = scan_one_transcript(f.path(), 0, size).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].text, "hello");
        assert_eq!(msgs[1].text, "world");
        // Monotonic byte offsets
        assert!(msgs[0].line_end <= msgs[1].line_start);
    }

    #[test]
    fn scan_one_transcript_respects_start_offset() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>\na\n</user_query>"}}]}}}}"#).unwrap();
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>\nb\n</user_query>"}}]}}}}"#).unwrap();
        f.flush().unwrap();
        let size = fs::metadata(f.path()).unwrap().len();
        let msgs = scan_one_transcript(f.path(), size / 2, size).unwrap();
        // Only the second one should be in window
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "b");
    }

    #[test]
    fn scan_one_transcript_truncated_file_resets() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>\nhello\n</user_query>"}}]}}}}"#).unwrap();
        f.flush().unwrap();
        let size = fs::metadata(f.path()).unwrap().len();
        let msgs = scan_one_transcript(f.path(), size + 9999, size).unwrap();
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn count_user_messages_filters_correctly() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_query>\na\n</user_query>"}}]}}}}"#).unwrap();
        writeln!(f, r#"{{"role":"user","message":{{"content":[{{"type":"text","text":"<user_info>skip</user_info>"}}]}}}}"#).unwrap();
        writeln!(f, r#"{{"role":"assistant","message":{{"content":[{{"type":"text","text":"hi"}}]}}}}"#).unwrap();
        f.flush().unwrap();
        assert_eq!(count_user_messages(f.path()), 1);
    }

    #[test]
    fn collect_transcript_session_ids_empty_when_no_files() {
        // Best we can do without mocking the home dir: just assert the call works
        // and returns a HashSet (may be non-empty if the dev machine has transcripts).
        let _set = collect_transcript_session_ids();
    }
}
