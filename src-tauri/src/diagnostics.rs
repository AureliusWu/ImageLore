use crate::state::AppState;
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::State;

const MAX_LOG_BYTES: u64 = 1024 * 1024;
const MAX_LOG_FILES: usize = 5;
const ACTIVE_LOG: &str = "imagelore.log";
const RECOVERY_NOTICE: &str = "recovery-last.txt";

static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn logs_dir(root: &Path) -> PathBuf {
    root.join("logs")
}

fn rotated_name(index: usize) -> String {
    format!("imagelore.{}.log", index)
}

fn rotate_if_needed(dir: &Path, max_bytes: u64, max_files: usize) -> Result<(), String> {
    if max_files < 1 {
        return Ok(());
    }
    let active = dir.join(ACTIVE_LOG);
    let size = active.metadata().map(|m| m.len()).unwrap_or(0);
    if size < max_bytes {
        return Ok(());
    }
    let rotated = max_files.saturating_sub(1);
    if rotated == 0 {
        let _ = fs::remove_file(&active);
        return Ok(());
    }
    let oldest = dir.join(rotated_name(rotated));
    let _ = fs::remove_file(oldest);
    for index in (1..rotated).rev() {
        let from = dir.join(rotated_name(index));
        let to = dir.join(rotated_name(index + 1));
        if from.exists() {
            let _ = fs::remove_file(&to);
            fs::rename(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    if active.exists() {
        let first = dir.join(rotated_name(1));
        let _ = fs::remove_file(&first);
        fs::rename(active, first).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn append_with_policy(
    root: &Path,
    max_bytes: u64,
    max_files: usize,
    level: &str,
    message: &str,
) -> Result<(), String> {
    let _guard = LOG_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|e| e.to_string())?;
    let dir = logs_dir(root);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    rotate_if_needed(&dir, max_bytes, max_files)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(ACTIVE_LOG))
        .map_err(|e| e.to_string())?;
    let line = format!("{} [{}] {}\n", now(), level, message.replace('\n', " | "));
    file.write_all(line.as_bytes()).map_err(|e| e.to_string())
}

pub fn log(root: &Path, level: &str, message: &str) {
    let _ = append_with_policy(root, MAX_LOG_BYTES, MAX_LOG_FILES, level, message);
}

pub fn recovery_notice_path(root: &Path) -> PathBuf {
    root.join(RECOVERY_NOTICE)
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticStatus {
    pub data_dir: String,
    pub database_path: String,
    pub backups_dir: String,
    pub logs_dir: String,
    pub active_log: String,
    pub active_log_size: u64,
    pub recovery_notice: Option<String>,
}

#[tauri::command]
pub fn diagnostics_status(state: State<'_, AppState>) -> Result<DiagnosticStatus, String> {
    let dir = logs_dir(&state.data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let active = dir.join(ACTIVE_LOG);
    let active_log_size = active.metadata().map(|m| m.len()).unwrap_or(0);
    let recovery_notice = fs::read_to_string(recovery_notice_path(&state.data_dir)).ok();
    Ok(DiagnosticStatus {
        data_dir: state.data_dir.to_string_lossy().to_string(),
        database_path: state.database_path.to_string_lossy().to_string(),
        backups_dir: state.backups_dir.to_string_lossy().to_string(),
        logs_dir: dir.to_string_lossy().to_string(),
        active_log: active.to_string_lossy().to_string(),
        active_log_size,
        recovery_notice,
    })
}

fn open_folder(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_data_folder(state: State<'_, AppState>) -> Result<bool, String> {
    open_folder(&state.data_dir)?;
    Ok(true)
}

#[tauri::command]
pub fn open_logs_folder(state: State<'_, AppState>) -> Result<bool, String> {
    open_folder(&logs_dir(&state.data_dir))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "imagelore-log-test-{}-{}",
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn rotating_log_keeps_bounded_generations() {
        let root = temp_root();
        let payload = "x".repeat(80);
        append_with_policy(&root, 32, 3, "INFO", &payload).unwrap();
        append_with_policy(&root, 32, 3, "INFO", &payload).unwrap();
        append_with_policy(&root, 32, 3, "INFO", &payload).unwrap();
        let dir = logs_dir(&root);
        assert!(dir.join(ACTIVE_LOG).exists());
        assert!(dir.join(rotated_name(1)).exists());
        assert!(dir.join(rotated_name(2)).exists());
        assert!(!dir.join(rotated_name(3)).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_notice_path_stays_in_data_root() {
        let root = PathBuf::from("C:/ImageLoreData");
        assert_eq!(recovery_notice_path(&root), root.join(RECOVERY_NOTICE));
    }
}
