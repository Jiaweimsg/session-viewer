use crate::conversation::RoleTag;
use serde_json::Value;

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
