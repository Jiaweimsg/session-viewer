use std::process::Command;

pub fn resume_session(session_id: String, work_dir: String) -> Result<(), String> {
    let work_dir = normalize_path(&work_dir);

    if !std::path::Path::new(&work_dir).exists() {
        return Err(format!("Working directory does not exist: {}", work_dir));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let resume_arg = format!("opencode --session {}", session_id);
        Command::new("cmd")
            .args(["/c", "start", "", "/d", &work_dir, "cmd", "/k", &resume_arg])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\" to do script \"cd '{}' && opencode --session {}\"",
            work_dir, session_id
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let cmd_str = format!("cd '{}' && opencode --session {}", work_dir, session_id);

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
