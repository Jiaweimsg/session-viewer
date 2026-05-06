use std::path::Path;
use std::process::Command;

/// Resume a Copilot CLI session in a terminal window
pub fn resume_session(session_id: String, cwd: String) -> Result<(), String> {
    if !Path::new(&cwd).exists() {
        return Err(format!("Working directory does not exist: {}", cwd));
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\"\nactivate\ndo script \"cd '{}' && copilot --resume={}\"\nend tell",
            cwd, session_id
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let resume_cmd = format!("copilot --resume={}", session_id);
        Command::new("cmd")
            .args(["/c", "start", "", "/d", &cwd, "cmd", "/k", &resume_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let cmd_str = format!("cd '{}' && copilot --resume={}", cwd, session_id);
        let bash_cmd = format!("bash -c '{}'", cmd_str);
        let terminals: [(&str, Vec<&str>); 4] = [
            ("gnome-terminal", vec!["--", "bash", "-c", &cmd_str]),
            ("konsole", vec!["-e", "bash", "-c", &cmd_str]),
            ("xfce4-terminal", vec!["-e", &bash_cmd]),
            ("xterm", vec!["-e", &bash_cmd]),
        ];
        let mut launched = false;
        for (terminal, args) in &terminals {
            if Command::new(terminal)
                .args(args)
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
