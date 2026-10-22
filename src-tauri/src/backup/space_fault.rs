//! Test-only capacity faults, scoped to an exact object path and current thread.
//! Windows faults follow a real bounded prefix write and flush. SQLite FULL
//! comes from a capped, uniquely named built-in memdb, never a fabricated error.

use rusqlite::{ffi, Connection, ErrorCode, OpenFlags};
use std::{
    cell::RefCell,
    ffi::CString,
    fs::File,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_PARTIAL_BYTES: u64 = 64 * 1024;
const SQLITE_CAP: i64 = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IoKind {
    WriteCreated,
    CopyTo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProbeKind {
    Bundle,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IoObservation {
    pub(super) path: PathBuf,
    pub(super) kind: IoKind,
    pub(super) requested_bytes: u64,
    pub(super) written_bytes: u64,
    pub(super) flushed: bool,
    pub(super) raw_os_error: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SqliteObservation {
    pub(super) staged_path: PathBuf,
    pub(super) effective_cap: i64,
    pub(super) sqlite_code: ErrorCode,
    pub(super) extended_code: i32,
    pub(super) error_message: String,
}

type IoReceipt = Rc<RefCell<Option<IoObservation>>>;
type SqliteReceipt = Rc<RefCell<Option<SqliteObservation>>>;

struct IoFault {
    path: PathBuf,
    kind: IoKind,
    after_bytes: u64,
    receipt: IoReceipt,
}

struct RawFault {
    parent: PathBuf,
    ordinal: usize,
    after_bytes: u64,
    receipt: IoReceipt,
}

struct ProbeCopyFault {
    parent: PathBuf,
    kind: ProbeKind,
    source_main: PathBuf,
    ordinal: usize,
    after_bytes: u64,
    receipt: IoReceipt,
}

struct CreateSqliteFault {
    parent: PathBuf,
    prefix: String,
    receipt: SqliteReceipt,
}

struct SqliteFault {
    staged_path: PathBuf,
    uri: String,
    effective_cap: i64,
    // This live connection retains the same named memdb store during VACUUM.
    _keeper: Connection,
    receipt: SqliteReceipt,
}

thread_local! {
    static IO_FAULT: RefCell<Option<IoFault>> = const { RefCell::new(None) };
    static RAW_FAULT: RefCell<Option<RawFault>> = const { RefCell::new(None) };
    static PROBE_COPY_FAULT: RefCell<Option<ProbeCopyFault>> = const { RefCell::new(None) };
    static CREATE_SQLITE_FAULT: RefCell<Option<CreateSqliteFault>> = const { RefCell::new(None) };
    static SQLITE_FAULT: RefCell<Option<SqliteFault>> = const { RefCell::new(None) };
}

fn exact_path(path: &Path) -> PathBuf {
    assert!(
        path.is_absolute(),
        "capacity fault needs an absolute fixture path"
    );
    assert!(
        path.file_name().is_some(),
        "capacity fault cannot name a volume root"
    );
    assert!(
        path.components()
            .all(|part| !matches!(part, Component::ParentDir | Component::CurDir)),
        "capacity fault needs ordinary components from a canonical fixture root"
    );
    // Do not require this future file or transaction directory to exist. The
    // caller supplies canonical fixture-root spelling; matching is exact Path
    // equality, without prefix, wildcard, basename or case-folded matching.
    path.to_path_buf()
}

fn check_budget(after_bytes: u64) {
    assert!(
        (1..=MAX_PARTIAL_BYTES).contains(&after_bytes),
        "capacity fault prefix must be nonempty and at most 64 KiB"
    );
}

fn io_scope<T>(request: IoFault, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<IoFault>);
    impl Drop for Reset {
        fn drop(&mut self) {
            IO_FAULT.with(|current| current.replace(self.0.take()));
        }
    }
    let _reset = Reset(IO_FAULT.with(|current| current.replace(Some(request))));
    run()
}

pub(super) fn with_io_fault<T>(
    path: &Path,
    kind: IoKind,
    after_bytes: u64,
    run: impl FnOnce() -> T,
) -> (T, Option<IoObservation>) {
    check_budget(after_bytes);
    let receipt = Rc::new(RefCell::new(None));
    let result = io_scope(
        IoFault {
            path: exact_path(path),
            kind,
            after_bytes,
            receipt: receipt.clone(),
        },
        run,
    );
    let observation = receipt.borrow().clone();
    (result, observation)
}

pub(super) fn with_raw_copy_fault<T>(
    exact_parent: &Path,
    ordinal: usize,
    after_bytes: u64,
    run: impl FnOnce() -> T,
) -> (T, Option<IoObservation>) {
    struct Reset(Option<RawFault>);
    impl Drop for Reset {
        fn drop(&mut self) {
            RAW_FAULT.with(|current| current.replace(self.0.take()));
        }
    }
    check_budget(after_bytes);
    assert!(ordinal < 3, "raw fault ordinal is DB=0, WAL=1 or SHM=2");
    let receipt = Rc::new(RefCell::new(None));
    let result = {
        let _reset = Reset(RAW_FAULT.with(|current| {
            current.replace(Some(RawFault {
                parent: exact_path(exact_parent),
                ordinal,
                after_bytes,
                receipt: receipt.clone(),
            }))
        }));
        run()
    };
    let observation = receipt.borrow().clone();
    (result, observation)
}

/// Called only after preserve has resolved a concrete new raw filename under
/// its held parent. The primitive below still matches that exact path + CopyTo.
pub(super) fn raw_copy_with_fault<T>(
    parent: &Path,
    ordinal: usize,
    actual_path: &Path,
    run: impl FnOnce() -> T,
) -> T {
    let request = RAW_FAULT.with(|current| {
        let mut current = current.borrow_mut();
        if current
            .as_ref()
            .is_some_and(|request| request.parent == parent && request.ordinal == ordinal)
        {
            current.take()
        } else {
            None
        }
    });
    let Some(request) = request else {
        return run();
    };
    assert_eq!(actual_path.parent(), Some(parent));
    io_scope(
        IoFault {
            path: exact_path(actual_path),
            kind: IoKind::CopyTo,
            after_bytes: request.after_bytes,
            receipt: request.receipt,
        },
        run,
    )
}

pub(super) fn with_probe_copy_fault<T>(
    exact_parent: &Path,
    kind: ProbeKind,
    exact_source_main: &Path,
    ordinal: usize,
    after_bytes: u64,
    run: impl FnOnce() -> T,
) -> (T, Option<IoObservation>) {
    struct Reset(Option<ProbeCopyFault>);
    impl Drop for Reset {
        fn drop(&mut self) {
            PROBE_COPY_FAULT.with(|current| current.replace(self.0.take()));
        }
    }
    check_budget(after_bytes);
    assert!(
        match kind {
            ProbeKind::Bundle => ordinal < 2,
            ProbeKind::File => ordinal == 0,
        },
        "probe copies DB/WAL for a bundle and DB only for a file"
    );
    let receipt = Rc::new(RefCell::new(None));
    let result = {
        let _reset = Reset(PROBE_COPY_FAULT.with(|current| {
            current.replace(Some(ProbeCopyFault {
                parent: exact_path(exact_parent),
                kind,
                source_main: exact_path(exact_source_main),
                ordinal,
                after_bytes,
                receipt: receipt.clone(),
            }))
        }));
        run()
    };
    let observation = receipt.borrow().clone();
    (result, observation)
}

/// Called after the constructor owns its new random directory. Match the
/// selected source cohort and copy role, then bind the primitive to only the
/// concrete destination path. No call counter or directory scan participates.
pub(super) fn probe_copy_with_fault<T>(
    parent: &Path,
    kind: ProbeKind,
    source_main: &Path,
    ordinal: usize,
    actual_path: &Path,
    run: impl FnOnce() -> T,
) -> T {
    let request = PROBE_COPY_FAULT.with(|current| {
        let mut current = current.borrow_mut();
        if current.as_ref().is_some_and(|request| {
            request.parent == parent
                && request.kind == kind
                && request.source_main == source_main
                && request.ordinal == ordinal
        }) {
            current.take()
        } else {
            None
        }
    });
    let Some(request) = request else {
        return run();
    };
    let probe_dir = actual_path.parent().expect("probe output has a directory");
    assert_eq!(probe_dir.parent(), Some(parent));
    assert_eq!(
        actual_path,
        super::companions(&probe_dir.join("library.sqlite3"))[ordinal]
    );
    io_scope(
        IoFault {
            path: exact_path(actual_path),
            kind: IoKind::CopyTo,
            after_bytes: request.after_bytes,
            receipt: request.receipt,
        },
        run,
    )
}

pub(super) struct IoFaultPlan(IoFault);

pub(super) fn take_io_fault(path: &Path, kind: IoKind) -> Option<IoFaultPlan> {
    IO_FAULT.with(|current| {
        let mut current = current.borrow_mut();
        if current
            .as_ref()
            .is_some_and(|request| request.path == path && request.kind == kind)
        {
            // Consume the exact request before any write, flush or observer.
            current.take().map(IoFaultPlan)
        } else {
            None
        }
    })
}

impl IoFaultPlan {
    fn after_flush(self, output: &File, written: u64, requested: u64) -> io::Result<io::Error> {
        let output_bytes = output.metadata()?.len();
        assert_eq!(written, self.0.after_bytes);
        assert!(
            written < requested,
            "the actual output must be a strict prefix"
        );
        assert_eq!(
            output_bytes, written,
            "observe the actual partial output size"
        );
        let error = io::Error::from_raw_os_error(112);
        *self.0.receipt.borrow_mut() = Some(IoObservation {
            path: self.0.path,
            kind: self.0.kind,
            requested_bytes: requested,
            written_bytes: output_bytes,
            flushed: true,
            raw_os_error: error.raw_os_error().unwrap(),
        });
        Ok(error)
    }

    pub(super) fn write_error(self, output: &mut File, bytes: &[u8]) -> io::Result<io::Error> {
        assert_eq!(self.0.kind, IoKind::WriteCreated);
        assert!(
            self.0.after_bytes < bytes.len() as u64,
            "prefix must be a partial write"
        );
        let count = usize::try_from(self.0.after_bytes).unwrap();
        output.write_all(&bytes[..count])?;
        output.sync_all()?;
        self.after_flush(output, count as u64, bytes.len() as u64)
    }

    pub(super) fn copy_error(self, input: &mut File, output: &mut File) -> io::Result<io::Error> {
        assert_eq!(self.0.kind, IoKind::CopyTo);
        let requested = input.metadata()?.len();
        assert!(
            self.0.after_bytes < requested,
            "prefix must be a partial copy"
        );
        let copied = io::copy(&mut input.take(self.0.after_bytes), output)?;
        output.sync_all()?;
        self.after_flush(output, copied, requested)
    }
}

fn sqlite_request(staged_path: &Path, receipt: SqliteReceipt) -> SqliteFault {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let uri = format!(
        "file:/imagelore-capacity-{:x}-{:x}-{:x}?vfs=memdb",
        std::process::id(),
        super::now_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;
    let keeper = Connection::open_with_flags_and_vfs(&uri, flags, "memdb").unwrap();
    let main = CString::new("main").unwrap();
    let mut effective_cap = SQLITE_CAP;
    // SAFETY: this live connection is used only on the registering thread.
    // CString and sqlite3_int64 storage outlive the synchronous API call. The
    // control affects only this unique named memdb store, never global config.
    let rc = unsafe {
        ffi::sqlite3_file_control(
            keeper.handle(),
            main.as_ptr(),
            ffi::SQLITE_FCNTL_SIZE_LIMIT,
            (&mut effective_cap as *mut i64).cast(),
        )
    };
    assert_eq!(rc, ffi::SQLITE_OK);
    assert_eq!(effective_cap, SQLITE_CAP);
    SqliteFault {
        staged_path: exact_path(staged_path),
        uri,
        effective_cap,
        _keeper: keeper,
        receipt,
    }
}

pub(super) fn with_sqlite_full<T>(
    exact_staged_path: &Path,
    run: impl FnOnce() -> T,
) -> (T, Option<SqliteObservation>) {
    struct Reset(Option<SqliteFault>);
    impl Drop for Reset {
        fn drop(&mut self) {
            SQLITE_FAULT.with(|current| current.replace(self.0.take()));
        }
    }
    let receipt = Rc::new(RefCell::new(None));
    let request = sqlite_request(exact_staged_path, receipt.clone());
    let result = {
        let _reset = Reset(SQLITE_FAULT.with(|current| current.replace(Some(request))));
        run()
    };
    let observation = receipt.borrow().clone();
    (result, observation)
}

pub(super) fn with_create_sqlite_full<T>(
    exact_parent: &Path,
    exact_prefix: &str,
    run: impl FnOnce() -> T,
) -> (T, Option<SqliteObservation>) {
    struct Reset(Option<CreateSqliteFault>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CREATE_SQLITE_FAULT.with(|current| current.replace(self.0.take()));
        }
    }
    assert!(
        !exact_prefix.is_empty() && !exact_prefix.contains(['/', '\\', ':', '\0']),
        "backup fault requires an exact ordinary prefix"
    );
    let receipt = Rc::new(RefCell::new(None));
    let result = {
        let _reset = Reset(CREATE_SQLITE_FAULT.with(|current| {
            current.replace(Some(CreateSqliteFault {
                parent: exact_path(exact_parent),
                prefix: exact_prefix.to_owned(),
                receipt: receipt.clone(),
            }))
        }));
        run()
    };
    let observation = receipt.borrow().clone();
    (result, observation)
}

/// The creator has already chosen its unique destination. Claim only its
/// exact backup parent and requested prefix, then run the unchanged SQLite
/// operation against a private bounded memdb, retaining the real FULL result.
pub(super) fn create_vacuum_override(
    connection: &Connection,
    parent: &Path,
    prefix: &str,
    actual_path: &Path,
) -> Option<rusqlite::Result<()>> {
    let request = CREATE_SQLITE_FAULT.with(|current| {
        let mut current = current.borrow_mut();
        if current
            .as_ref()
            .is_some_and(|request| request.parent == parent && request.prefix == prefix)
        {
            current.take()
        } else {
            None
        }
    })?;
    assert_eq!(actual_path.parent(), Some(parent));
    Some(run_bounded_vacuum(
        connection,
        sqlite_request(actual_path, request.receipt),
        false,
    ))
}

/// Return None for every other path. An exact match runs the real SQLite
/// VACUUM against the bounded store and returns its unmodified typed result.
pub(super) fn vacuum_override(
    connection: &Connection,
    staged_path: &Path,
) -> Option<rusqlite::Result<()>> {
    let request = SQLITE_FAULT.with(|current| {
        let mut current = current.borrow_mut();
        if current
            .as_ref()
            .is_some_and(|request| request.staged_path == staged_path)
        {
            current.take()
        } else {
            None
        }
    })?;
    Some(run_bounded_vacuum(connection, request, true))
}

fn run_bounded_vacuum(
    connection: &Connection,
    request: SqliteFault,
    synchronous_full: bool,
) -> rusqlite::Result<()> {
    // `request` retains its keeper until the real execute_batch has completed.
    // Restore already requests FULL; live backup creation does not change its
    // connection's synchronous setting merely to select this test destination.
    let pragma = if synchronous_full {
        "PRAGMA synchronous=FULL; "
    } else {
        ""
    };
    let result = connection.execute_batch(&format!(
        "{pragma}VACUUM INTO '{}';",
        request.uri.replace('\'', "''")
    ));
    if let Err(error) = &result {
        if let Some(native) = error.sqlite_error() {
            *request.receipt.borrow_mut() = Some(SqliteObservation {
                staged_path: request.staged_path.clone(),
                effective_cap: request.effective_cap,
                sqlite_code: native.code,
                extended_code: native.extended_code,
                error_message: error.to_string(),
            });
        }
    }
    result
}
