//! Resume only objects identified by a complete, validated restore journal.
//! Every live role and raw proof is acquired before the first mutation. A
//! readable replacement, or even a new commit in the same file, is not ours.

use super::companions;
use super::owned_handle::{
    DirectoryGuard, DirectoryIdentity, FileFingerprint, WindowsExclusiveFile,
};
use super::restore_journal::{self, Mode, Prepared};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

const TRANSACTION: &str = "restore.transaction";
const PREPARED: &str = "PREPARED.json";
const COMMITTED: &str = "COMMITTED";
const STAGED: &str = "staged";
const LABELS: [&str; 3] = ["database", "wal", "shm"];
const NAMES: [&str; 6] = [PREPARED, COMMITTED, STAGED, "database", "wal", "shm"];

fn io(error: std::io::Error) -> String {
    error.to_string()
}

struct Held {
    file: WindowsExclusiveFile,
    fingerprint: FileFingerprint,
}

impl Held {
    fn lock_metadata(path: &Path) -> Result<Option<Self>, String> {
        match WindowsExclusiveFile::lock_existing(path) {
            Ok(mut file) => {
                // Refuse oversized metadata before the streaming fingerprint;
                // a malformed journal cannot cause an unbounded hash read.
                file.read_limited(restore_journal::MAX_JOURNAL_BYTES)
                    .map_err(io)?;
                let fingerprint = file.fingerprint().map_err(io)?;
                Ok(Some(Self { file, fingerprint }))
            }
            Err(error) if error.raw_os_error() == Some(2) => Ok(None),
            Err(error) => Err(io(error)),
        }
    }

    fn lock(path: &Path) -> Result<Option<Self>, String> {
        match WindowsExclusiveFile::lock_existing(path) {
            Ok(mut file) => {
                let fingerprint = file.fingerprint().map_err(io)?;
                Ok(Some(Self { file, fingerprint }))
            }
            Err(error) if error.raw_os_error() == Some(2) => Ok(None),
            Err(error) => Err(io(error)),
        }
    }

    fn read(&mut self) -> Result<Vec<u8>, String> {
        self.file
            .read_limited(restore_journal::MAX_JOURNAL_BYTES)
            .map_err(io)
    }

    fn rename(&mut self, target: &Path) -> Result<(), String> {
        if self.file.fingerprint().map_err(io)? != self.fingerprint {
            return Err("恢复对象在移动前改变，已保留".into());
        }
        self.file.rename_no_replace(target).map_err(io)
    }

    fn delete(self) -> Result<(), String> {
        self.file.delete_matching(&self.fingerprint).map_err(io)
    }
}

fn lock_roles(paths: &[PathBuf; 3]) -> Result<[Option<Held>; 3], String> {
    let mut files = [None, None, None];
    for (file, path) in files.iter_mut().zip(paths) {
        *file = Held::lock(path)?;
    }
    Ok(files)
}

fn matches(file: &Option<Held>, expected: &Option<FileFingerprint>) -> bool {
    match (file, expected) {
        (Some(file), Some(expected)) => file.fingerprint == *expected,
        (None, None) => true,
        _ => false,
    }
}

fn require_match(file: &Held, expected: &FileFingerprint) -> Result<(), String> {
    if file.fingerprint != *expected {
        return Err("恢复文件的身份或内容已经改变，已保留全部事务文件".into());
    }
    Ok(())
}

fn absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.raw_os_error() == Some(2) => Ok(()),
        Err(error) => Err(io(error)),
        Ok(_) => Err(format!(
            "恢复路径出现未持有对象，已停止：{}",
            path.display()
        )),
    }
}

fn names(directory: &Path) -> Result<bool, String> {
    let mut empty = true;
    for entry in fs::read_dir(directory).map_err(io)? {
        let entry = entry.map_err(io)?;
        empty = false;
        if !NAMES
            .iter()
            .any(|name| entry.file_name() == OsStr::new(name))
        {
            return Err("恢复事务目录含有未知内容，已保留全部文件".into());
        }
    }
    Ok(empty)
}

