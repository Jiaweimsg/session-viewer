use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter};

use crate::claude::parser::path_encoder::get_projects_dir;
use crate::codex::parser::session_scanner::get_sessions_dir;

#[derive(Debug, Clone, Serialize)]
pub struct FsChangePayload {
    pub tool: String,
    pub paths: Vec<String>,
}

/// Start watching both Claude projects and Codex sessions directories for changes.
/// Emits "fs-change" events to the frontend when files are modified.
pub fn start_watcher(app_handle: AppHandle) -> Result<(), String> {
    let claude_dir = get_projects_dir();
    let codex_dir = get_sessions_dir();

    if claude_dir.is_none() && codex_dir.is_none() {
        return Err("Could not find any session directories to watch".to_string());
    }

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create watcher: {}", e);
                return;
            }
        };

        // Watch Claude projects directory if it exists
        if let Some(ref dir) = claude_dir {
            if dir.exists() {
                if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                    eprintln!("Failed to watch Claude projects directory: {}", e);
                }
            }
        }

        // Watch Codex sessions directory if it exists
        if let Some(ref dir) = codex_dir {
            if dir.exists() {
                if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                    eprintln!("Failed to watch Codex sessions directory: {}", e);
                }
            }
        }

        for event in rx {
            match event {
                Ok(event) => {
                    // Only emit for relevant file changes
                    let has_relevant_files = event.paths.iter().any(|p| {
                        p.extension()
                            .map(|e| e == "jsonl" || e == "json")
                            .unwrap_or(false)
                    });

                    if has_relevant_files {
                        // Determine which tool the change belongs to
                        let tool = determine_tool(&event.paths, &claude_dir, &codex_dir);

                        let paths: Vec<String> = event
                            .paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();

                        let payload = FsChangePayload { tool, paths };
                        let _ = app_handle.emit("fs-change", payload);
                    }
                }
                Err(e) => {
                    eprintln!("Watch error: {}", e);
                }
            }
        }
    });

    Ok(())
}

/// Determine which tool a set of changed paths belongs to
fn determine_tool(
    paths: &[std::path::PathBuf],
    claude_dir: &Option<std::path::PathBuf>,
    codex_dir: &Option<std::path::PathBuf>,
) -> String {
    for path in paths {
        let path_str = path.to_string_lossy();
        if let Some(ref dir) = claude_dir {
            if path_str.starts_with(&dir.to_string_lossy().to_string()) {
                return "claude".to_string();
            }
        }
        if let Some(ref dir) = codex_dir {
            if path_str.starts_with(&dir.to_string_lossy().to_string()) {
                return "codex".to_string();
            }
        }
    }
    "unknown".to_string()
}
