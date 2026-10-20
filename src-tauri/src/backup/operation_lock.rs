//! One persistent, object-held operation lock for cooperating restore processes.
//! Existence is not ownership. The file is never truncated, removed, or replaced.

use super::owned_handle::{DirectoryGuard, WindowsExclusiveFile};
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const LOCK_NAME: &str = "restore.operation.lock";
const SIGNATURE: &[u8] = b"ImageLore restore operation lock\nformat=1\n";

#[derive(Debug)]
pub(super) struct OperationGuard {
    // Fields drop in declaration order: release the operation before ancestors.
    _file: WindowsExclusiveFile,
    _directory: DirectoryGuard,
    _ancestors: Vec<DirectoryGuard>,
}

fn lock_error(error: std::io::Error) -> String {
    if matches!(error.raw_os_error(), Some(32 | 33)) {
        format!("资料库恢复操作忙，请稍后重试；operation lock: {error}")
    } else {
        format!("无法取得资料库恢复操作锁；operation lock: {error}")
    }
}

pub(super) fn acquire(data_dir: &Path) -> Result<OperationGuard, String> {
    acquire_before_publish(data_dir, &|_| Ok(()))
}

fn acquire_before_publish(
    data_dir: &Path,
    before_publish: &impl Fn(&Path) -> Result<(), String>,
) -> Result<OperationGuard, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let ancestors = DirectoryGuard::lock_ancestors(data_dir).map_err(lock_error)?;
    let directory = DirectoryGuard::lock_existing(data_dir).map_err(lock_error)?;
    let data = std::fs::canonicalize(data_dir).map_err(lock_error)?;
    let path = data.join(LOCK_NAME);
    // A creator can win between OPEN_EXISTING and publishing a complete file.
    // Reopen the winner; never unlink a stale lock or change ACLs for access.
    for _ in 0..8 {
        let file = match WindowsExclusiveFile::lock_existing(&path) {
            Ok(mut file) => {
                let bytes = file.read_limited(SIGNATURE.len()).map_err(lock_error)?;
                if bytes != SIGNATURE {
                    return Err("操作锁不是受支持的完整协议文件，已保留原文件".into());
                }
                file
            }
            Err(error) if error.raw_os_error() == Some(2) => {
                let temporary = data.join(format!(
                    "restore.operation.next-{:x}-{:x}-{:x}.lock",
                    std::process::id(),
                    super::now_nanos(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match WindowsExclusiveFile::create_new(&temporary) {
                    Ok(mut file) => {
                        // Only a complete, flushed signature can reach the fixed
                        // name. An unpublished/partial temp remains evidence and
                        // never prevents another operation from acquiring a lock.
                        file.write_created(SIGNATURE).map_err(lock_error)?;
                        before_publish(&temporary)?;
                        match file.rename_no_replace(&path) {
                            Ok(()) => (),
                            Err(error)
                                if matches!(error.raw_os_error(), Some(32 | 33 | 80 | 183)) =>
                            {
                                continue;
                            }
                            Err(error) => return Err(lock_error(error)),
                        }
                        file
                    }
                    Err(error) if matches!(error.raw_os_error(), Some(80 | 183)) => continue,
                    Err(error) => return Err(lock_error(error)),
                }
            }
            Err(error) => return Err(lock_error(error)),
        };
        return Ok(OperationGuard {
            _file: file,
            _directory: directory,
            _ancestors: ancestors,
        });
    }
    Err("操作锁名称持续变化，已停止；请稍后重试，未删除或覆盖任何锁文件".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    fn fixture(label: &str) -> PathBuf {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = temporary.join(format!(
            "imagelore-operation-{label}-{}-{}",
            std::process::id(),
            super::super::now_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("fixture.marker"),
            root.to_string_lossy().as_bytes(),
        )
        .unwrap();
        root
    }

    fn checked_fixture(root: &Path) {
        use std::os::windows::fs::MetadataExt;
        assert!(root.is_absolute());
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("imagelore-operation-"));
        assert_eq!(fs::canonicalize(root).unwrap(), root);
        assert_eq!(
            fs::symlink_metadata(root).unwrap().file_attributes() & 0x400,
            0
        );
        assert_eq!(
            fs::read(root.join("fixture.marker")).unwrap(),
            root.to_string_lossy().as_bytes()
        );
    }

    fn cleanup(root: PathBuf) {
        checked_fixture(&root);
        for entry in fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            assert!(
                matches!(
                    entry.file_name().to_str(),
                    Some("fixture.marker" | LOCK_NAME)
                ) || entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("restore.operation.next-")
            );
            assert!(entry.file_type().unwrap().is_file());
            fs::remove_file(entry.path()).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn operation_lock_child_process() {
        use std::io::Write;
        let Some(root) = std::env::var_os("IMAGELORE_OPERATION_LOCK_CHILD") else {
            return;
        };
        let root = PathBuf::from(root);
        checked_fixture(&root);
        if std::env::var_os("IMAGELORE_OPERATION_LOCK_CHILD_EXIT").is_some() {
            let _held = acquire(&root).unwrap();
            println!("operation lock held until process exit");
            std::io::stdout().flush().unwrap();
            // Deliberately bypass Rust Drop: the OS must release the handle.
            std::process::exit(0);
        }
        let error = acquire(&root).unwrap_err();
        assert!(error.contains("操作忙"), "{error}");
        assert!(error.contains("32") || error.contains("33"), "{error}");
        println!("independent process refused by live operation lock");
    }

    #[test]
    fn persistent_operation_lock_blocks_a_process_then_reuses_the_same_object() {
        use std::os::windows::process::CommandExt;
        let root = fixture("process");
        let held = acquire(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "backup::operation_lock::tests::operation_lock_child_process",
                "--nocapture",
            ])
            .env("IMAGELORE_OPERATION_LOCK_CHILD", &root)
            .env_remove("IMAGELORE_OPERATION_LOCK_CHILD_EXIT")
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("independent process refused"));
        assert!(acquire(&root).unwrap_err().contains("操作忙"));
        drop(held);
        let path = root.join(LOCK_NAME);
        let mut file = WindowsExclusiveFile::lock_existing(&path).unwrap();
        let original = file.fingerprint().unwrap();
        assert_eq!(file.read_limited(SIGNATURE.len()).unwrap(), SIGNATURE);
        drop(file);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "backup::operation_lock::tests::operation_lock_child_process",
                "--nocapture",
            ])
            .env("IMAGELORE_OPERATION_LOCK_CHILD", &root)
            .env("IMAGELORE_OPERATION_LOCK_CHILD_EXIT", "1")
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("held until process exit"));
        drop(acquire(&root).unwrap());
        let mut file = WindowsExclusiveFile::lock_existing(&path).unwrap();
        assert_eq!(file.fingerprint().unwrap(), original);
        drop(file);
        cleanup(root);
    }

    #[test]
    fn foreign_or_incomplete_lock_bytes_and_directory_are_preserved() {
        for bytes in [
            &b"foreign lock sentinel"[..],
            &b""[..],
            &SIGNATURE[..SIGNATURE.len() - 1],
        ] {
            let root = fixture("foreign");
            let path = root.join(LOCK_NAME);
            fs::write(&path, bytes).unwrap();
            let modified = fs::metadata(&path).unwrap().modified().unwrap();
            assert!(acquire(&root).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
            cleanup(root);
        }
        let root = fixture("directory");
        let path = root.join(LOCK_NAME);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("foreign.sentinel"), b"directory must survive").unwrap();
        assert!(acquire(&root).is_err());
        assert_eq!(
            fs::read(path.join("foreign.sentinel")).unwrap(),
            b"directory must survive"
        );
        fs::remove_file(path.join("foreign.sentinel")).unwrap();
        fs::remove_dir(path).unwrap();
        cleanup(root);
    }

    #[test]
    fn failed_publication_preserves_temporary_and_does_not_block_future_acquire() {
        let root = fixture("publish-failure");
        let unpublished = std::cell::RefCell::new(None);
        let result = acquire_before_publish(&root, &|path| {
            *unpublished.borrow_mut() = Some(path.to_path_buf());
            Err("injected failure after signature flush before publication".into())
        });
        assert!(result.unwrap_err().contains("injected failure"));
        assert!(!root.join(LOCK_NAME).exists());
        let unpublished = unpublished.into_inner().unwrap();
        assert_eq!(fs::read(&unpublished).unwrap(), SIGNATURE);
        let mut file = WindowsExclusiveFile::lock_existing(&unpublished).unwrap();
        let before = file.fingerprint().unwrap();
        drop(file);
        drop(acquire(&root).unwrap());
        assert_eq!(fs::read(root.join(LOCK_NAME)).unwrap(), SIGNATURE);
        let mut file = WindowsExclusiveFile::lock_existing(&unpublished).unwrap();
        assert_eq!(file.fingerprint().unwrap(), before);
        drop(file);
        cleanup(root);
    }

    #[test]
    fn reparse_data_directory_and_lock_name_do_not_touch_the_target() {
        use std::os::windows::process::CommandExt;
        let root = fixture("reparse");
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("foreign.sentinel"), b"junction target sentinel").unwrap();
        for name in ["junction", LOCK_NAME] {
            let junction = root.join(name);
            let output = std::process::Command::new("cmd.exe")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(&junction)
                .arg(&target)
                .creation_flags(0x0800_0000)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(if name == LOCK_NAME {
                acquire(&root)
            } else {
                acquire(&junction)
            }
            .is_err());
            assert_eq!(
                fs::read(target.join("foreign.sentinel")).unwrap(),
                b"junction target sentinel"
            );
            assert!(!target.join(LOCK_NAME).exists());
            // Remove only this exact junction; never recurse into its target.
            fs::remove_dir(&junction).unwrap();
        }
        fs::remove_file(target.join("foreign.sentinel")).unwrap();
        fs::remove_dir(&target).unwrap();
        cleanup(root);
    }
}
