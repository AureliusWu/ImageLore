use super::super::owned_handle::{FileFingerprint, WindowsExclusiveFile};
use super::super::{guarded_restore, guarded_resume, operation_lock, restore_journal};
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    os::windows::{fs::MetadataExt, process::CommandExt},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const CHILD_TEST: &str = "backup::tests::restore_process_exit_tests::restore_process_child";
const CHILD_SPEC: &str = "IMAGELORE_RESTORE_PROCESS_SPEC";
const READY_LIMIT: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Purpose {
    ApplyPending,
    StagePending,
    RecoverCorrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Point {
    Prepared,
    ReservedDatabase,
    ReservedWal,
    Reserved,
    Installed,
    RestoredDatabase,
    RestoredWal,
    Committed,
    OriginalRemoved,
    PendingRemoved,
    PreparedRemoved,
}

impl Point {
    fn committed(self) -> bool {
        matches!(
            self,
            Self::Committed | Self::OriginalRemoved | Self::PendingRemoved | Self::PreparedRemoved
        )
    }

    fn restore_step(self) -> Option<RestoreStep> {
        match self {
            Self::Prepared => Some(RestoreStep::Prepared),
            Self::ReservedDatabase => Some(RestoreStep::OriginalReserved(0)),
            Self::ReservedWal => Some(RestoreStep::OriginalReserved(1)),
            Self::Reserved => Some(RestoreStep::Reserved),
            Self::Installed => Some(RestoreStep::Installed),
            Self::Committed => Some(RestoreStep::Committed),
            _ => None,
        }
    }

    fn resume_step(self) -> Option<guarded_resume::ResumeStep> {
        match self {
            Self::OriginalRemoved => Some(guarded_resume::ResumeStep::Original),
            Self::PendingRemoved => Some(guarded_resume::ResumeStep::Pending),
            Self::PreparedRemoved => Some(guarded_resume::ResumeStep::Prepared),
            Self::RestoredDatabase => Some(guarded_resume::ResumeStep::Restored(0)),
            Self::RestoredWal => Some(guarded_resume::ResumeStep::Restored(1)),
            _ => None,
        }
    }

    fn rollback_injected(self) -> bool {
        matches!(self, Self::RestoredDatabase | Self::RestoredWal)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessSpec {
    root: PathBuf,
    nonce: String,
    purpose: Purpose,
    point: Point,
}

impl ProcessSpec {
    fn data(&self) -> PathBuf {
        self.root.join("data")
    }
    fn active(&self) -> PathBuf {
        self.data().join("library.sqlite3")
    }
    fn pending(&self) -> PathBuf {
        self.data().join("restore.pending.sqlite3")
    }
    fn source(&self) -> PathBuf {
        match self.purpose {
            Purpose::ApplyPending => self.pending(),
            Purpose::StagePending | Purpose::RecoverCorrupt => {
                self.data().join("backups/selected.sqlite3")
            }
        }
    }
    fn target(&self) -> PathBuf {
        match self.purpose {
            Purpose::ApplyPending | Purpose::RecoverCorrupt => self.active(),
            Purpose::StagePending => self.pending(),
        }
    }
    fn transaction(&self) -> PathBuf {
        self.data().join(match self.purpose {
            Purpose::ApplyPending | Purpose::RecoverCorrupt => "restore.transaction",
            Purpose::StagePending => "restore.pending.transaction",
        })
    }
    fn target_name(&self) -> &'static str {
        match self.purpose {
            Purpose::ApplyPending | Purpose::RecoverCorrupt => "library.sqlite3",
            Purpose::StagePending => "restore.pending.sqlite3",
        }
    }
    fn ownership(&self) -> Vec<u8> {
        format!(
            "synthetic restore process interruption fixture\n{}\n",
            self.nonce
        )
        .into_bytes()
    }
}

struct ProcessFixture {
    spec: ProcessSpec,
    temporary: PathBuf,
}

impl ProcessFixture {
    fn new(purpose: Purpose, point: Point) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        let nonce = format!(
            "{}-{}-{}",
            std::process::id(),
            now_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let root = temporary.join(format!("imagelore-restore-process-{nonce}"));
        fs::create_dir(&root).unwrap();
        let spec = ProcessSpec {
            root,
            nonce,
            purpose,
            point,
        };
        process_write(&spec.root.join("fixture.marker"), &spec.ownership());
        fs::create_dir(spec.data()).unwrap();
        fs::create_dir(spec.data().join("backups")).unwrap();
        drop(operation_lock::acquire(&spec.data()).unwrap());
        let fixture = Self { spec, temporary };
        fixture.assert_owned();
        fixture
    }

    fn assert_owned(&self) {
        assert_process_fixture(&self.spec);
        assert_eq!(
            fs::canonicalize(std::env::temp_dir()).unwrap(),
            self.temporary
        );
    }

    fn cleanup(self) {
        fn ordinary_tree(path: &Path) {
            let metadata = fs::symlink_metadata(path).unwrap();
            assert_eq!(metadata.file_attributes() & 0x400, 0);
            assert!(metadata.is_dir() || metadata.is_file());
            if metadata.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    ordinary_tree(&entry.unwrap().path());
                }
            }
        }
        self.assert_owned();
        ordinary_tree(&self.spec.root);
        assert_eq!(fs::canonicalize(&self.spec.root).unwrap(), self.spec.root);
        fs::remove_dir_all(self.spec.root).unwrap();
    }
}

