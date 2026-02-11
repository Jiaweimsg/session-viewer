use std::fs;
use std::path::Path;
use std::process::Command;

use crate::claude::models::session::SessionsIndex;

pub fn resume_session(
    session_id: String,
    project_path: String,
    file_path: Option<String>,
) -> Result<(), String> {
    // Try to resolve the real project path from sessions-index.json
    let effective_path = resolve_project_path(&session_id, &project_path, file_path.as_deref());
    let effective_path = normalize_path(&effective_path);

    if !Path::new(&effective_path).exists() {
        return Err(format!("Project path does not exist: {}", effective_path));
    }

    run_in_terminal(&effective_path, &session_id)
}

/// Try to read originalPath from sessions-index.json for a more accurate project path
fn resolve_project_path(
    session_id: &str,
    fallback_path: &str,
    file_path: Option<&str>,
) -> String {
    // If file_path is provided, look for sessions-index.json in the same directory
    if let Some(fp) = file_path {
        let fp_path = Path::new(fp);
        if let Some(parent) = fp_path.parent() {
            let index_path = parent.join("sessions-index.json");
            if let Ok(content) = fs::read_to_string(&index_path) {
                if let Ok(index) = serde_json::from_str::<SessionsIndex>(&content) {
                    // Use originalPath from index if available
                    if let Some(ref orig) = index.original_path {
                        if Path::new(orig).exists() {
                            return orig.clone();
                        }
                    }
                }
            }
        }
    }

    // Also try the project_path as an encoded directory name containing sessions-index.json
    let index_path = Path::new(fallback_path).join("sessions-index.json");
    if let Ok(content) = fs::read_to_string(&index_path) {
        if let Ok(index) = serde_json::from_str::<SessionsIndex>(&content) {
            if let Some(ref orig) = index.original_path {
                if Path::new(orig).exists() {
                    return orig.clone();
                }
            }
        }
    }

    let _ = session_id; // used for potential future index entry lookup
    fallback_path.to_string()
}

fn run_in_terminal(project_path: &str, session_id: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let resume_arg = format!("claude --resume {}", session_id);
        Command::new("cmd")
            .args(["/c", "start", "", "/d", project_path, "cmd", "/k", &resume_arg])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\" to do script \"cd '{}' && claude --resume {}\"",
            project_path, session_id
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let cmd_str = format!(
            "cd '{}' && claude --resume {}",
            project_path, session_id
        );

        let xfce_arg = format!("bash -c '{}'", cmd_str);
        let xterm_arg = format!("bash -c '{}'", cmd_str);
        let terminals: [(&str, &[&str]); 4] = [
            ("gnome-terminal", &["--", "bash", "-c", &cmd_str]),
            ("konsole", &["-e", "bash", "-c", &cmd_str]),
            ("xfce4-terminal", &["-e", &xfce_arg]),
            ("xterm", &["-e", &xterm_arg]),
        ];

        let mut launched = false;
        for (terminal, args) in &terminals {
            if Command::new(terminal)
                .args(*args)
                .process_group(0)
                .spawn()
                .is_ok()
            {
                launched = true;
                break;
            }
        }

        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}

fn normalize_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.replace('\\', "/")
    }
}
