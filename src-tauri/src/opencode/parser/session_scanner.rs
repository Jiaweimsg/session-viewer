use std::path::PathBuf;
use std::fs;

/// Get OpenCode storage directory.
/// Tries XDG-style ~/.local/share/opencode/storage first (Linux/macOS/WSL),
/// then falls back to the platform data dir (%APPDATA%\opencode\storage on Windows).
pub fn get_storage_dir() -> Option<PathBuf> {
    for candidate in candidate_storage_dirs() {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn candidate_storage_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("share").join("opencode").join("storage"));
    }
    if let Some(data) = dirs::data_dir() {
        candidates.push(data.join("opencode").join("storage"));
    }
    if let Some(data_local) = dirs::data_local_dir() {
        candidates.push(data_local.join("opencode").join("storage"));
    }
    candidates
}

/// Get project directory
pub fn get_project_dir() -> Option<PathBuf> {
    get_storage_dir().map(|p| p.join("project"))
}

/// Get session directory
pub fn get_session_dir() -> Option<PathBuf> {
    get_storage_dir().map(|p| p.join("session"))
}

/// Get message directory
pub fn get_message_dir() -> Option<PathBuf> {
    get_storage_dir().map(|p| p.join("message"))
}

/// Scan all project hashes (subdirectories in session/)
pub fn scan_project_hashes() -> Vec<String> {
    let session_dir = match get_session_dir() {
        Some(dir) => dir,
        None => return vec![],
    };
    
    let mut hashes = Vec::new();
    
    if let Ok(entries) = fs::read_dir(session_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name != "global" {
                        hashes.push(name.to_string());
                    }
                }
            }
        }
    }
    
    hashes
}

/// Scan session files for a specific project hash
pub fn scan_session_files(project_hash: &str) -> Vec<PathBuf> {
    let session_dir = match get_session_dir() {
        Some(dir) => dir.join(project_hash),
        None => return vec![],
    };
    
    let mut session_files = Vec::new();
    
    if let Ok(entries) = fs::read_dir(session_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                session_files.push(path);
            }
        }
    }
    
    session_files
}

/// Scan all session files across all projects
pub fn scan_all_session_files() -> Vec<PathBuf> {
    let project_hashes = scan_project_hashes();
    let mut all_sessions = Vec::new();
    
    for hash in project_hashes {
        all_sessions.extend(scan_session_files(&hash));
    }
    
    all_sessions
}

/// Get short name from a path (last component)
pub fn short_name_from_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}
