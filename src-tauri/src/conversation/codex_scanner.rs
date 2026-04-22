//! Codex conversation scanner.
//!
//! Walks `~/.codex/sessions/**/*.jsonl` (via crate::codex::parser::session_scanner),
//! extracts real user prompts, filters Codex system messages (role-based + context
//! block prefix), backfills model from the following `turn_context` row, and
//! synthesizes a stable uuid per message as `{session_id}_{line_start_offset}`.

use crate::codex::parser::jsonl::extract_session_meta;
use crate::codex::parser::session_scanner::{scan_all_session_files, short_name_from_path};
use crate::conversation::scanner::{classify_role_tag, PendingMessage};
use crate::conversation::state::ConversationState;
use crate::conversation::{ConversationMessage, RoleTag};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

/// Matches any `<xxx_context>` opening tag at the start (after whitespace).
/// Covers `<environment_context>`, `<task_context>`, etc.
fn is_context_block(text: &str) -> bool {
    let t = text.trim_start();
    if !t.starts_with('<') {
        return false;
    }
    // Find the first '>' after the opening '<'.
    let rest = &t[1..];
    let Some(end) = rest.find('>') else {
        return false;
    };
    let tag = &rest[..end];
    // tag must end with "_context" and consist of [a-z_]
    if !tag.ends_with("_context") {
        return false;
    }
    tag.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}

/// Matches Codex's ALL-CAPS XML-style injection tags like `<INSTRUCTIONS>`,
/// `<SYSTEM>`, etc. Appear at the very start of a role=user message whose
/// content is actually a project-wide instruction preamble.
fn is_allcaps_tag_start(text: &str) -> bool {
    let t = text.trim_start();
    let Some(rest) = t.strip_prefix('<') else {
        return false;
    };
    let Some(end) = rest.find('>') else {
        return false;
    };
    let tag = &rest[..end];
    if tag.is_empty() {
        return false;
    }
    tag.chars().all(|c| c.is_ascii_uppercase() || c == '_')
}

/// Matches Codex's Markdown AGENTS.md preamble, e.g.
/// `# AGENTS.md instructions for /Users/bin\n\n<INSTRUCTIONS>\n...`.
fn is_agents_md_preamble(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("# AGENTS.md") || t.starts_with("# AGENT.md")
}

/// Combined check: is this Codex-injected boilerplate (not a real user prompt)?
fn is_system_injection(text: &str) -> bool {
    is_context_block(text) || is_allcaps_tag_start(text) || is_agents_md_preamble(text)
}

/// Extract plain text from a Codex `response_item` row IF it's a real user prompt.
/// Returns None for role=system/developer, context-block injections, or empty text.
pub fn extract_codex_user_text(v: &Value) -> Option<String> {
    // Must be a response_item with payload.type == "message" and role == "user".
    if v.get("type")?.as_str()? != "response_item" {
        return None;
    }
    let payload = v.get("payload")?;
    if payload.get("type")?.as_str()? != "message" {
        return None;
    }
    let role = payload.get("role")?.as_str()?;
    if role != "user" {
        return None;
    }

    let content = payload.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    let ty = item.get("type")?.as_str()?;
                    // Codex uses "input_text" for user text, "output_text" for assistant.
                    if ty == "input_text" || ty == "text" {
                        Some(item.get("text")?.as_str()?.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            if parts.is_empty() {
                return None;
            }
            parts.join("\n\n")
        }
        _ => return None,
    };

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_system_injection(trimmed) {
        return None;
    }
    Some(text)
}

/// Walk a window of subsequent lines, returning the first turn_context.payload.model.
pub fn lookup_turn_context_model(window: &[Value]) -> Option<String> {
    for v in window {
        let Some(ty) = v.get("type").and_then(|x| x.as_str()) else {
            continue;
        };
        if ty != "turn_context" {
            continue;
        }
        let Some(model) = v
            .get("payload")
            .and_then(|p| p.get("model"))
            .and_then(|x| x.as_str())
        else {
            continue;
        };
        if model.is_empty() {
            continue;
        }
        return Some(model.to_string());
    }
    None
}

/// Synthesize a stable uuid for a Codex user message (no native uuid exists).
pub fn codex_uuid(session_id: &str, line_start_offset: u64) -> String {
    format!("{}_{}", session_id, line_start_offset)
}

