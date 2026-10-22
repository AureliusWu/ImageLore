//! Windows restore: held objects, no-replace moves, and a durable ownership record.
//! The journal is written before any original is moved. Cleanup and interrupted
//! rollback use the same recorded identities, never the apparent validity of an
//! unrelated file at the active name.

use super::owned_handle::{DirectoryGuard, ExclusiveBundle, FileFingerprint, WindowsExclusiveFile};
use super::restore_journal::{self, Mode, Prepared, RawRecord, TargetKind};
use super::{
    companions, known_sqlite_damage, now_nanos, sql_path, supported_marker, validate_in_place,
    RestoreStep, VALIDATION_CACHE_KIB,
};
use rusqlite::{Connection, OpenFlags};
use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const LABELS: [&str; 3] = ["database", "wal", "shm"];

pub(super) enum RestorePurpose {
    ApplyPending,
    RecoverCorrupt,
    StagePending,
    LegacyPromote {
        expected_source: Box<[Option<FileFingerprint>; 3]>,
    },
}

impl RestorePurpose {
    fn target(&self) -> TargetKind {
        match self {
            Self::ApplyPending | Self::RecoverCorrupt => TargetKind::ActiveLibrary,
            Self::StagePending | Self::LegacyPromote { .. } => TargetKind::PendingCandidate,
        }
    }

    fn mode(&self) -> Mode {
        match self {
            Self::ApplyPending => Mode::Pending,
            Self::RecoverCorrupt => Mode::Recovery,
            Self::StagePending | Self::LegacyPromote { .. } => Mode::StagePending,
        }
    }

    fn check_target(&self, current: &[Option<FileFingerprint>; 3]) -> Result<(), String> {
        if matches!(self, Self::RecoverCorrupt) && current[0].is_none() {
            return Err("自动恢复时活动资料库已不存在，已保留备份候选".into());
        }
        if matches!(self, Self::LegacyPromote { .. }) && current.iter().any(Option::is_some) {
            return Err("旧候选提升时 pending 名称已被占用，已保留双方".into());
        }
        Ok(())
    }
}

fn io(error: std::io::Error) -> String {
    error.to_string()
}

fn token() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        now_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.raw_os_error() == Some(2) => Ok(()),
        Err(error) => Err(io(error)),
        Ok(_) => Err(format!(
            "恢复路径出现未持有的文件，已停止并保留双方：{}",
            path.display()
        )),
    }
}

fn same_object(a: &FileFingerprint, b: &FileFingerprint) -> bool {
    a.volume_serial == b.volume_serial && a.file_id == b.file_id
}

struct Workspace {
    database: PathBuf,
    data: PathBuf,
    _ancestors: Vec<DirectoryGuard>,
    guard: DirectoryGuard,
}

impl Workspace {
    fn open(database: &Path, data_dir: &Path) -> Result<Self, String> {
        if !database.is_absolute() || !data_dir.is_absolute() {
            return Err("恢复需要绝对资料库和数据目录路径".into());
        }
        let ancestors = DirectoryGuard::lock_ancestors(data_dir).map_err(io)?;
        let guard = DirectoryGuard::lock_existing(data_dir).map_err(io)?;
        let data = fs::canonicalize(data_dir).map_err(io)?;
        let parent = fs::canonicalize(database.parent().ok_or("资料库缺少父目录")?).map_err(io)?;
        if parent != data {
            return Err("资料库必须直接位于受保护的数据目录内".into());
        }
        let name = database
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or("资料库名称无效")?;
        if name.is_empty() || name.contains(['/', '\\', ':', '\0']) || name.ends_with(['.', ' ']) {
            return Err("资料库名称无效".into());
        }
        Ok(Self {
            database: data.join(name),
            data,
            _ancestors: ancestors,
            guard,
        })
    }
}

/// A disposable SQLite view owns every possible DB/WAL/SHM name up front.
/// SQLite only opens this view, while the real bundle stays exclusively held.
/// Cleanup checks the created identities and rejects unknown directory entries.
struct Probe {
    directory: DirectoryGuard,
    paths: [PathBuf; 3],
    identities: [FileFingerprint; 3],
}

