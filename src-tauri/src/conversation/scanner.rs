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