fn unique_identities(prepared: &Prepared) -> Result<(), String> {
    let mut identities = vec![
        prepared.data_directory.clone(),
        prepared.transaction_directory.clone(),
    ];
    for fingerprint in prepared
        .originals
        .iter()
        .flatten()
        .chain(std::iter::once(&prepared.staged))
        .chain(
            prepared
                .pending
                .iter()
                .flat_map(|roles| roles.iter().flatten()),
        )
        .chain(prepared.raw.iter().flatten().map(|raw| &raw.fingerprint))
    {
        let identity = DirectoryIdentity {
            volume_serial: fingerprint.volume_serial,
            file_id: fingerprint.file_id,
        };
        if identities.contains(&identity) {
            return Err("恢复记录中的文件身份互为别名，已保留全部文件".into());
        }
        identities.push(identity);
    }
    if identities[0] == identities[1] {
        return Err("恢复目录身份互为别名，已保留全部文件".into());
    }
    Ok(())
}

struct State {
    active_paths: [PathBuf; 3],
    reserved_paths: [PathBuf; 3],
    pending_paths: [PathBuf; 3],
    directory: DirectoryGuard,
    prepared: Option<Held>,
    committed: Option<Held>,
    active: [Option<Held>; 3],
    reserved: [Option<Held>; 3],
    stage: Option<Held>,
    pending: Option<[Option<Held>; 3]>,
    // Raw files are evidence, never cleanup targets. Keep their locks and every
    // ancestor guard alive throughout rollback or committed cleanup.
    _raw: Vec<Held>,
    _raw_parents: Vec<DirectoryGuard>,
}

impl State {
    fn check_unheld_names(&self) -> Result<(), String> {
        names(self.directory.path())?;
        for (path, held) in self
            .active_paths
            .iter()
            .zip(&self.active)
            .chain(self.reserved_paths.iter().zip(&self.reserved))
        {
            if held.is_none() {
                absent(path)?;
            }
        }
        if self.stage.is_none() {
            absent(&self.directory.path().join(STAGED))?;
        }
        if self.prepared.is_none() {
            absent(&self.directory.path().join(PREPARED))?;
        }
        if self.committed.is_none() {
            absent(&self.directory.path().join(COMMITTED))?;
        }
        if let Some(pending) = &self.pending {
            for (path, held) in self.pending_paths.iter().zip(pending) {
                if held.is_none() {
                    absent(path)?;
                }
            }
        }
        Ok(())
    }

    fn validate_rollback(&self, prepared: &Prepared) -> Result<(), String> {
        let mut all_originals_active = true;
        for ordinal in 0..3 {
            let expected = &prepared.originals[ordinal];
            let active = &self.active[ordinal];
            let reserved = &self.reserved[ordinal];
            if let Some(reserved) = reserved {
                require_match(
                    reserved,
                    expected.as_ref().ok_or("恢复保留区出现未记录文件")?,
                )?;
            }
            let active_is_stage = ordinal == 0
                && active
                    .as_ref()
                    .is_some_and(|file| file.fingerprint == prepared.staged);
            if let Some(active) = active {
                if !active_is_stage {
                    require_match(active, expected.as_ref().ok_or("活动路径出现未记录文件")?)?;
                }
            }
            if expected.is_some() {
                let in_active = active.is_some() && !active_is_stage;
                if in_active == reserved.is_some() {
                    return Err("原件必须恰好位于活动区或保留区之一，已停止恢复".into());
                }
            }
            all_originals_active &= matches(active, expected) && reserved.is_none();
        }
        if let Some(stage) = &self.stage {
            require_match(stage, &prepared.staged)?;
        }
        let installed = self.active[0]
            .as_ref()
            .is_some_and(|file| file.fingerprint == prepared.staged);
        if self.stage.is_some() && installed {
            return Err("暂存资料库出现在两个位置，已保留".into());
        }
        if self.stage.is_none() && !installed && !all_originals_active {
            return Err("恢复暂存对象缺失且原件尚未全部归位，已保留".into());
        }
        Ok(())
    }