impl Probe {
    fn copy_file(work: &Workspace, source: &mut WindowsExclusiveFile) -> Result<Self, String> {
        let directory = DirectoryGuard::create_new_in(
            &work.guard,
            OsStr::new(&format!("restore.probe-{}", token())),
        )
        .map_err(io)?;
        let paths = companions(&directory.path().join("library.sqlite3"));
        #[cfg(test)]
        let source_main = source.path().to_path_buf();
        let mut identities = Vec::new();
        for (ordinal, path) in paths.iter().enumerate() {
            let mut copy = if ordinal == 0 {
                #[cfg(test)]
                let result = super::space_fault::probe_copy_with_fault(
                    &work.data,
                    super::space_fault::ProbeKind::File,
                    &source_main,
                    ordinal,
                    path,
                    || source.copy_to(path),
                );
                #[cfg(not(test))]
                let result = source.copy_to(path);
                result.map_err(io)?
            } else {
                WindowsExclusiveFile::create_new(path).map_err(io)?
            };
            identities.push(copy.fingerprint().map_err(io)?);
        }
        Ok(Self {
            directory,
            paths,
            identities: identities.try_into().map_err(|_| "检查副本角色数量无效")?,
        })
    }

    fn copy(work: &Workspace, bundle: &mut ExclusiveBundle) -> Result<Self, String> {
        bundle.check_missing_sidecars().map_err(io)?;
        let directory = DirectoryGuard::create_new_in(
            &work.guard,
            OsStr::new(&format!("restore.probe-{}", token())),
        )
        .map_err(io)?;
        let paths = companions(&directory.path().join("library.sqlite3"));
        #[cfg(test)]
        let source_main = bundle.files_mut()[0]
            .as_ref()
            .map(|source| source.path().to_path_buf());
        let mut identities = Vec::new();
        for (ordinal, path) in paths.iter().enumerate() {
            let mut copy = if ordinal < 2 {
                match bundle.files_mut()[ordinal].as_mut() {
                    Some(source) => {
                        #[cfg(test)]
                        let result = super::space_fault::probe_copy_with_fault(
                            &work.data,
                            super::space_fault::ProbeKind::Bundle,
                            source_main.as_deref().ok_or("检查副本缺少主资料库")?,
                            ordinal,
                            path,
                            || source.copy_to(path),
                        );
                        #[cfg(not(test))]
                        let result = source.copy_to(path);
                        result.map_err(io)?
                    }
                    None if ordinal == 0 => return Err("检查副本缺少主资料库".into()),
                    None => WindowsExclusiveFile::create_new(path).map_err(io)?,
                }
            } else {
                WindowsExclusiveFile::create_new(path).map_err(io)?
            };
            identities.push(copy.fingerprint().map_err(io)?);
        }
        bundle.check_missing_sidecars().map_err(io)?;
        Ok(Self {
            directory,
            paths,
            identities: identities.try_into().map_err(|_| "检查副本角色数量无效")?,
        })
    }

    fn run<T>(self, action: impl FnOnce(&Path) -> Result<T, String>) -> Result<T, String> {
        let result = action(&self.paths[0]);
        let cleanup = self.cleanup();
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => Err(format!("{error}；检查副本已保留：{cleanup}")),
        }
    }

    fn cleanup(self) -> Result<(), String> {
        let mut files = Vec::new();
        for (path, expected) in self.paths.iter().zip(&self.identities) {
            match WindowsExclusiveFile::lock_existing(path) {
                Ok(mut file) => {
                    let actual = file.fingerprint().map_err(io)?;
                    if !same_object(expected, &actual) {
                        return Err("检查副本的对象身份改变，已保留全部副本".into());
                    }
                    files.push((file, actual));
                }
                // SQLite can remove its own disposable WAL/SHM on close.
                Err(error) if error.raw_os_error() == Some(2) => (),
                Err(error) => return Err(io(error)),
            }
        }
        for entry in fs::read_dir(self.directory.path()).map_err(io)? {
            let path = entry.map_err(io)?.path();
            if !self.paths.contains(&path) {
                return Err("检查副本目录出现未知内容，已保留".into());
            }
        }
        for (file, fingerprint) in files {
            file.delete_matching(&fingerprint).map_err(io)?;
        }
        let identity = self.directory.identity().map_err(io)?;
        self.directory.delete_owned_empty(&identity).map_err(io)
    }
}

