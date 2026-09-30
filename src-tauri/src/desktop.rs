#[cfg(target_os = "windows")]
pub fn refresh_existing_shortcut(data_dir: &std::path::Path) {
    use std::{fs, os::windows::process::CommandExt, process::Command};
    let state = data_dir.join("desktop-shortcut.json");
    if !state.is_file() {
        return;
    }
    let data_dir = data_dir.to_owned();
    std::thread::spawn(move || {
        let update = || -> Result<(), String> {
            let executable = std::env::current_exe().map_err(|e| e.to_string())?;
            let token = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos();
            let script = data_dir.join(format!(
                "update-desktop-shortcut-{}-{token}.ps1",
                std::process::id()
            ));
            fs::write(
                &script,
                include_str!("../../scripts/update_desktop_shortcut.ps1"),
            )
            .map_err(|e| e.to_string())?;
            let output = Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
                .arg(&script)
                .arg("-ExecutablePath")
                .arg(executable)
                .arg("-ExpectedVersion")
                .arg(env!("CARGO_PKG_VERSION"))
                .arg("-StatePath")
                .arg(state)
                .arg("-ManagedOnly")
                .creation_flags(0x08000000)
                .output();
            let _ = fs::remove_file(&script);
            let output = output.map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }
            Ok(())
        };
        if let Err(error) = update() {
            crate::diagnostics::log(&data_dir, "WARN", &format!("desktop shortcut: {error}"));
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn refresh_existing_shortcut(_data_dir: &std::path::Path) {}