/// Scan a single Codex JSONL file from `start_offset` to EOF.
/// Requires reading the file header (first ~5 lines) to extract session_meta,
/// which is cheap and we do it every time regardless of start_offset.
pub fn scan_one_file(
    path: &Path,
    start_offset: u64,
    file_size: u64,
) -> std::io::Result<Vec<PendingMessage>> {
    let start_offset = if start_offset > file_size { 0 } else { start_offset };
    let is_fresh_scan = start_offset == 0;

    // Grab session_meta from the file header (independent seek).
    let meta = extract_session_meta(path);
    let Some(meta) = meta else {
        return Ok(Vec::new());
    };
    let session_id = meta.id.clone();
    let cwd = meta.cwd.clone();
    let project = short_name_from_path(&cwd);
    let git_branch = meta.git_branch.clone().filter(|s| !s.is_empty());

    // Stream from start_offset; for each line, track its *start* offset and *end* offset.
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start_offset))?;
    let mut reader = BufReader::new(file);

    let mut lines: Vec<(u64, u64, Value)> = Vec::new(); // (line_start, line_end, parsed)
    let mut cursor = start_offset;
    loop {
        let line_start = cursor;
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
            lines.push((line_start, cursor, v));
        }
    }

    let mut first_emitted = false;
    let mut results = Vec::new();
    for (i, (line_start, line_end, v)) in lines.iter().enumerate() {
        let Some(text) = extract_codex_user_text(v) else {
            continue;
        };

        let timestamp = v
            .get("timestamp")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if timestamp.is_empty() {
            // Without a timestamp the server can't bucket by date; skip.
            continue;
        }

        let tail: Vec<Value> = lines[i + 1..]
            .iter()
            .map(|(_, _, val)| val.clone())
            .collect();
        let model = lookup_turn_context_model(&tail);

        let is_first_in_window = !first_emitted;
        let role_tag = classify_role_tag(&text, is_first_in_window, is_fresh_scan);
        if role_tag == RoleTag::First {
            first_emitted = true;
        }

        let uuid = codex_uuid(&session_id, *line_start);

        results.push(PendingMessage {
            file: path.to_path_buf(),
            line_end: *line_end,
            message: ConversationMessage {
                uuid,
                session_id: session_id.clone(),
                parent_uuid: None,
                timestamp,
                project: project.clone(),
                cwd: cwd.clone(),
                git_branch: git_branch.clone(),
                model,
                role_tag,
                text,
            },
        });
    }

    Ok(results)
}