pub(super) fn fingerprints(
    bundle: &mut ExclusiveBundle,
) -> Result<[Option<FileFingerprint>; 3], String> {
    let mut values = [None, None, None];
    for (value, file) in values.iter_mut().zip(bundle.files_mut()) {
        *value = file
            .as_mut()
            .map(WindowsExclusiveFile::fingerprint)
            .transpose()
            .map_err(io)?;
    }
    bundle.check_missing_sidecars().map_err(io)?;
    Ok(values)
}

pub(super) fn validate_bundle(data_dir: &Path, bundle: &mut ExclusiveBundle) -> Result<(), String> {
    let work = Workspace::open(&data_dir.join("library.sqlite3"), data_dir)?;
    Probe::copy(&work, bundle)?.run(|probe| validate_in_place(probe).map_err(|e| e.to_string()))
}

fn probe_health(path: &Path) -> Result<bool, String> {
    let conn = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(error) if known_sqlite_damage(&error) => return Ok(true),
        Err(error) => return Err(error.to_string()),
    };
    conn.busy_timeout(std::time::Duration::ZERO)
        .map_err(|e| e.to_string())?;
    match conn.query_row(
        "SELECT value FROM app_meta WHERE key='schema_version'",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(value) if supported_marker(&value) => (),
        Ok(value) => return Err(format!("活动资料库版本 {value} 不受支持，拒绝恢复覆盖")),
        Err(error) if known_sqlite_damage(&error) => return Ok(true),
        Err(error) => return Err(format!("无法复核活动资料库版本，拒绝恢复覆盖：{error}")),
    }
    match conn.pragma_update(None, "cache_size", -VALIDATION_CACHE_KIB) {
        Ok(()) => (),
        Err(error) if known_sqlite_damage(&error) => return Ok(true),
        Err(error) => return Err(error.to_string()),
    }
    match conn.query_row("PRAGMA integrity_check(1)", [], |r| r.get::<_, String>(0)) {
        Ok(status) => Ok(status != "ok"),
        Err(error) if known_sqlite_damage(&error) => Ok(true),
        Err(error) => Err(error.to_string()),
    }
}

fn preserve(
    work: &Workspace,
    bundle: &mut ExclusiveBundle,
    dir: &Path,
    prefix: &str,
    parents: &mut Vec<DirectoryGuard>,
) -> Result<([Option<RawRecord>; 3], Vec<WindowsExclusiveFile>), String> {
    if !prefix
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || prefix.is_empty()
    {
        return Err("原件保全前缀无效".into());
    }
    let absolute = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        return Err("原件保全目录需要绝对路径".into());
    };
    // Resolve each component under held non-reparse parents. Do not follow a
    // junction while creating a missing preservation directory.
    let relative = absolute
        .strip_prefix(&work.data)
        .or_else(|_| absolute.strip_prefix(work.guard.path()))
        .map_err(|_| "原件保全目录必须位于数据目录内")?;
    let mut current = work.data.clone();
    if parents.is_empty() {
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err("原件保全目录包含无效路径分量".into());
            };
            let child = current.join(name);
            let guard = match DirectoryGuard::lock_existing(&child) {
                Ok(guard) => guard,
                Err(error)
                    if error.raw_os_error() == Some(2) || error.raw_os_error() == Some(3) =>
                {
                    let parent = parents.last().unwrap_or(&work.guard);
                    DirectoryGuard::create_new_in(parent, name).map_err(io)?
                }
                Err(error) => return Err(io(error)),
            };
            current = child;
            parents.push(guard);
        }
    } else {
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err("原件保全目录包含无效路径分量".into());
            };
            current.push(name);
        }
        let held_path = parents.last().ok_or("原件保全目录守卫缺失")?.path();
        if fs::canonicalize(held_path).map_err(io)? != current {
            return Err("原件保全目录与持续持有的目录身份不一致".into());
        }
    }
    let token = token();
    let mut records = [None, None, None];
    let mut held = Vec::new();
    for (ordinal, label) in ["sqlite3", "wal", "shm"].iter().enumerate() {
        if let Some(source) = bundle.files_mut()[ordinal].as_mut() {
            let original = source.fingerprint().map_err(io)?;
            let path = current.join(format!("{prefix}-{token}.{label}.raw"));
            #[cfg(test)]
            let copy = super::space_fault::raw_copy_with_fault(&current, ordinal, &path, || {
                source.copy_to(&path)
            });
            #[cfg(not(test))]
            let copy = source.copy_to(&path);
            let mut copy = copy.map_err(|error| format!("保全原件 {label} 失败：{error}"))?;
            let before_close = copy.fingerprint().map_err(io)?;
            drop(copy); // settle Windows' close-time last-write timestamp
            let mut copy = WindowsExclusiveFile::lock_existing(&path).map_err(io)?;
            let actual = copy.fingerprint().map_err(io)?;
            if !same_object(&before_close, &actual)
                || actual.size != original.size
                || actual.sha256 != original.sha256
            {
                return Err("原件保全副本身份或内容不一致，已停止恢复".into());
            }
            let relative_path = path
                .strip_prefix(&work.data)
                .map_err(|_| "原件副本越过数据目录")?
                .to_str()
                .ok_or("原件副本路径编码无效")?
                .replace('\\', "/");
            records[ordinal] = Some(RawRecord {
                relative_path,
                fingerprint: actual,
            });
            held.push(copy);
        }
    }
    bundle.check_missing_sidecars().map_err(io)?;
    Ok((records, held))
}

