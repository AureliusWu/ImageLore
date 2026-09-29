use crate::{db, state::AppState};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::State;

fn open_path(path: &Path, select: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("explorer");
        if select {
            command.arg("/select,").arg(path);
        } else {
            command.arg(path);
        }
        command.spawn().map_err(|error| error.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        if select {
            command.arg("-R");
        }
        command
            .arg(path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if select {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn open_external(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let path = {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        db::get_asset(&conn, id)?.path
    };
    open_path(Path::new(&path), false)?;
    Ok(true)
}

#[tauri::command]
pub fn open_containing_folder(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let path = {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        db::get_asset(&conn, id)?.path
    };
    open_path(Path::new(&path), true)?;
    Ok(true)
}

fn normalized_copy_target(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return path.canonicalize().map_err(|error| error.to_string());
    }

    let file_name = path.file_name().ok_or("目标文件名无效")?;
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or(Path::new("."));

    Ok(parent
        .canonicalize()
        .map_err(|error| error.to_string())?
        .join(file_name))
}

fn copy_file_to(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    if !source_path.exists() {
        return Err("原图片文件不存在，无法另存为".into());
    }

    let source = source_path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let destination = normalized_copy_target(destination_path)?;
    if source == destination {
        return Err("目标位置与原文件相同".into());
    }

    fs::copy(&source, &destination).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn copy_asset_to(
    state: State<'_, AppState>,
    id: i64,
    destination: String,
) -> Result<bool, String> {
    let source = {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        db::get_asset(&conn, id)?.path
    };
    copy_file_to(Path::new(&source), Path::new(&destination))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::copy_file_to;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "imagelore-preview-test-{}-{}",
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn save_as_copies_bytes_and_rejects_same_file() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.png");
        let copy = root.join("copy.png");
        fs::write(&source, b"imagelore-preview-regression").unwrap();

        copy_file_to(&source, &copy).unwrap();
        assert_eq!(fs::read(&copy).unwrap(), b"imagelore-preview-regression");
        assert!(copy_file_to(&source, &source)
            .unwrap_err()
            .contains("目标位置与原文件相同"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_as_rejects_missing_source() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let error = copy_file_to(&root.join("missing.png"), &root.join("copy.png")).unwrap_err();
        assert!(error.contains("原图片文件不存在"));
        let _ = fs::remove_dir_all(root);
    }
}
