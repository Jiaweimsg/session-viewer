use std::fs;
use std::path::PathBuf;

/// Get the Codex home directory (~/.codex)
pub fn get_codex_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

/// Get the Codex sessions directory (~/.codex/sessions)
pub fn get_sessions_dir() -> Option<PathBuf> {
    get_codex_home().map(|h| h.join("sessions"))
}

/// Scan all JSONL files under ~/.codex/sessions/<year>/<month>/<day>/*.jsonl
pub fn scan_all_session_files() -> Vec<PathBuf> {
    let sessions_dir = match get_sessions_dir() {
        Some(d) if d.exists() => d,
        _ => return Vec::new(),
    };

    let mut files: Vec<PathBuf> = Vec::new();

    // Iterate year directories
    let year_dirs = match fs::read_dir(&sessions_dir) {
        Ok(d) => d,
        Err(_) => return files,
    };

    for year_entry in year_dirs.flatten() {
        let year_path = year_entry.path();
        if !year_path.is_dir() {
            continue;
        }

        // Iterate month directories
        let month_dirs = match fs::read_dir(&year_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for month_entry in month_dirs.flatten() {
            let month_path = month_entry.path();
            if !month_path.is_dir() {
                continue;
            }

            // Iterate day directories
            let day_dirs = match fs::read_dir(&month_path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for day_entry in day_dirs.flatten() {
                let day_path = day_entry.path();
                if !day_path.is_dir() {
                    continue;
                }

                // Collect .jsonl files
                let jsonl_files = match fs::read_dir(&day_path) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                for file_entry in jsonl_files.flatten() {
                    let file_path = file_entry.path();
                    if file_path
                        .extension()
                        .map(|e| e == "jsonl")
                        .unwrap_or(false)
                    {
                        files.push(file_path);
                    }
                }
            }
        }
    }

    files
}

/// Extract the short name (last path segment) from a full path
pub fn short_name_from_path(path: &str) -> String {
    let path = path.trim_end_matches(['/', '\\']);
    if let Some(pos) = path.rfind(['/', '\\']) {
        path[pos + 1..].to_string()
    } else {
        path.to_string()
    }
}

/// Parse date from a file path like .../2025/01/15/rollout-xxx.jsonl
/// Returns "2025-01-15"
pub fn extract_date_from_path(path: &PathBuf) -> Option<String> {
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Look for pattern: sessions / year / month / day / file.jsonl
    let len = components.len();
    if len >= 4 {
        let day = components[len - 2];
        let month = components[len - 3];
        let year = components[len - 4];

        // Validate that they look like date components
        if year.len() == 4
            && year.chars().all(|c| c.is_ascii_digit())
            && month.len() <= 2
            && month.chars().all(|c| c.is_ascii_digit())
            && day.len() <= 2
            && day.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!(
                "{}-{:0>2}-{:0>2}",
                year, month, day
            ));
        }
    }
    None
}

/// Extract session ID (UUID) from filename like rollout-1234567890-abcdef.jsonl
#[allow(dead_code)]
pub fn extract_session_id_from_filename(path: &PathBuf) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    // filename format: rollout-<timestamp>-<uuid>
    // We want the UUID part
    if stem.starts_with("rollout-") {
        let rest = &stem["rollout-".len()..];
        // Find the first '-' which separates timestamp from UUID
        if let Some(pos) = rest.find('-') {
            return Some(rest[pos + 1..].to_string());
        }
    }
    // Fallback: use the whole stem
    Some(stem.to_string())
}