struct Attempt {
    work: Workspace,
    directory: Option<DirectoryGuard>,
    source: ExclusiveBundle,
    originals: Option<ExclusiveBundle>,
    stage: Option<WindowsExclusiveFile>,
    prepared: Option<WindowsExclusiveFile>,
    committed: Option<WindowsExclusiveFile>,
    raw: Vec<WindowsExclusiveFile>,
    raw_directories: Vec<DirectoryGuard>,
    recorded: bool,
}

impl Attempt {
    fn install(
        &mut self,
        preserve_dir: &Path,
        prefix: &str,
        purpose: &RestorePurpose,
        hook: &impl Fn(RestoreStep) -> Result<(), String>,
    ) -> Result<(), String> {
        let directory = self.directory.as_ref().ok_or("恢复目录未持有")?;
        let staged_path = directory.path().join("staged");
        let mut empty = WindowsExclusiveFile::create_new(&staged_path).map_err(io)?;
        let stage_identity = empty.fingerprint().map_err(io)?;
        self.stage = Some(empty);
        #[cfg(test)]
        hook(RestoreStep::StageAllocated)?;
        let probe = Probe::copy(&self.work, &mut self.source)?;
        // Keep creation ownership through all fallible probe preparation. Only
        // SQLite's synchronous VACUUM operation needs the empty target released.
        drop(self.stage.take());
        let snapshot = probe.run(|probe| {
            validate_in_place(probe).map_err(|e| e.to_string())?;
            let conn = Connection::open_with_flags(probe, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| e.to_string())?;
            conn.busy_timeout(std::time::Duration::ZERO)
                .map_err(|e| e.to_string())?;
            let vacuum = || {
                conn.execute_batch(&format!(
                    "PRAGMA synchronous=FULL; VACUUM INTO '{}';",
                    sql_path(&staged_path)
                ))
            };
            #[cfg(test)]
            let result =
                super::space_fault::vacuum_override(&conn, &staged_path).unwrap_or_else(vacuum);
            #[cfg(not(test))]
            let result = vacuum();
            result.map_err(|e| e.to_string())?;
            drop(conn);
            super::sync_file(&staged_path)
        });
        let mut staged = WindowsExclusiveFile::lock_existing(&staged_path).map_err(io)?;
        if !same_object(&stage_identity, &staged.fingerprint().map_err(io)?) {
            return Err("暂存资料库身份改变，已保留全部文件".into());
        }
        self.stage = Some(staged);
        snapshot?;
        hook(RestoreStep::Staged)?;

        let mut originals =
            ExclusiveBundle::lock_optional_database(&self.work.database).map_err(io)?;
        let initial = fingerprints(&mut originals)?;
        purpose.check_target(&initial)?;
        let (mut raw, raw_files) = preserve(
            &self.work,
            &mut originals,
            preserve_dir,
            prefix,
            &mut self.raw_directories,
        )?;
        self.raw.extend(raw_files);
        hook(RestoreStep::Preserved)?;
        if initial[0].is_some() {
            let corrupt = Probe::copy(&self.work, &mut originals)?.run(probe_health)?;
            if matches!(purpose, RestorePurpose::RecoverCorrupt) && !corrupt {
                return Err("自动恢复最终复核未确认损坏，已保留当前资料库与备份候选".into());
            }
        }
        // This deliberately precedes the final acquisition. A writer that
        // repairs the library here is observed by the new guarded copy below.
        drop(originals);
        if initial[0].is_some() {
            hook(RestoreStep::Probed)?;
        }
        let mut originals =
            ExclusiveBundle::lock_optional_database(&self.work.database).map_err(io)?;
        let current = fingerprints(&mut originals)?;
        purpose.check_target(&current)?;
        if current != initial {
            let (updated, raw_files) = preserve(
                &self.work,
                &mut originals,
                preserve_dir,
                prefix,
                &mut self.raw_directories,
            )?;
            raw = updated;
            self.raw.extend(raw_files);
        }
        if current[0].is_some() {
            let corrupt = Probe::copy(&self.work, &mut originals)?.run(probe_health)?;
            if matches!(purpose, RestorePurpose::RecoverCorrupt) && !corrupt {
                return Err("自动恢复原库锁内复核未确认损坏，已保留当前资料库与备份候选".into());
            }
            if !corrupt {
                Probe::copy(&self.work, &mut originals)?
                    .run(|probe| validate_in_place(probe).map_err(|e| e.to_string()))?;
            }
            #[cfg(test)]
            if corrupt {
                hook(RestoreStep::WriteChecked)?;
            }
        }
        let staged_fingerprint = self
            .stage
            .as_mut()
            .ok_or("恢复暂存句柄缺失")?
            .fingerprint()
            .map_err(io)?;
        let prepared = Prepared {
            token: token(),
            database_name: self
                .work
                .database
                .file_name()
                .and_then(OsStr::to_str)
                .ok_or("资料库名称编码无效")?
                .to_owned(),
            data_directory: self.work.guard.identity().map_err(io)?,
            transaction_directory: directory.identity().map_err(io)?,
            mode: purpose.mode(),
            originals: current,
            staged: staged_fingerprint,
            pending: if matches!(purpose, RestorePurpose::ApplyPending) {
                Some(fingerprints(&mut self.source)?)
            } else {
                None
            },
            raw,
        };
        let prepared_bytes = restore_journal::encode_prepared(&prepared)?;
        let commit_bytes = restore_journal::encode_commit(&prepared)?;
        self.prepared = Some(
            WindowsExclusiveFile::create_new(&directory.path().join("PREPARED.json"))
                .map_err(io)?,
        );
        self.prepared
            .as_mut()
            .ok_or("准备记录句柄缺失")?
            .write_created(&prepared_bytes)
            .map_err(io)?;
        self.recorded = true;
        #[cfg(test)]
        hook(RestoreStep::Prepared)?;
        originals.check_missing_sidecars().map_err(io)?;
        for (original, label) in originals.files_mut().iter_mut().zip(LABELS) {
            if let Some(original) = original {
                original
                    .rename_no_replace(&directory.path().join(label))
                    .map_err(io)?;
                #[cfg(test)]
                hook(RestoreStep::OriginalReserved(
                    LABELS
                        .iter()
                        .position(|role| *role == label)
                        .expect("fixed original role"),
                ))?;
            }
        }
        self.originals = Some(originals);
        hook(RestoreStep::Reserved)?;
        for path in companions(&self.work.database) {
            absent(&path)?;
        }
        #[cfg(test)]
        hook(RestoreStep::InstallReady)?;
        self.stage
            .as_mut()
            .ok_or("恢复暂存句柄缺失")?
            .rename_no_replace(&self.work.database)
            .map_err(io)?;
        hook(RestoreStep::Installed)?;
        for path in companions(&self.work.database).iter().skip(1) {
            absent(path)?;
        }
        // The staged object remains exclusively held at its new active name.
        // Validate a copy, including the complete ImageLore business schema.
        Probe::copy_file(&self.work, self.stage.as_mut().ok_or("恢复安装句柄缺失")?)?
            .run(|probe| validate_in_place(probe).map_err(|e| e.to_string()))?;
        if self
            .stage
            .as_mut()
            .ok_or("恢复安装句柄缺失")?
            .fingerprint()
            .map_err(io)?
            != prepared.staged
        {
            return Err("安装对象改变，已保留事务和原件".into());
        }
        let mut commit =
            WindowsExclusiveFile::create_new(&directory.path().join("COMMITTED")).map_err(io)?;
        commit.write_created(&commit_bytes).map_err(io)?;
        self.committed = Some(commit);
        #[cfg(test)]
        hook(RestoreStep::Committed)?;
        Ok(())
    }