fn assert_process_fixture(spec: &ProcessSpec) {
    let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
    let metadata = fs::symlink_metadata(&spec.root).unwrap();
    assert!(metadata.is_dir());
    assert_eq!(metadata.file_attributes() & 0x400, 0);
    assert!(spec.root.is_absolute());
    assert_eq!(fs::canonicalize(&spec.root).unwrap(), spec.root);
    assert_eq!(spec.root.parent(), Some(temporary.as_path()));
    assert!(spec
        .root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("imagelore-restore-process-"));
    assert_eq!(
        fs::read(spec.root.join("fixture.marker")).unwrap(),
        spec.ownership()
    );
    let data = fs::symlink_metadata(spec.data()).unwrap();
    assert!(data.is_dir());
    assert_eq!(data.file_attributes() & 0x400, 0);
    assert_eq!(fs::canonicalize(spec.data()).unwrap(), spec.data());
}

fn process_write(path: &Path, bytes: &[u8]) {
    let mut file = WindowsExclusiveFile::create_new(path).unwrap();
    file.write_created(bytes).unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessProof {
    bytes: Vec<u8>,
    modified: SystemTime,
    fingerprint: FileFingerprint,
}

fn process_proof(path: &Path) -> ProcessProof {
    ProcessProof {
        bytes: fs::read(path).unwrap(),
        modified: fs::metadata(path).unwrap().modified().unwrap(),
        fingerprint: WindowsExclusiveFile::lock_existing(path)
            .unwrap()
            .fingerprint()
            .unwrap(),
    }
}

fn process_cohort(path: &Path) -> [Option<ProcessProof>; 3] {
    companions(path).map(|path| match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            assert!(metadata.is_file());
            assert_eq!(metadata.file_attributes() & 0x400, 0);
            Some(process_proof(&path))
        }
        Err(error) if error.raw_os_error() == Some(2) => None,
        Err(error) => panic!("cannot inspect {}: {error}", path.display()),
    })
}

#[derive(Debug, PartialEq, Eq)]
struct ProcessBusiness {
    marker: String,
    extra_asset: String,
    asset_count: i64,
    prompt: String,
}