    fn validate_committed(&self, prepared: &Prepared) -> Result<(), String> {
        require_match(
            self.active[0]
                .as_ref()
                .ok_or("已提交恢复的活动资料库缺失")?,
            &prepared.staged,
        )?;
        if self.active.iter().skip(1).any(Option::is_some) || self.stage.is_some() {
            return Err("已提交恢复出现新侧车或重复暂存对象，已保留全部文件".into());
        }
        for (file, expected) in self.reserved.iter().zip(&prepared.originals) {
            if let Some(file) = file {
                require_match(file, expected.as_ref().ok_or("保留区出现未记录文件")?)?;
            }
        }
        if let Some(pending) = &self.pending {
            let expected = prepared.pending.as_ref().ok_or("待恢复记录缺失")?;
            for (file, expected) in pending.iter().zip(expected) {
                if let Some(file) = file {
                    require_match(file, expected.as_ref().ok_or("待恢复路径出现新文件")?)?;
                }
            }
        }
        Ok(())
    }

    fn rollback(mut self, prepared: &Prepared) -> Result<(), String> {
        self.validate_rollback(prepared)?;
        self.check_unheld_names()?;
        if self.active[0]
            .as_ref()
            .is_some_and(|file| file.fingerprint == prepared.staged)
        {
            let mut stage = self.active[0].take().ok_or("安装句柄缺失")?;
            stage.rename(&self.directory.path().join(STAGED))?;
            self.stage = Some(stage);
        }
        for ordinal in 0..3 {
            self.check_unheld_names()?;
            if let Some(mut original) = self.reserved[ordinal].take() {
                original.rename(&self.active_paths[ordinal])?;
                self.active[ordinal] = Some(original);
            }
        }
        self.check_unheld_names()?;
        if let Some(stage) = self.stage.take() {
            stage.delete()?;
        }
        self.check_unheld_names()?;
        if let Some(record) = self.prepared.take() {
            record.delete()?;
        }
        self.check_unheld_names()?;
        let identity = self.directory.identity().map_err(io)?;
        self.directory.delete_matching_empty(&identity).map_err(io)
    }

    fn cleanup(mut self, prepared: &Prepared) -> Result<(), String> {
        self.validate_committed(prepared)?;
        self.check_unheld_names()?;
        for ordinal in 0..3 {
            self.check_unheld_names()?;
            if let Some(original) = self.reserved[ordinal].take() {
                original.delete()?;
            }
        }
        for ordinal in 0..3 {
            self.check_unheld_names()?;
            if let Some(pending) = self
                .pending
                .as_mut()
                .and_then(|roles| roles[ordinal].take())
            {
                pending.delete()?;
            }
        }
        self.check_unheld_names()?;
        if let Some(record) = self.prepared.take() {
            record.delete()?;
        }
        self.check_unheld_names()?;
        // COMMITTED is self-contained and always removed last. A crash after
        // PREPARED removal therefore remains fully recoverable.
        self.committed.take().ok_or("提交记录句柄缺失")?.delete()?;
        self.check_unheld_names()?;
        let identity = self.directory.identity().map_err(io)?;
        self.directory.delete_matching_empty(&identity).map_err(io)
    }
}