    fn clean_unrecorded(&mut self) -> Result<(), String> {
        if let Some(file) = self.stage.take() {
            // No original has moved yet; this is only our create-new stage.
            let mut file = file;
            let fp = file.fingerprint().map_err(io)?;
            file.delete_matching(&fp).map_err(io)?;
        }
        if let Some(file) = self.prepared.take() {
            file.delete_owned().map_err(io)?;
        }
        if let Some(directory) = self.directory.take() {
            let identity = directory.identity().map_err(io)?;
            directory.delete_owned_empty(&identity).map_err(io)?;
        }
        Ok(())
    }
}

pub(super) fn replace(
    database: &Path,
    data_dir: &Path,
    source: &Path,
    preserve_dir: &Path,
    prefix: &str,
    purpose: RestorePurpose,
    hook: impl Fn(RestoreStep) -> Result<(), String>,
) -> Result<(), String> {
    let target = purpose.target();
    super::guarded_resume::finish_target(database, data_dir, target)?;
    let work = Workspace::open(database, data_dir)?;
    target.validate_database_name(
        work.database
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or("资料库名称无效")?,
    )?;
    let _source_parents = DirectoryGuard::lock_ancestors(source).map_err(io)?;
    if matches!(
        purpose,
        RestorePurpose::ApplyPending | RestorePurpose::LegacyPromote { .. }
    ) {
        let source_parent =
            fs::canonicalize(source.parent().ok_or("恢复候选缺少父目录")?).map_err(io)?;
        let expected = if matches!(purpose, RestorePurpose::ApplyPending) {
            "restore.pending.sqlite3"
        } else {
            "restore.pending.previous.sqlite3"
        };
        if source_parent.join(source.file_name().ok_or("恢复候选名称无效")?)
            != work.data.join(expected)
        {
            return Err("恢复候选不属于当前操作的固定名称".into());
        }
    }
    let mut source = ExclusiveBundle::lock_database(source).map_err(io)?;
    if let RestorePurpose::LegacyPromote { expected_source } = &purpose {
        if fingerprints(&mut source)? != **expected_source {
            return Err("旧候选在提升前发生变化，已保留双方".into());
        }
    }
    let directory =
        DirectoryGuard::create_new_in(&work.guard, OsStr::new(target.transaction_name()))
            .map_err(io)?;
    let mut attempt = Attempt {
        work,
        directory: Some(directory),
        source,
        originals: None,
        stage: None,
        prepared: None,
        committed: None,
        raw: Vec::new(),
        raw_directories: Vec::new(),
        recorded: false,
    };
    let result = attempt.install(preserve_dir, prefix, &purpose, &hook);
    if !attempt.recorded {
        let cleanup = attempt.clean_unrecorded();
        drop(attempt);
        return match (result, cleanup) {
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(format!("{error}；未移动原件，暂存文件已保留：{cleanup}"))
            }
            (Ok(()), _) => Err("恢复事务未写入准备记录".into()),
        };
    }
    // Reacquire and recheck the complete recorded state. A new commit in the
    // gap must cause preservation, not rollback or cleanup of changed content.
    drop(attempt);
    #[cfg(test)]
    let result = {
        let released = hook(RestoreStep::Released);
        match (result, released) {
            (Err(error), _) | (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    };
    let finish = super::guarded_resume::finish_target(database, data_dir, target);
    match (result, finish) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("恢复已提交，清理待完成：{error}")),
        (Err(error), Err(finish)) => Err(format!("{error}；已保留事务供安全重试：{finish}")),
    }
}
