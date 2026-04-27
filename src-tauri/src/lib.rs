mod blocklist;
mod claude;
mod codex;
mod copilot;
mod cursor;
mod identity;
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
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

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
    // Windows: 抑制系统级"应用程序无法正常启动"对话框
    // 比如部分用户的 git.exe 安装不全 (0xC0000142 / SEM_FAILCRITICALERRORS)，
    // 子进程继承父进程的 error mode —— git spawn 失败时只返回错误，不再弹窗。
    #[cfg(windows)]
    suppress_windows_error_dialogs();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
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
            commands::get_upload_blocklist,
            commands::set_upload_blocklist,
            commands::get_identity_view,
            commands::get_identity_override,
            commands::set_identity_override,
        ])
        .on_window_event(|window, event| {
            // 关窗到托盘：拦截关闭请求，隐藏窗口而不是退出进程。
            // 用户从托盘菜单 Quit 才真正退出。这样上报循环在 app
            // "关闭"后仍能跑。macOS 上单独按 Cmd+Q 仍可正常退出（不会进入这条路径）。
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // 默认开启开机自启 —— 用户安装后无需进设置即可享受后台上报。
            // 只在首次/未启用时主动 enable，避免反复写 LaunchAgent / 注册表。
            // 失败仅记日志，不阻塞启动。
            let autolaunch = app.autolaunch();
            match autolaunch.is_enabled() {
                Ok(false) => {
                    if let Err(e) = autolaunch.enable() {
                        eprintln!("[Autostart] enable failed: {}", e);
                    } else {
                        eprintln!("[Autostart] enabled by default");
                    }
                }
                Ok(true) => {}
                Err(e) => eprintln!("[Autostart] is_enabled query failed: {}", e),
            }

            // ── Tray icon ──────────────────────────────────────
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let report_item =
                MenuItem::with_id(app, "report_now", "立即上报", true, None::<&str>)?;
            let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &report_item, &separator, &quit_item])?;

            let tray_handle = handle.clone();
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Session Viewer")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                            let _ = w.unminimize();
                        }
                    }
                    "report_now" => {
                        let server = report_server();
                        tauri::async_runtime::spawn(async move {
                            let _ = report::send_all_reports(&server).await;
                            let _ = conversation::uploader::flush(
                                &server,
                                &["claude_code", "codex", "cursor"],
                            )
                            .await;
                        });
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(move |_tray, event| {
                    // 左键单击：显示/聚焦窗口（macOS 用户期望左键直接出窗口）
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray_handle.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                            let _ = w.unminimize();
                        }
                    }
                })
                .build(app)?;

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

/// Windows: 关闭系统级 critical-error 弹窗（`SEM_FAILCRITICALERRORS`）和
/// 崩溃报告对话框（`SEM_NOGPFAULTERRORBOX`）。子进程继承该 mode，因此当
/// `git.exe` 等被外部破坏的二进制启动失败 (0xC0000142 等) 时，系统不再弹
/// "应用程序无法正常启动"对话框，spawn 直接返回错误由我们 fallback。
#[cfg(windows)]
fn suppress_windows_error_dialogs() {
    extern "system" {
        fn SetErrorMode(u_mode: u32) -> u32;
    }
    const SEM_FAILCRITICALERRORS: u32 = 0x0001;
    const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
    unsafe {
        // 读当前 mode（SetErrorMode 返回旧值），保留其他位再 OR 上目标位。
        let prev = SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
        SetErrorMode(prev | SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
}