pub(super) fn finish(database: &Path, data_dir: &Path) -> Result<(), String> {
    if !database.is_absolute() || !data_dir.is_absolute() {
        return Err("恢复需要绝对资料库和数据目录路径".into());
    }
    let _ancestors = DirectoryGuard::lock_ancestors(data_dir).map_err(io)?;
    let data_guard = DirectoryGuard::lock_existing(data_dir).map_err(io)?;
    let data = fs::canonicalize(data_dir).map_err(io)?;
    if fs::canonicalize(database.parent().ok_or("资料库缺少父目录")?).map_err(io)? != data {
        return Err("资料库必须直接位于数据目录中".into());
    }
    let name = database
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("资料库名称编码无效")?;
    if name.is_empty() || name.contains(['/', '\\', ':', '\0']) || name.ends_with(['.', ' ']) {
        return Err("资料库名称无效".into());
    }
    let directory = match DirectoryGuard::lock_existing_mutable(&data.join(TRANSACTION)) {
        Ok(directory) => directory,
        Err(error) if error.raw_os_error() == Some(2) => return Ok(()),
        Err(error) => return Err(io(error)),
    };
    if names(directory.path())? {
        // This fixed application-reserved directory contains no files at all.
        // The OS only deletes it if it is still empty, using its held identity.
        let identity = directory.identity().map_err(io)?;
        return directory.delete_matching_empty(&identity).map_err(io);
    }
    let mut prepared_record = Held::lock_metadata(&directory.path().join(PREPARED))?;
    let mut commit_record = Held::lock_metadata(&directory.path().join(COMMITTED))?;
    let from_prepared = prepared_record
        .as_mut()
        .map(|record| restore_journal::decode_prepared(&record.read()?, name))
        .transpose()?;
    let from_commit = commit_record
        .as_mut()
        .map(|record| restore_journal::decode_commit(&record.read()?, name))
        .transpose()?;
    if let (Some(left), Some(right)) = (&from_prepared, &from_commit) {
        if left != right {
            return Err("恢复准备记录和提交记录不一致，已保留全部文件".into());
        }
    }
    let committed = from_commit.is_some();
    let prepared = from_prepared
        .or(from_commit)
        .ok_or("非空恢复目录缺少可验证记录，已保留")?;
    if prepared.data_directory != data_guard.identity().map_err(io)?
        || prepared.transaction_directory != directory.identity().map_err(io)?
    {
        return Err("恢复目录身份与记录不一致，已保留全部文件".into());
    }
    unique_identities(&prepared)?;
    let active_paths = companions(&data.join(name));
    let reserved_paths = LABELS.map(|label| directory.path().join(label));
    let pending_paths = companions(&data.join("restore.pending.sqlite3"));
    let active = lock_roles(&active_paths)?;
    let reserved = lock_roles(&reserved_paths)?;
    let stage = Held::lock(&directory.path().join(STAGED))?;
    let pending = if committed && prepared.mode == Mode::Pending {
        Some(lock_roles(&pending_paths)?)
    } else {
        None
    };
    let mut raw = Vec::new();
    let mut raw_parents = Vec::new();
    for record in prepared.raw.iter().flatten() {
        let relative: PathBuf = record.relative_path.split(['/', '\\']).collect();
        let path = data.join(relative);
        raw_parents.extend(DirectoryGuard::lock_ancestors(&path).map_err(io)?);
        let parent = fs::canonicalize(path.parent().ok_or("原件副本缺少父目录")?).map_err(io)?;
        if !parent.starts_with(&data) {
            return Err("原件副本越过数据目录，已保留事务".into());
        }
        let raw_path = parent.join(path.file_name().ok_or("原件副本名称无效")?);
        let file = Held::lock(&raw_path)?.ok_or("恢复原件副本缺失，已保留事务")?;
        require_match(&file, &record.fingerprint)?;
        raw.push(file);
    }
    let state = State {
        active_paths,
        reserved_paths,
        pending_paths,
        directory,
        prepared: prepared_record,
        committed: commit_record,
        active,
        reserved,
        stage,
        pending,
        _raw: raw,
        _raw_parents: raw_parents,
    };
    if committed {
        state.cleanup(&prepared)
    } else {
        state.rollback(&prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::super::restore_journal::RawRecord;
    use super::*;
    use std::{
        io::Write,
        os::windows::fs::MetadataExt,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Fixture {
        root: PathBuf,
        temporary: PathBuf,
        marker: Vec<u8>,
    }

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let token = format!(
                "{}-{}-{}",
                std::process::id(),
                super::super::now_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
            let root = temporary.join(format!("imagelore-guarded-resume-{token}"));
            fs::create_dir(&root).unwrap();
            let marker = format!("synthetic guarded restore fixture\n{token}\n").into_bytes();
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join("fixture.marker"))
                .unwrap();
            file.write_all(&marker).unwrap();
            file.sync_all().unwrap();
            drop(file);
            fs::create_dir(root.join("data")).unwrap();
            fs::create_dir(root.join("data/recovery")).unwrap();
            fs::create_dir(root.join("data/restore.transaction")).unwrap();
            Self {
                root: fs::canonicalize(root).unwrap(),
                temporary,
                marker,
            }
        }

        fn data(&self) -> PathBuf {
            self.root.join("data")
        }
        fn active(&self) -> PathBuf {
            self.data().join("library.sqlite3")
        }
        fn txn(&self, name: &str) -> PathBuf {
            self.data().join(TRANSACTION).join(name)
        }
        fn pending(&self) -> PathBuf {
            self.data().join("restore.pending.sqlite3")
        }

        fn cleanup(self) {
            fn ordinary_tree(path: &Path) {
                let metadata = fs::symlink_metadata(path).unwrap();
                assert_eq!(
                    metadata.file_attributes() & 0x400,
                    0,
                    "fixture reparse point must be preserved"
                );
                assert!(metadata.is_dir() || metadata.is_file());
                if metadata.is_dir() {
                    for entry in fs::read_dir(path).unwrap() {
                        ordinary_tree(&entry.unwrap().path());
                    }
                }
            }
            let metadata = fs::symlink_metadata(&self.root).unwrap();
            assert!(metadata.is_dir());
            assert_eq!(metadata.file_attributes() & 0x400, 0);
            let resolved = fs::canonicalize(&self.root).unwrap();
            let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
            assert_eq!(temporary, self.temporary);
            assert_eq!(resolved, self.root);
            assert_eq!(resolved.parent(), Some(temporary.as_path()));
            assert_eq!(
                fs::read(resolved.join("fixture.marker")).unwrap(),
                self.marker
            );
            ordinary_tree(&resolved);
            assert_eq!(fs::canonicalize(&self.root).unwrap(), resolved);
            fs::remove_dir_all(resolved).unwrap();
        }
    }

    fn write_file(path: &Path, bytes: &[u8]) -> FileFingerprint {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);
        WindowsExclusiveFile::lock_existing(path)
            .unwrap()
            .fingerprint()
            .unwrap()
    }

    fn setup(fixture: &Fixture, installed: bool, pending: bool) -> Prepared {
        let original = write_file(&fixture.txn("database"), b"original user bytes");
        let raw = write_file(
            &fixture.data().join("recovery/original.raw"),
            b"original user bytes",
        );
        let staged = write_file(
            &if installed {
                fixture.active()
            } else {
                fixture.txn(STAGED)
            },
            b"restore candidate bytes",
        );
        Prepared {
            token: "abcd-1234".into(),
            database_name: "library.sqlite3".into(),
            data_directory: DirectoryGuard::lock_existing(&fixture.data())
                .unwrap()
                .identity()
                .unwrap(),
            transaction_directory: DirectoryGuard::lock_existing(&fixture.data().join(TRANSACTION))
                .unwrap()
                .identity()
                .unwrap(),
            mode: if pending {
                Mode::Pending
            } else {
                Mode::Recovery
            },
            originals: [Some(original), None, None],
            staged,
            pending: pending.then(|| {
                [
                    Some(write_file(&fixture.pending(), b"pending source bytes")),
                    None,
                    None,
                ]
            }),
            raw: [
                Some(RawRecord {
                    relative_path: "recovery/original.raw".into(),
                    fingerprint: raw,
                }),
                None,
                None,
            ],
        }
    }

    fn prepared(fixture: &Fixture, record: &Prepared) {
        write_file(
            &fixture.txn(PREPARED),
            &restore_journal::encode_prepared(record).unwrap(),
        );
    }

    #[test]
    fn interrupted_restore_rolls_back_known_objects_and_keeps_pending_and_raw() {
        let fixture = Fixture::new();
        let record = setup(&fixture, true, true);
        prepared(&fixture, &record);
        finish(&fixture.active(), &fixture.data()).unwrap();
        assert_eq!(fs::read(fixture.active()).unwrap(), b"original user bytes");
        assert_eq!(
            fs::read(fixture.pending()).unwrap(),
            b"pending source bytes"
        );
        assert_eq!(
            fs::read(fixture.data().join("recovery/original.raw")).unwrap(),
            b"original user bytes"
        );
        assert!(!fixture.data().join(TRANSACTION).exists());
        fixture.cleanup();
    }

    #[test]
    fn interrupted_restore_preserves_foreign_active_before_any_mutation() {
        let fixture = Fixture::new();
        let record = setup(&fixture, false, true);
        prepared(&fixture, &record);
        write_file(&fixture.active(), b"new independent library");
        let journal = fs::read(fixture.txn(PREPARED)).unwrap();
        assert!(finish(&fixture.active(), &fixture.data()).is_err());
        assert_eq!(
            fs::read(fixture.active()).unwrap(),
            b"new independent library"
        );
        assert_eq!(
            fs::read(fixture.txn("database")).unwrap(),
            b"original user bytes"
        );
        assert_eq!(
            fs::read(fixture.txn(STAGED)).unwrap(),
            b"restore candidate bytes"
        );
        assert_eq!(
            fs::read(fixture.pending()).unwrap(),
            b"pending source bytes"
        );
        assert_eq!(fs::read(fixture.txn(PREPARED)).unwrap(), journal);
        fixture.cleanup();
    }

    #[test]
    fn interrupted_restore_preserves_new_content_on_the_same_installed_file_id() {
        let fixture = Fixture::new();
        let record = setup(&fixture, true, true);
        prepared(&fixture, &record);
        fs::write(
            fixture.active(),
            b"later committed user data in same object",
        )
        .unwrap();
        let after = WindowsExclusiveFile::lock_existing(&fixture.active())
            .unwrap()
            .fingerprint()
            .unwrap();
        assert_eq!(after.file_id, record.staged.file_id);
        assert_ne!(after.sha256, record.staged.sha256);
        assert!(finish(&fixture.active(), &fixture.data()).is_err());
        assert_eq!(
            fs::read(fixture.active()).unwrap(),
            b"later committed user data in same object"
        );
        assert_eq!(
            fs::read(fixture.txn("database")).unwrap(),
            b"original user bytes"
        );
        assert_eq!(
            fs::read(fixture.pending()).unwrap(),
            b"pending source bytes"
        );
        assert!(fixture.txn(PREPARED).exists());
        fixture.cleanup();
    }

    #[test]
    fn committed_restore_resumes_partial_cleanup_with_only_the_commit_record() {
        let fixture = Fixture::new();
        let mut record = setup(&fixture, true, true);
        record.pending.as_mut().unwrap()[1] = Some(write_file(
            &companions(&fixture.pending())[1],
            b"pending WAL source",
        ));
        write_file(
            &fixture.txn(COMMITTED),
            &restore_journal::encode_commit(&record).unwrap(),
        );
        // Simulate previously completed original/main-pending cleanup and a
        // crash after PREPARED removal but before final COMMITTED removal.
        fs::remove_file(fixture.txn("database")).unwrap();
        fs::remove_file(fixture.pending()).unwrap();
        finish(&fixture.active(), &fixture.data()).unwrap();
        assert_eq!(
            fs::read(fixture.active()).unwrap(),
            b"restore candidate bytes"
        );
        assert_eq!(
            fs::read(fixture.data().join("recovery/original.raw")).unwrap(),
            b"original user bytes"
        );
        assert!(!companions(&fixture.pending())[1].exists());
        assert!(!fixture.data().join(TRANSACTION).exists());
        fixture.cleanup();
    }

    #[test]
    fn rollback_resumes_after_originals_returned_and_stage_was_already_cleaned() {
        let fixture = Fixture::new();
        let record = setup(&fixture, false, true);
        prepared(&fixture, &record);
        fs::rename(fixture.txn("database"), fixture.active()).unwrap();
        fs::remove_file(fixture.txn(STAGED)).unwrap();
        finish(&fixture.active(), &fixture.data()).unwrap();
        assert_eq!(fs::read(fixture.active()).unwrap(), b"original user bytes");
        assert_eq!(
            fs::read(fixture.pending()).unwrap(),
            b"pending source bytes"
        );
        assert!(!fixture.data().join(TRANSACTION).exists());
        fixture.cleanup();
    }

    #[test]
    fn unrecorded_nonempty_directory_is_preserved_and_only_an_empty_one_is_removed() {
        for nonempty in [true, false] {
            let fixture = Fixture::new();
            if nonempty {
                write_file(&fixture.txn(STAGED), b"unrecorded stage");
                assert!(finish(&fixture.active(), &fixture.data()).is_err());
                assert_eq!(fs::read(fixture.txn(STAGED)).unwrap(), b"unrecorded stage");
            } else {
                finish(&fixture.active(), &fixture.data()).unwrap();
                assert!(!fixture.data().join(TRANSACTION).exists());
            }
            fixture.cleanup();
        }
    }

    #[test]
    fn torn_record_or_changed_raw_preserves_every_live_object() {
        for torn in [true, false] {
            let fixture = Fixture::new();
            let record = setup(&fixture, true, true);
            let mut bytes = restore_journal::encode_prepared(&record).unwrap();
            if torn {
                bytes.pop();
            }
            write_file(&fixture.txn(PREPARED), &bytes);
            if !torn {
                fs::write(
                    fixture.data().join("recovery/original.raw"),
                    b"changed raw proof",
                )
                .unwrap();
            }
            assert!(finish(&fixture.active(), &fixture.data()).is_err());
            assert_eq!(
                fs::read(fixture.active()).unwrap(),
                b"restore candidate bytes"
            );
            assert_eq!(
                fs::read(fixture.txn("database")).unwrap(),
                b"original user bytes"
            );
            assert_eq!(
                fs::read(fixture.pending()).unwrap(),
                b"pending source bytes"
            );
            assert_eq!(fs::read(fixture.txn(PREPARED)).unwrap(), bytes);
            fixture.cleanup();
        }
    }

    #[test]
    fn committed_restore_preserves_foreign_sidecar_and_unrecorded_directory_content() {
        for extra in ["sidecar", "unknown"] {
            let fixture = Fixture::new();
            let record = setup(&fixture, true, true);
            prepared(&fixture, &record);
            write_file(
                &fixture.txn(COMMITTED),
                &restore_journal::encode_commit(&record).unwrap(),
            );
            let extra_path = if extra == "sidecar" {
                companions(&fixture.active())[1].clone()
            } else {
                fixture.txn("foreign.bin")
            };
            write_file(&extra_path, b"foreign bytes");
            assert!(finish(&fixture.active(), &fixture.data()).is_err());
            assert_eq!(fs::read(extra_path).unwrap(), b"foreign bytes");
            assert_eq!(
                fs::read(fixture.txn("database")).unwrap(),
                b"original user bytes"
            );
            assert_eq!(
                fs::read(fixture.pending()).unwrap(),
                b"pending source bytes"
            );
            assert!(fixture.txn(PREPARED).exists());
            assert!(fixture.txn(COMMITTED).exists());
            fixture.cleanup();
        }
    }
}