// SQLite opens only a disposable copy outside dataDir. Reading a real WAL
// cohort directly could rebuild its SHM and invalidate preservation assertions.
fn process_business_copy(spec: &ProcessSpec, paths: &[Option<PathBuf>; 3]) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = spec.root.join(format!(
        "business-probe-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let copies = companions(&directory.join("library.sqlite3"));
    for ordinal in 0..2 {
        if let Some(source) = &paths[ordinal] {
            let mut input = WindowsExclusiveFile::lock_existing(source).unwrap();
            drop(input.copy_to(&copies[ordinal]).unwrap());
        }
    }
    assert!(paths[0].is_some());
    copies[0].clone()
}

fn read_process_business(conn: &Connection) -> ProcessBusiness {
    ProcessBusiness {
        marker: conn
            .query_row("SELECT value FROM marker", [], |row| row.get(0))
            .unwrap(),
        extra_asset: conn
            .query_row("SELECT name FROM assets WHERE id=2", [], |row| row.get(0))
            .unwrap(),
        asset_count: conn
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .unwrap(),
        prompt: conn
            .query_row(
                "SELECT prompt FROM prompt_state WHERE asset_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap(),
    }
}

fn process_business(spec: &ProcessSpec, paths: &[Option<PathBuf>; 3]) -> ProcessBusiness {
    let copy = process_business_copy(spec, paths);
    validate_in_place(&copy).unwrap();
    let conn = Connection::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    read_process_business(&conn)
}

// Recovery's original and raw bytes intentionally contain one damaged extra
// index leaf. They must remain schema-11/readable business data, while an
// actual SQLite integrity result proves physical damage instead of NOTADB.
fn process_corrupt_business(spec: &ProcessSpec, paths: &[Option<PathBuf>; 3]) -> ProcessBusiness {
    let copy = process_business_copy(spec, paths);
    let conn = Connection::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_meta WHERE key='schema_version'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "11"
    );
    let integrity: String = conn
        .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
        .unwrap();
    assert_ne!(integrity, "ok");
    assert!(
        integrity.contains("process_integrity_index"),
        "unexpected damage: {integrity}"
    );
    let business = read_process_business(&conn);
    drop(conn);
    assert!(matches!(
        validate_in_place(&copy),
        Err(ValidationFailure::Rejected(_))
    ));
    business
}

fn business_at(spec: &ProcessSpec, path: &Path) -> ProcessBusiness {
    let paths = companions(path).map(|path| path.try_exists().unwrap().then_some(path));
    process_business(spec, &paths)
}

fn corrupt_business_at(spec: &ProcessSpec, path: &Path) -> ProcessBusiness {
    let paths = companions(path).map(|path| path.try_exists().unwrap().then_some(path));
    process_corrupt_business(spec, &paths)
}

fn assert_business(business: &ProcessBusiness, marker: &str, extra: &str) {
    assert_eq!(business.marker, marker);
    assert_eq!(business.extra_asset, extra);
    assert_eq!(business.asset_count, 2);
    assert_eq!(business.prompt, "sentinel prompt");
}

// Copy an actually committed, uncheckpointed SQLite WAL while its sole source
// connection is idle. Close that source before the transaction child starts;
// the independent DB/WAL/SHM copies remain the real test cohort.
fn seed_process_wal(spec: &ProcessSpec, target: &Path, label: &str, marker: &str, extra: &str) {
    let staging = spec.root.join(format!("wal-seed-{label}.sqlite3"));
    let conn = seed(&staging, "marker before WAL commit");
    let damage_index = if spec.purpose == Purpose::RecoverCorrupt
        && target == spec.active().as_path()
    {
        conn.execute_batch("CREATE TABLE process_integrity_sentinel(value INTEGER); INSERT INTO process_integrity_sentinel VALUES(123); CREATE INDEX process_integrity_index ON process_integrity_sentinel(value);").unwrap();
        let page_size: usize = conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .unwrap();
        let root_page: usize = conn
            .query_row(
                "SELECT rootpage FROM sqlite_master WHERE name='process_integrity_index'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        Some((page_size, root_page))
    } else {
        None
    };
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; BEGIN IMMEDIATE;")
        .unwrap();
    conn.execute("UPDATE marker SET value=?1", [marker])
        .unwrap();
    pending_stage_insert_extra_asset(&conn, extra);
    conn.execute_batch("COMMIT;").unwrap();
    let from = companions(&staging);
    let to = companions(target);
    for (source, destination) in from.iter().zip(&to) {
        assert!(source.is_file());
        assert!(!destination.exists());
        assert!(fs::metadata(source).unwrap().len() > 0);
        fs::copy(source, destination).unwrap();
    }
    drop(conn);
    for path in from {
        if path.try_exists().unwrap() {
            fs::remove_file(path).unwrap();
        }
    }
    assert!(process_cohort(target).iter().all(Option::is_some));
    assert!(
        fs::metadata(&to[1]).unwrap().len() > 32,
        "the copied WAL must contain committed frames"
    );
    // Prove the expected business change still needs WAL replay: a main-only
    // disposable copy has the old marker and lacks the newly committed asset.
    let main_only = spec.root.join(format!("main-only-{label}"));
    fs::create_dir(&main_only).unwrap();
    let main_only_database = main_only.join("library.sqlite3");
    let mut main = WindowsExclusiveFile::lock_existing(target).unwrap();
    drop(main.copy_to(&main_only_database).unwrap());
    drop(main);
    let without_wal =
        Connection::open_with_flags(&main_only_database, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .unwrap();
    assert_eq!(
        without_wal
            .query_row("SELECT value FROM marker", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "marker before WAL commit"
    );
    assert_eq!(
        without_wal
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(without_wal);
    assert_business(&business_at(spec, target), marker, extra);
    if let Some((page_size, root_page)) = damage_index {
        // The extra index was fully committed before WAL mode. Prove no WAL
        // frame shadows its leaf, then alter only its single-row index payload.
        // The application tables and schema marker remain readable.
        let wal = fs::read(&to[1]).unwrap();
        assert_eq!(
            u32::from_be_bytes(wal[8..12].try_into().unwrap()) as usize,
            page_size
        );
        let frames = wal[32..].chunks_exact(page_size + 24);
        assert!(frames.remainder().is_empty());
        for frame in frames {
            assert_ne!(
                u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize,
                root_page
            );
        }
        let before = process_cohort(target);
        let main = fs::read(target).unwrap();
        let page = (root_page - 1) * page_size;
        assert_eq!(main[page], 10, "the independent extra index is a leaf");
        let cell = u16::from_be_bytes([main[page + 8], main[page + 9]]) as usize;
        assert_eq!(&main[page + cell..page + cell + 5], &[4, 3, 1, 9, 123]);
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = fs::OpenOptions::new().write(true).open(target).unwrap();
            file.seek(SeekFrom::Start((page + cell + 4) as u64))
                .unwrap();
            file.write_all(&[124]).unwrap();
            file.sync_all().unwrap();
        }
        let damaged = process_cohort(target);
        let mut expected_main = main;
        expected_main[page + cell + 4] = 124;
        assert_eq!(damaged[0].as_ref().unwrap().bytes, expected_main);
        let old_main = &before[0].as_ref().unwrap().fingerprint;
        let damaged_main = &damaged[0].as_ref().unwrap().fingerprint;
        assert_eq!(damaged_main.volume_serial, old_main.volume_serial);
        assert_eq!(damaged_main.file_id, old_main.file_id);
        assert_eq!(damaged_main.size, old_main.size);
        assert_ne!(damaged_main.sha256, old_main.sha256);
        assert_eq!(&damaged[1..], &before[1..]);
        assert_business(&corrupt_business_at(spec, target), marker, extra);
        assert_eq!(
            process_cohort(target),
            damaged,
            "the corrupt oracle may open only its copy"
        );
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    nonce: String,
    pid: u32,
    purpose: Purpose,
    point: Point,
}

fn process_ready_and_park(spec: &ProcessSpec) -> ! {
    let ready = Ready {
        nonce: spec.nonce.clone(),
        pid: std::process::id(),
        purpose: spec.purpose,
        point: spec.point,
    };
    let bytes = serde_json::to_vec(&ready).unwrap();
    assert!(bytes.len() < READY_LIMIT);
    let mut file = WindowsExclusiveFile::create_new(&spec.root.join("ready.next")).unwrap();
    file.write_created(&bytes).unwrap();
    file.rename_no_replace(&spec.root.join("ready.json"))
        .unwrap();
    drop(file);
    // This never returns, unwinds, or requests a rollback. The parent forcibly
    // kills this still-live process while the production transaction holds files.
    loop {
        std::thread::park();
    }
}

#[test]
fn restore_process_child() {
    let Some(path) = std::env::var_os(CHILD_SPEC) else {
        return;
    };
    let path = PathBuf::from(path);
    let bytes = fs::read(&path).unwrap();
    assert!(bytes.len() <= READY_LIMIT);
    let spec: ProcessSpec = serde_json::from_slice(&bytes).unwrap();
    assert_process_fixture(&spec);
    assert_eq!(fs::canonicalize(path.parent().unwrap()).unwrap(), spec.root);
    assert_eq!(path.file_name().unwrap(), "child-spec.json");
    assert!(spec.point != Point::PendingRemoved || spec.purpose == Purpose::ApplyPending);
    let _operation = operation_lock::acquire(&spec.data()).unwrap();
    let transaction = spec.transaction();
    let run = || {
        guarded_restore::replace(
            &spec.target(),
            &spec.data(),
            &spec.source(),
            &spec.data().join("recovery/process-interruption"),
            "process-interruption",
            match spec.purpose {
                Purpose::ApplyPending => guarded_restore::RestorePurpose::ApplyPending,
                Purpose::StagePending => guarded_restore::RestorePurpose::StagePending,
                Purpose::RecoverCorrupt => guarded_restore::RestorePurpose::RecoverCorrupt,
            },
            |step| {
                if spec.point.rollback_injected() && step == RestoreStep::Installed {
                    // Enter real rollback. The measured crash happens later,
                    // only after an actual successful original-role rename.
                    return Err("injected failure before real partial rollback".into());
                }
                if spec.point.restore_step() == Some(step) {
                    process_ready_and_park(&spec);
                }
                Ok(())
            },
        )
    };
    let result = if let Some(step) = spec.point.resume_step() {
        let signal = spec.clone();
        guarded_resume::with_resume_hook(
            &transaction,
            step,
            move || process_ready_and_park(&signal),
            run,
        )
    } else {
        run()
    };
    panic!("transaction returned before the requested kill boundary: {result:?}");
}

struct ReapChild {
    child: Child,
    reaped: bool,
}

impl Drop for ReapChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < deadline {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    let _ = self.child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn child_logs(spec: &ProcessSpec) -> String {
    format!(
        "stdout={}\nstderr={}\nfixture={}",
        fs::read_to_string(spec.root.join("child.stdout")).unwrap_or_default(),
        fs::read_to_string(spec.root.join("child.stderr")).unwrap_or_default(),
        spec.root.display()
    )
}

fn force_child_at_boundary(spec: &ProcessSpec) -> ExitStatus {
    process_write(
        &spec.root.join("child-spec.json"),
        &serde_json::to_vec(spec).unwrap(),
    );
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(spec.root.join("child.stdout"))
        .unwrap();
    let errors = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(spec.root.join("child.stderr"))
        .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
        .env(CHILD_SPEC, spec.root.join("child-spec.json"))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(errors))
        .creation_flags(0x0800_0000)
        .spawn()
        .unwrap();
    let mut child = ReapChild {
        child,
        reaped: false,
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.child.try_wait().unwrap() {
            child.reaped = true;
            panic!(
                "child exited before ready: {status:?}; {}",
                child_logs(spec)
            );
        }
        if spec.root.join("ready.json").try_exists().unwrap() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "child ready timeout; {}",
            child_logs(spec)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready_bytes = fs::read(spec.root.join("ready.json")).unwrap();
    assert!(ready_bytes.len() <= READY_LIMIT);
    let ready: Ready = serde_json::from_slice(&ready_bytes).unwrap();
    assert_eq!(ready.nonce, spec.nonce);
    assert_eq!(ready.pid, child.child.id());
    assert_eq!(ready.purpose, spec.purpose);
    assert_eq!(ready.point, spec.point);
    assert!(
        child.child.try_wait().unwrap().is_none(),
        "ready must describe a live child"
    );
    let busy = operation_lock::acquire(&spec.data()).unwrap_err();
    assert!(
        busy.contains("操作忙"),
        "child must still hold the real operation guard: {busy}"
    );
    // std::process::Child::kill forcibly terminates the Windows child. No
    // process::exit, panic, callback Err, or Rust Drop simulates this boundary.
    child.child.kill().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "terminated child did not exit; {}",
            child_logs(spec)
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let waited = child.child.wait().unwrap();
    child.reaped = true;
    assert_eq!(status.code(), waited.code());
    assert!(
        !status.success(),
        "forced termination must have an unsuccessful actual exit status"
    );
    assert!(
        status.code().is_some(),
        "capture the actual Windows termination exit code"
    );
    println!(
        "forced Windows child termination: purpose={:?}, point={:?}, pid={}, exit={:?}",
        spec.purpose,
        spec.point,
        ready.pid,
        status.code()
    );
    status
}

fn interrupted_record(spec: &ProcessSpec) -> restore_journal::Prepared {
    let prepared = spec.transaction().join("PREPARED.json");
    let committed = spec.transaction().join("COMMITTED");
    assert_eq!(committed.exists(), spec.point.committed());
    let record = if prepared.exists() {
        let record =
            restore_journal::decode_prepared(&fs::read(prepared).unwrap(), spec.target_name())
                .unwrap();
        if committed.exists() {
            assert_eq!(
                restore_journal::decode_commit(&fs::read(committed).unwrap(), spec.target_name())
                    .unwrap(),
                record
            );
        }
        record
    } else {
        assert_eq!(spec.point, Point::PreparedRemoved);
        restore_journal::decode_commit(&fs::read(committed).unwrap(), spec.target_name()).unwrap()
    };
    assert_eq!(
        record.mode,
        match spec.purpose {
            Purpose::ApplyPending => restore_journal::Mode::Pending,
            Purpose::StagePending => restore_journal::Mode::StagePending,
            Purpose::RecoverCorrupt => restore_journal::Mode::Recovery,
        }
    );
    record
}

fn assert_boundary_state(
    spec: &ProcessSpec,
    record: &restore_journal::Prepared,
    original: &[Option<ProcessProof>; 3],
) {
    let target = process_cohort(&spec.target());
    let reserved = ["database", "wal", "shm"].map(|label| spec.transaction().join(label));
    match spec.point {
        Point::Prepared => {
            assert_eq!(&target, original);
            assert!(reserved.iter().all(|path| !path.exists()));
            assert_eq!(
                process_proof(&spec.transaction().join("staged")).fingerprint,
                record.staged
            );
        }
        Point::Reserved => {
            assert!(target.iter().all(Option::is_none));
            for (path, expected) in reserved.iter().zip(original) {
                assert_eq!(process_proof(path), *expected.as_ref().unwrap());
            }
            assert_eq!(
                process_proof(&spec.transaction().join("staged")).fingerprint,
                record.staged
            );
        }
        Point::ReservedDatabase | Point::ReservedWal => {
            let moved = if spec.point == Point::ReservedDatabase {
                1
            } else {
                2
            };
            for (ordinal, (path, expected)) in reserved.iter().zip(original).enumerate() {
                if ordinal < moved {
                    assert!(target[ordinal].is_none());
                    assert_eq!(process_proof(path), *expected.as_ref().unwrap());
                } else {
                    assert_eq!(target[ordinal], *expected);
                    assert!(!path.exists());
                }
            }
            assert_eq!(
                process_proof(&spec.transaction().join("staged")).fingerprint,
                record.staged
            );
        }
        Point::RestoredDatabase | Point::RestoredWal => {
            let restored = if spec.point == Point::RestoredDatabase {
                1
            } else {
                2
            };
            for (ordinal, (path, expected)) in reserved.iter().zip(original).enumerate() {
                if ordinal < restored {
                    assert_eq!(target[ordinal], *expected);
                    assert!(!path.exists());
                } else {
                    assert!(target[ordinal].is_none());
                    assert_eq!(process_proof(path), *expected.as_ref().unwrap());
                }
            }
            assert_eq!(
                process_proof(&spec.transaction().join("staged")).fingerprint,
                record.staged
            );
        }
        _ => {
            assert_eq!(target[0].as_ref().unwrap().fingerprint, record.staged);
            assert!(target[1..].iter().all(Option::is_none));
            for (ordinal, path) in reserved.iter().enumerate() {
                let removed = matches!(spec.point, Point::PendingRemoved | Point::PreparedRemoved)
                    || (spec.point == Point::OriginalRemoved && ordinal == 0);
                if removed {
                    assert!(!path.exists());
                } else {
                    assert_eq!(process_proof(path), *original[ordinal].as_ref().unwrap());
                }
            }
        }
    }
    if spec.point == Point::PendingRemoved {
        let source = process_cohort(&spec.source());
        assert!(source[0].is_none());
        assert!(source[1..].iter().all(Option::is_some));
    }
}

fn run_process_interruption(purpose: Purpose, point: Point) {
    let fixture = ProcessFixture::new(purpose, point);
    let spec = &fixture.spec;
    seed_process_wal(
        spec,
        &spec.active(),
        "active",
        "old active committed WAL",
        "old active extra asset",
    );
    match purpose {
        Purpose::ApplyPending => seed_process_wal(
            spec,
            &spec.pending(),
            "pending",
            "selected pending committed WAL",
            "selected pending extra asset",
        ),
        Purpose::StagePending => {
            seed_process_wal(
                spec,
                &spec.pending(),
                "pending",
                "old pending committed WAL",
                "old pending extra asset",
            );
            seed_process_wal(
                spec,
                &spec.source(),
                "source",
                "selected source committed WAL",
                "selected source extra asset",
            );
        }
        Purpose::RecoverCorrupt => seed_process_wal(
            spec,
            &spec.source(),
            "source",
            "selected source committed WAL",
            "selected source extra asset",
        ),
    }
    let original = process_cohort(&spec.target());
    let source = process_cohort(&spec.source());
    let active = process_cohort(&spec.active());
    let lock = process_proof(&spec.data().join("restore.operation.lock"));
    assert!(original.iter().all(Option::is_some));
    assert!(source.iter().all(Option::is_some));
    let status = force_child_at_boundary(spec);
    assert!(!status.success());
    // Killing the process must release its live operation object without
    // deleting/replacing the persistent lock or needing a stale-lock workaround.
    drop(operation_lock::acquire(&spec.data()).unwrap());
    assert_eq!(
        process_proof(&spec.data().join("restore.operation.lock")),
        lock
    );
    let record = interrupted_record(spec);
    assert_boundary_state(spec, &record, &original);
    let raw_paths = record
        .raw
        .each_ref()
        .map(|raw| raw.as_ref().map(|raw| spec.data().join(&raw.relative_path)));
    let raw_before = raw_paths
        .each_ref()
        .map(|path| path.as_ref().map(|path| process_proof(path)));
    for ordinal in 0..3 {
        let raw = raw_before[ordinal].as_ref().unwrap();
        let old = original[ordinal].as_ref().unwrap();
        assert_eq!(raw.bytes, old.bytes);
        assert_eq!(
            raw.fingerprint,
            record.raw[ordinal].as_ref().unwrap().fingerprint
        );
    }
    let (old_marker, old_extra) = match purpose {
        Purpose::ApplyPending | Purpose::RecoverCorrupt => {
            ("old active committed WAL", "old active extra asset")
        }
        Purpose::StagePending => ("old pending committed WAL", "old pending extra asset"),
    };
    let raw_business = if purpose == Purpose::RecoverCorrupt {
        process_corrupt_business(spec, &raw_paths)
    } else {
        process_business(spec, &raw_paths)
    };
    assert_business(&raw_business, old_marker, old_extra);
    match purpose {
        Purpose::ApplyPending | Purpose::RecoverCorrupt => {
            finish_or_rollback(&spec.active(), &spec.data()).unwrap()
        }
        Purpose::StagePending => finish_pending_stage(&spec.data()).unwrap(),
    }
    assert!(!spec.transaction().exists());
    if point.committed() {
        assert_eq!(
            process_cohort(&spec.target())[0]
                .as_ref()
                .unwrap()
                .fingerprint,
            record.staged
        );
        assert!(process_cohort(&spec.target())[1..]
            .iter()
            .all(Option::is_none));
        if purpose == Purpose::ApplyPending {
            assert!(process_cohort(&spec.source()).iter().all(Option::is_none));
        }
    } else {
        assert_eq!(process_cohort(&spec.target()), original);
        assert_eq!(process_cohort(&spec.source()), source);
        let retained_business = if purpose == Purpose::RecoverCorrupt {
            corrupt_business_at(spec, &spec.target())
        } else {
            business_at(spec, &spec.target())
        };
        assert_business(&retained_business, old_marker, old_extra);
        assert_eq!(
            process_cohort(&spec.target()),
            original,
            "the original-data oracle must leave all roles intact"
        );
    }
    for ordinal in 0..3 {
        assert_eq!(
            process_proof(raw_paths[ordinal].as_ref().unwrap()),
            *raw_before[ordinal].as_ref().unwrap()
        );
    }
    if purpose == Purpose::StagePending {
        assert_eq!(process_cohort(&spec.source()), source);
        assert_eq!(process_cohort(&spec.active()), active);
        if !point.committed() {
            let guard = operation_lock::acquire(&spec.data()).unwrap();
            guarded_restore::replace(
                &spec.pending(),
                &spec.data(),
                &spec.source(),
                &spec.data().join("recovery/retry-stage"),
                "process-retry",
                guarded_restore::RestorePurpose::StagePending,
                |_| Ok(()),
            )
            .unwrap();
            drop(guard);
        }
        assert_business(
            &business_at(spec, &spec.pending()),
            "selected source committed WAL",
            "selected source extra asset",
        );
    }
    if purpose == Purpose::RecoverCorrupt {
        assert_eq!(process_cohort(&spec.source()), source);
        if !point.committed() {
            let restored = recover_latest_valid_backup(
                &spec.active(),
                &spec.data(),
                &spec.data().join("backups"),
                "synthetic real process termination retry of physical index damage",
            )
            .unwrap()
            .expect("normal recovery must select the unchanged valid backup");
            assert_eq!(restored.name, "selected.sqlite3");
            assert_eq!(PathBuf::from(restored.path), spec.source());
        }
        assert_business(
            &business_at(spec, &spec.active()),
            "selected source committed WAL",
            "selected source extra asset",
        );
        assert_eq!(
            process_cohort(&spec.source()),
            source,
            "Recovery never consumes its backup source"
        );
    }
    apply_pending_restore(&spec.active(), &spec.data(), &spec.data().join("backups")).unwrap();
    let (selected_marker, selected_extra) = match purpose {
        Purpose::ApplyPending => (
            "selected pending committed WAL",
            "selected pending extra asset",
        ),
        Purpose::StagePending | Purpose::RecoverCorrupt => (
            "selected source committed WAL",
            "selected source extra asset",
        ),
    };
    assert_business(
        &business_at(spec, &spec.active()),
        selected_marker,
        selected_extra,
    );
    assert!(process_cohort(&spec.pending()).iter().all(Option::is_none));
    assert!(!spec.transaction().exists());
    let installed = process_cohort(&spec.active());
    // A second startup must be idempotent; candidate consumption cannot revive
    // an old choice or require the persistent operation-lock file to be removed.
    finish_pending_stage(&spec.data()).unwrap();
    apply_pending_restore(&spec.active(), &spec.data(), &spec.data().join("backups")).unwrap();
    assert_eq!(process_cohort(&spec.active()), installed);
    assert!(process_cohort(&spec.pending()).iter().all(Option::is_none));
    if matches!(purpose, Purpose::StagePending | Purpose::RecoverCorrupt) {
        assert_eq!(process_cohort(&spec.source()), source);
    }
    for (path, expected) in raw_paths.iter().zip(&raw_before) {
        assert_eq!(
            process_proof(path.as_ref().unwrap()),
            *expected.as_ref().unwrap()
        );
    }
    assert_eq!(
        process_proof(&spec.data().join("restore.operation.lock")),
        lock
    );
    println!("PASS forced-process restore: purpose={purpose:?}, point={point:?}, exit={:?}, real original/source DB-WAL-SHM, complete raw and business assertions", status.code());
    fixture.cleanup();
}

#[test]
fn forced_process_termination_active_apply_resumes_all_durable_boundaries() {
    for point in [
        Point::Prepared,
        Point::ReservedDatabase,
        Point::ReservedWal,
        Point::Reserved,
        Point::Installed,
        Point::RestoredDatabase,
        Point::RestoredWal,
        Point::Committed,
        Point::OriginalRemoved,
        Point::PendingRemoved,
        Point::PreparedRemoved,
    ] {
        run_process_interruption(Purpose::ApplyPending, point);
    }
}

#[test]
fn forced_process_termination_pending_stage_resumes_all_durable_boundaries() {
    for point in [
        Point::Prepared,
        Point::ReservedDatabase,
        Point::ReservedWal,
        Point::Reserved,
        Point::Installed,
        Point::RestoredDatabase,
        Point::RestoredWal,
        Point::Committed,
        Point::OriginalRemoved,
        Point::PreparedRemoved,
    ] {
        run_process_interruption(Purpose::StagePending, point);
    }
}

#[test]
fn forced_process_termination_corrupt_recovery_resumes_all_recorded_boundaries() {
    for point in [
        Point::Prepared,
        Point::ReservedDatabase,
        Point::ReservedWal,
        Point::Reserved,
        Point::Installed,
        Point::RestoredDatabase,
        Point::RestoredWal,
        Point::Committed,
        Point::OriginalRemoved,
        Point::PreparedRemoved,
    ] {
        run_process_interruption(Purpose::RecoverCorrupt, point);
    }
}