/// Walk all Codex session files incrementally (per-file byte offset).
pub fn scan_all(state: &ConversationState) -> Vec<PendingMessage> {
    let files = scan_all_session_files();
    let mut out = Vec::new();
    for p in files {
        let start = state.offset_for(&p);
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        };
        let size = meta.len();
        if start >= size {
            continue;
        }
        match scan_one_file(&p, start, size) {
            Ok(mut v) => out.append(&mut v),
            Err(e) => eprintln!("[Conversation/codex] scan failed for {:?}: {}", p, e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn is_context_block_matches_codex_tags() {
        assert!(is_context_block("<environment_context>\n  <cwd>/foo</cwd>"));
        assert!(is_context_block("<task_context>x</task_context>"));
        assert!(is_context_block("   <environment_context>...")); // leading ws
    }

    #[test]
    fn is_context_block_rejects_regular_text() {
        assert!(!is_context_block("hello <environment_context> friend")); // not first
        assert!(!is_context_block("<html>not context"));
        assert!(!is_context_block("<ENV_CONTEXT>")); // not lowercase (handled by allcaps check instead)
        assert!(!is_context_block("regular text"));
        assert!(!is_context_block(""));
    }

    #[test]
    fn is_allcaps_tag_start_matches() {
        assert!(is_allcaps_tag_start("<INSTRUCTIONS>\n# rules"));
        assert!(is_allcaps_tag_start("<SYSTEM>x</SYSTEM>"));
        assert!(is_allcaps_tag_start("   <NOTES>preamble"));
    }

    #[test]
    fn is_allcaps_tag_start_rejects() {
        assert!(!is_allcaps_tag_start("<lowercase>"));
        assert!(!is_allcaps_tag_start("not a tag"));
        assert!(!is_allcaps_tag_start("<>"));
        assert!(!is_allcaps_tag_start("<Mixed>"));
    }

    #[test]
    fn is_agents_md_preamble_matches() {
        assert!(is_agents_md_preamble("# AGENTS.md instructions for /Users/bin"));
        assert!(is_agents_md_preamble("   # AGENTS.md\n"));
        assert!(is_agents_md_preamble("# AGENT.md preamble"));
    }

    #[test]
    fn is_agents_md_preamble_rejects() {
        assert!(!is_agents_md_preamble("# random heading"));
        assert!(!is_agents_md_preamble("help me with AGENTS.md"));
        assert!(!is_agents_md_preamble(""));
    }

    #[test]
    fn is_system_injection_combines_all_checks() {
        assert!(is_system_injection("<environment_context>..."));
        assert!(is_system_injection("<INSTRUCTIONS>..."));
        assert!(is_system_injection("# AGENTS.md instructions for /x"));
        assert!(!is_system_injection("how do I write an AGENTS.md?"));
        assert!(!is_system_injection("real question"));
    }

    #[test]
    fn extract_user_text_basic() {
        let v = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}]
            }
        });
        assert_eq!(extract_codex_user_text(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn extract_user_text_rejects_developer_and_system() {
        for role in &["developer", "system", "assistant"] {
            let v = json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": role,
                    "content": [{"type": "input_text", "text": "hi"}]
                }
            });
            assert_eq!(
                extract_codex_user_text(&v),
                None,
                "role {} should be filtered",
                role
            );
        }
    }

    #[test]
    fn extract_user_text_rejects_context_block() {
        let v = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "<environment_context>\n  <cwd>/x</cwd>"}]
            }
        });
        assert_eq!(extract_codex_user_text(&v), None);
    }

    #[test]
    fn extract_user_text_rejects_non_response_item() {
        let v = json!({"type": "event_msg", "payload": {"role": "user", "content": "x"}});
        assert_eq!(extract_codex_user_text(&v), None);
    }

    #[test]
    fn extract_user_text_joins_multiple_input_text_segments() {
        let v = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "a"},
                    {"type": "input_text", "text": "b"}
                ]
            }
        });
        assert_eq!(extract_codex_user_text(&v).as_deref(), Some("a\n\nb"));
    }

    #[test]
    fn lookup_turn_context_model_finds_first() {
        let w = vec![
            json!({"type": "event_msg", "payload": {}}),
            json!({"type": "turn_context", "payload": {"model": "gpt-5.1-codex-max"}}),
            json!({"type": "turn_context", "payload": {"model": "other"}}),
        ];
        assert_eq!(
            lookup_turn_context_model(&w).as_deref(),
            Some("gpt-5.1-codex-max")
        );
    }

    #[test]
    fn lookup_turn_context_model_skips_empty() {
        let w = vec![
            json!({"type": "turn_context", "payload": {"model": ""}}),
            json!({"type": "turn_context", "payload": {"model": "real"}}),
        ];
        assert_eq!(lookup_turn_context_model(&w).as_deref(), Some("real"));
    }

    #[test]
    fn lookup_turn_context_model_none_when_missing() {
        let w = vec![json!({"type": "event_msg", "payload": {}})];
        assert_eq!(lookup_turn_context_model(&w), None);
    }

    #[test]
    fn codex_uuid_composes_stably() {
        assert_eq!(codex_uuid("sess-123", 4567), "sess-123_4567");
    }

    // Integration test on a synthetic codex jsonl file.
    #[test]
    fn scan_one_file_fresh_extracts_real_user_prompts() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        // Line 1: session_meta
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:00Z","type":"session_meta","payload":{{"id":"sess-abc","cwd":"/Users/bin/IdeaProjects/demo","cli_version":"0.64.0","model_provider":"liao"}}}}"#).unwrap();
        // Line 2: env context injection (user role, should be filtered)
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:01Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"<environment_context>\n  <cwd>/x</cwd>"}}]}}}}"#).unwrap();
        // Line 3: real user prompt
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:02Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"Real question here"}}]}}}}"#).unwrap();
        // Line 4: turn_context (gives us model)
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:03Z","type":"turn_context","payload":{{"model":"gpt-5.1-codex-max"}}}}"#).unwrap();
        // Line 5: developer message (should be filtered)
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:04Z","type":"response_item","payload":{{"type":"message","role":"developer","content":[{{"type":"input_text","text":"system thing"}}]}}}}"#).unwrap();
        // Line 6: second real user prompt
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:05Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"follow up"}}]}}}}"#).unwrap();
        f.flush().unwrap();

        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();

        assert_eq!(result.len(), 2, "expected 2 real user prompts after filtering");
        assert_eq!(result[0].message.text, "Real question here");
        assert_eq!(result[0].message.role_tag, RoleTag::First);
        assert_eq!(result[0].message.session_id, "sess-abc");
        assert_eq!(result[0].message.project, "demo");
        assert_eq!(result[0].message.model.as_deref(), Some("gpt-5.1-codex-max"));
        assert!(result[0].message.uuid.starts_with("sess-abc_"));
        assert_eq!(result[1].message.text, "follow up");
        assert_eq!(result[1].message.role_tag, RoleTag::Followup);
    }

    #[test]
    fn scan_one_file_incremental_no_first_tag() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:00Z","type":"session_meta","payload":{{"id":"sess-abc","cwd":"/a/demo"}}}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:01Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"first"}}]}}}}"#).unwrap();
        writeln!(f, r#"{{"timestamp":"2026-04-22T10:00:02Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"second"}}]}}}}"#).unwrap();
        f.flush().unwrap();

        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), size / 2, size).unwrap();
        // Not fresh scan — nothing should be tagged First
        assert!(result.iter().all(|p| p.message.role_tag != RoleTag::First));
    }

    #[test]
    fn scan_one_file_without_session_meta_returns_empty() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"x"}}]}}}}"#).unwrap();
        f.flush().unwrap();
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert!(result.is_empty(), "no session_meta → scan returns empty");
    }
}
