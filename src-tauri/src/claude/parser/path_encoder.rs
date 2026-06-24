use std::path::{Path, PathBuf};

/// Get the Claude home directory (~/.claude)
pub fn get_claude_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Get the Claude projects directory (~/.claude/projects)
pub fn get_projects_dir() -> Option<PathBuf> {
    get_claude_home().map(|h| h.join("projects"))
}

/// Expand a leading `~` / `~/` to the user's home directory.
fn expand_tilde(p: &str) -> PathBuf {
    if p == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(p));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

/// 所有要扫描的 Claude projects 目录:默认 `~/.claude/projects` 永远第一位,
/// 后接配置文件 `scan-dirs.json` 里列出的额外目录(多账号 config 场景)。
///
/// 额外目录智能识别:展开 `~`,若 `dir/projects` 是目录则用它(填的是账号根
/// 目录,如 `~/.claude-cc-bin`),否则用 `dir` 本身(直接填了 projects 目录)。
/// 结果按 canonicalize 去重,跳过不存在的额外目录。
pub fn get_all_projects_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    // 默认 ~/.claude/projects 永远第一位(即使不存在也保留,由调用方各自处理)
    if let Some(def) = get_projects_dir() {
        let key = def.canonicalize().unwrap_or_else(|_| def.clone());
        if seen.insert(key) {
            out.push(def);
        }
    }

    // 配置文件里的额外目录
    for raw in crate::scan_dirs::load().paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let base = expand_tilde(trimmed);
        // 智能识别:优先 base/projects,否则 base 本身
        let sub = base.join("projects");
        let candidate = if sub.is_dir() { sub } else { base };
        if !candidate.is_dir() {
            continue;
        }
        let key = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        if seen.insert(key) {
            out.push(candidate);
        }
    }

    out
}

/// Get the stats cache file path (~/.claude/stats-cache.json)
pub fn get_stats_cache_path() -> Option<PathBuf> {
    get_claude_home().map(|h| h.join("stats-cache.json"))
}

/// Collect every `.jsonl` session file under one project dir, including the
/// `subagents/agent-*.jsonl` files Claude writes for Agent/Task tool calls.
///
/// Without recursion, subagent runs (which often hold the *largest*
/// `cache_creation_input_tokens` chunks per session — the spawned context is
/// rehydrated as a fresh 5m cache) get silently dropped, under-reporting any
/// user that relies on subagents.
pub fn list_session_jsonl_files(project_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_jsonl(project_dir, &mut out);
    out
}

fn walk_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_jsonl(&path, out);
        } else if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Decode an encoded project directory name back to a path
/// e.g. "C--Users-zuolan-Desktop-LB" -> "C:\Users\zuolan\Desktop\LB" (on Windows)
/// The encoding replaces path separators with '-' and ':' with '-'
pub fn decode_project_path(encoded: &str) -> String {
    // The encoding scheme used by Claude:
    // - Path separators (/ or \) are replaced with '-'
    // - Drive colon (C:) becomes "C-"
    // So "C--Users-zuolan-Desktop-LB" means "C:\Users\zuolan\Desktop\LB" on Windows
    // And "-Users-zuolan-Desktop-LB" means "/Users/zuolan/Desktop/LB" on Unix

    if cfg!(windows) {
        // Windows drive pattern: "C--Users-foo-bar" where [A-Za-z] + '-' is drive + ':'
        let first = encoded.chars().next();
        let is_drive_encoded = encoded.len() >= 2
            && encoded.chars().nth(1) == Some('-')
            && first.map(|c| c.is_ascii_alphabetic()).unwrap_or(false);

        if is_drive_encoded {
            let drive = &encoded[0..1];
            let rest = &encoded[2..]; // skip "C-"
            let path_part = rest.replace('-', "\\");
            format!("{}:{}", drive, path_part)
        } else {
            // Unix-style encoded path (e.g. WSL: "-mnt-c-proj"). Leading '-' maps to '/'.
            encoded.replace('-', "/")
        }
    } else {
        // On Unix, pattern is like "-Users-zuolan-Desktop-LB" -> "/Users/zuolan/Desktop/LB"
        encoded.replace('-', "/")
    }
}

/// Extract the last path segment as a short name
pub fn short_name_from_path(path: &str) -> String {
    let path = path.trim_end_matches(['/', '\\']);
    if let Some(pos) = path.rfind(['/', '\\']) {
        path[pos + 1..].to_string()
    } else {
        path.to_string()
    }
}
