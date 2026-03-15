use std::path::PathBuf;
use std::fs;

/// Returns the VS Code workspace storage root directory.
/// macOS: ~/Library/Application Support/Code/User/workspaceStorage
/// Linux: ~/.config/Code/User/workspaceStorage
/// Windows: %APPDATA%\Code\User\workspaceStorage
pub fn get_workspace_storage_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library/Application Support/Code/User/workspaceStorage")
        })
    }
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir().map(|c| c.join("Code/User/workspaceStorage"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir().map(|d| d.join("Code/User/workspaceStorage"))
    }
}

/// Extract the human-readable workspace path from a workspace.json file.
/// The file contains either `{"folder": "file:///path"}` or `{"workspace": "file:///path"}`.
pub fn get_workspace_path(workspace_storage: &PathBuf, hash: &str) -> Option<String> {
    let workspace_json = workspace_storage.join(hash).join("workspace.json");
    let content = fs::read_to_string(&workspace_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let uri = json
        .get("folder")
        .or_else(|| json.get("workspace"))
        .and_then(|v| v.as_str())?
        .to_string();

    // Decode percent-encoded URI: strip "file://" prefix, decode %20 etc.
    let path = uri
        .strip_prefix("file://")
        .unwrap_or(&uri)
        .to_string();

    // URL-decode
    let decoded = percent_decode(&path);
    Some(decoded)
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Ok(h), Ok(l)) = (
                std::str::from_utf8(&bytes[i + 1..i + 2]),
                std::str::from_utf8(&bytes[i + 2..i + 3]),
            ) {
                if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h, l), 16) {
                    result.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// List all workspace hashes that contain a chatSessions directory.
pub fn scan_workspace_hashes(workspace_storage: &PathBuf) -> Vec<String> {
    let mut hashes = Vec::new();

    if let Ok(entries) = fs::read_dir(workspace_storage) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let chat_sessions = path.join("chatSessions");
                if chat_sessions.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        hashes.push(name.to_string());
                    }
                }
            }
        }
    }

    hashes
}

/// List all chat session files (.json and .jsonl) for a workspace.
pub fn scan_session_files(workspace_storage: &PathBuf, workspace_hash: &str) -> Vec<PathBuf> {
    let chat_dir = workspace_storage.join(workspace_hash).join("chatSessions");
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(&chat_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "json" || ext == "jsonl" {
                        files.push(path);
                    }
                }
            }
        }
    }

    files
}

/// List all session files across all workspaces.
pub fn scan_all_session_files(workspace_storage: &PathBuf) -> Vec<PathBuf> {
    let hashes = scan_workspace_hashes(workspace_storage);
    let mut all = Vec::new();
    for hash in &hashes {
        all.extend(scan_session_files(workspace_storage, hash));
    }
    all
}

/// Extract the short name (last path component) from a workspace path string.
pub fn short_name_from_path(path: &str) -> String {
    // Strip trailing slashes
    let path = path.trim_end_matches('/').trim_end_matches('\\');
    PathBuf::from(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}
