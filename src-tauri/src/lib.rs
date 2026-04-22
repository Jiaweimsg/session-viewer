mod claude;
mod codex;
mod copilot;
mod cursor;
mod opencode;
mod commands;
mod report;
mod conversation;
mod shared_models;
mod state;
mod version_check;
mod watcher;

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DEFAULT_REPORT_SERVER: &str = "http://172.36.164.85:3000";
const REPORT_INITIAL_DELAY_SECS: u64 = 30;
const REPORT_INTERVAL_SECS: u64 = 300; // 5 minutes
const CONVERSATION_INITIAL_DELAY_SECS: u64 = 60;
const CONVERSATION_INTERVAL_SECS: u64 = 300;

/// Report server resolved at startup.
/// Env var `SESSION_VIEWER_REPORT_SERVER` overrides the default — useful
/// for local E2E smoke tests against a dev server on 127.0.0.1.
fn report_server() -> String {
    match std::env::var("SESSION_VIEWER_REPORT_SERVER") {
        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => DEFAULT_REPORT_SERVER.to_string(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_projects,
            commands::get_sessions,
            commands::get_sessions_grouped,
            commands::get_messages,
            commands::global_search,
            commands::get_stats,
            commands::get_token_summary,
            commands::get_advanced_stats,
            commands::report_usage,
            commands::resume_session,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // Start file system watcher in background
            if let Err(e) = watcher::fs_watcher::start_watcher(handle.clone()) {
                eprintln!("Warning: Failed to start file watcher: {}", e);
            }

            // Shared "uploads blocked" flag, flipped by version_check when the
            // server's min_client_version is newer than this build. Version check
            // runs at the top of every metrics cycle (5 min), self-healing if the
            // admin lowers the min version while the client stays running.
            let upload_blocked = Arc::new(AtomicBool::new(false));

            // Start auto-report in background (all tools)
            let report_flag = upload_blocked.clone();
            let report_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let server = report_server();
                eprintln!("[AutoReport] scheduled: first in {}s, then every {}s", REPORT_INITIAL_DELAY_SECS, REPORT_INTERVAL_SECS);
                tokio::time::sleep(std::time::Duration::from_secs(REPORT_INITIAL_DELAY_SECS)).await;
                loop {
                    // Re-check server's min_client_version each cycle.
                    version_check::enforce_min_version(&server, report_handle.clone(), report_flag.clone()).await;

                    if report_flag.load(Ordering::SeqCst) {
                        eprintln!("[AutoReport] skipped (client version blocked)");
                    } else {
                        eprintln!("[AutoReport] reporting all tools to {}", server);
                        match report::send_all_reports(&server).await {
                            Ok(total) => eprintln!("[AutoReport] success: {} total records", total),
                            Err(e) => eprintln!("[AutoReport] error: {}", e),
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(REPORT_INTERVAL_SECS)).await;
                }
            });

            // Start conversation collection loop (Claude Code only, independent of metrics)
            let conv_flag = upload_blocked.clone();
            tauri::async_runtime::spawn(async move {
                let server = report_server();
                eprintln!(
                    "[Conversation] scheduled: first in {}s, then every {}s",
                    CONVERSATION_INITIAL_DELAY_SECS, CONVERSATION_INTERVAL_SECS
                );
                tokio::time::sleep(std::time::Duration::from_secs(
                    CONVERSATION_INITIAL_DELAY_SECS,
                ))
                .await;
                loop {
                    if conv_flag.load(Ordering::SeqCst) {
                        eprintln!("[Conversation] skipped (client version blocked)");
                    } else {
                        eprintln!("[Conversation] scanning + uploading to {}", server);
                        match conversation::uploader::flush(&server, &["claude_code", "codex", "cursor"]).await {
                            Ok(n) => eprintln!("[Conversation] cycle ok: {} messages", n),
                            Err(e) => eprintln!("[Conversation] cycle failed: {}", e),
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(
                        CONVERSATION_INTERVAL_SECS,
                    ))
                    .await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
