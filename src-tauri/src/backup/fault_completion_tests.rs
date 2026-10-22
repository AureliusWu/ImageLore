use super::super::{
    guarded_restore, operation_lock,
    owned_handle::{FileFingerprint, WindowsExclusiveFile},
    restore_journal, space_fault,
};
use super::restore_process_exit_tests::{
    assert_business, business_at, process_cohort, process_proof, Point, ProcessFixture,
    ProcessProof, ProcessSpec, Purpose,
};
use super::space_failure_tests::{
    assert_io_receipt, assert_no_transaction, assert_normal_retry, assert_original_business,
    run_formal_restore, scene, space_fixture,
};
use super::*;
use sha2::{Digest, Sha256};
use space_fault::{IoKind, ProbeKind};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    io::Read,
    mem::size_of,
    os::windows::{fs::MetadataExt, fs::OpenOptionsExt, io::AsRawHandle},
};

const PURPOSES: [Purpose; 3] = [
    Purpose::ApplyPending,
    Purpose::StagePending,
    Purpose::RecoverCorrupt,
];
const PARTIAL_PREFIX: u64 = 31;

#[derive(Debug, PartialEq, Eq)]
struct LiveBusiness {
    marker: String,
    assets: Vec<(i64, String, String, String)>,
    prompts: Vec<(i64, String, String, String, i64)>,
}

fn live_business(conn: &Connection) -> LiveBusiness {
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert!(!conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    LiveBusiness {
        marker: conn
            .query_row("SELECT value FROM marker", [], |row| row.get(0))
            .unwrap(),
        assets: conn
            .prepare("SELECT id,path,name,portable_id FROM assets ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        prompts: conn
            .prepare("SELECT asset_id,prompt,negative_prompt,model,updated_at FROM prompt_state ORDER BY asset_id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
    }
}

#[test]
fn backup_create_real_sqlite_full_keeps_live_wal_and_prior_backup_and_retries() {
    let fixture = ProcessFixture::new(Purpose::ApplyPending, Point::Prepared);
    let spec = &fixture.spec;
    let state = test_state(&spec.data());
    {
        let conn = state.db.lock().unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        conn.execute("UPDATE marker SET value='committed live backup fault'", [])
            .unwrap();
        pending_stage_insert_extra_asset(&conn, "live backup extra asset");
        conn.execute(
            "UPDATE prompt_state SET prompt='committed WAL backup prompt' WHERE asset_id=1",
            [],
        )
        .unwrap();
    }
    let wal = state.database_path.with_extension("sqlite3-wal");
    assert!(fs::metadata(&wal).unwrap().len() > 0);
    // A real successful create is an independent positive control on the same
    // source and destination before selecting the bounded engine operation.
    let prior = create(&state, "capacity-control").unwrap();
    let prior_path = Path::new(&prior.path);
    validate(prior_path).unwrap();
    let prior_proof = process_proof(prior_path);
    let before = live_business(&state.db.lock().unwrap());
    let db_bytes = fs::read(&state.database_path).unwrap();
    let wal_bytes = fs::read(&wal).unwrap();
    let existing_names: Vec<_> = records(&state.backups_dir)
        .unwrap()
        .into_iter()
        .map(|record| record.name)
        .collect();
    let (result, observation) =
        space_fault::with_create_sqlite_full(&state.backups_dir, "capacity-failed", || {
            create(&state, "capacity-failed")
        });
    let error = result.unwrap_err();
    let observation = observation.expect("the real create VACUUM must consume its exact request");
    assert_eq!(observation.effective_cap, 8192);
    assert_eq!(observation.sqlite_code, rusqlite::ErrorCode::DiskFull);
    assert_eq!(observation.extended_code, rusqlite::ffi::SQLITE_FULL);
    assert!(error.contains(&observation.error_message), "{error}");
    assert_eq!(
        observation.staged_path.parent(),
        Some(state.backups_dir.as_path())
    );
    assert!(observation
        .staged_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("capacity-failed-"));
    assert!(!observation.staged_path.exists());
    assert_eq!(process_proof(prior_path), prior_proof);
    assert_eq!(live_business(&state.db.lock().unwrap()), before);
    assert_eq!(fs::read(&state.database_path).unwrap(), db_bytes);
    assert_eq!(fs::read(&wal).unwrap(), wal_bytes);
    assert_eq!(
        records(&state.backups_dir)
            .unwrap()
            .into_iter()
            .map(|record| record.name)
            .collect::<Vec<_>>(),
        existing_names
    );
    let retry = create(&state, "capacity-failed").unwrap();
    assert_ne!(Path::new(&retry.path), observation.staged_path.as_path());
    validate(Path::new(&retry.path)).unwrap();
    assert_eq!(
        live_business(
            &Connection::open_with_flags(&retry.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
        ),
        before
    );
    assert_eq!(process_proof(prior_path), prior_proof);
    assert_eq!(live_business(&state.db.lock().unwrap()), before);
    println!("PASS completion fault: create real SQLite FULL/13, live committed WAL business and previous backup preserved, failed target excluded, formal create retry succeeded");
    drop(state);
    fixture.cleanup();
}

fn assert_probe_partial(
    spec: &ProcessSpec,
    observation: &space_fault::IoObservation,
    ordinal: usize,
    expected_bytes: Option<&[u8]>,
) -> Option<ProcessProof> {
    assert_io_receipt(
        observation,
        &observation.path,
        IoKind::CopyTo,
        PARTIAL_PREFIX,
    );
    let directory = observation.path.parent().unwrap();
    assert_eq!(directory.parent(), Some(spec.data().as_path()));
    assert!(directory
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("restore.probe-"));
    assert_eq!(
        observation.path,
        companions(&directory.join("library.sqlite3"))[ordinal]
    );
    assert_ne!(observation.path, spec.target());
    // Safe cleanup may remove a known incomplete probe. If retained as forensic
    // evidence, its real bytes must be a strict prefix, never a successful DB.
    if observation.path.exists() {
        let partial = process_proof(&observation.path);
        assert_eq!(partial.bytes.len() as u64, PARTIAL_PREFIX);
        if let Some(expected_bytes) = expected_bytes {
            assert_eq!(partial.bytes, expected_bytes[..PARTIAL_PREFIX as usize]);
        }
        assert!(validate(&observation.path).is_err());
        Some(partial)
    } else {
        None
    }
}

#[test]
fn source_probe_partial_copy_keeps_original_and_source_and_formal_retry_succeeds() {
    for purpose in PURPOSES {
        for ordinal in 0..2 {
            let fixture = space_fixture(purpose);
            let spec = &fixture.spec;
            let original = process_cohort(&spec.target());
            let source = process_cohort(&spec.source());
            let active = process_cohort(&spec.active());
            let (result, observation) = space_fault::with_probe_copy_fault(
                &spec.data(),
                ProbeKind::Bundle,
                &spec.source(),
                ordinal,
                PARTIAL_PREFIX,
                || run_formal_restore(spec, purpose),
            );
            let error = result.unwrap_err();
            assert!(error.contains("os error 112"), "{error}");
            let observation = observation.expect("the exact guarded source probe copy must fail");
            let partial = assert_probe_partial(
                spec,
                &observation,
                ordinal,
                Some(&source[ordinal].as_ref().unwrap().bytes),
            );
            assert_no_transaction(spec);
            assert_eq!(process_cohort(&spec.target()), original);
            assert_eq!(process_cohort(&spec.source()), source);
            assert_original_business(spec, purpose);
            assert_eq!(process_cohort(&spec.target()), original);
            assert_normal_retry(spec, purpose, &source, &active);
            if let Some(partial) = partial.filter(|_| observation.path.exists()) {
                assert_eq!(process_proof(&observation.path), partial);
            }
            println!("PASS completion fault: purpose={purpose:?}, source Probe::copy role={ordinal}, real partial copy+sync112, originals/source unchanged and formal retry succeeded");
            fixture.cleanup();
        }
    }
}

#[test]
fn installed_probe_partial_copy_rolls_back_exact_original_and_formal_retry_succeeds() {
    for purpose in PURPOSES {
        let fixture = space_fixture(purpose);
        let spec = &fixture.spec;
        let original = process_cohort(&spec.target());
        let source = process_cohort(&spec.source());
        let active = process_cohort(&spec.active());
        let (result, observation) = space_fault::with_probe_copy_fault(
            &spec.data(),
            ProbeKind::File,
            &spec.target(),
            0,
            PARTIAL_PREFIX,
            || run_formal_restore(spec, purpose),
        );
        let error = result.unwrap_err();
        assert!(error.contains("os error 112"), "{error}");
        let observation = observation.expect("the exact installed-file probe copy must fail");
        let partial = assert_probe_partial(spec, &observation, 0, None);
        assert_no_transaction(spec);
        assert_eq!(process_cohort(&spec.target()), original);
        assert_eq!(process_cohort(&spec.source()), source);
        assert_original_business(spec, purpose);
        assert_eq!(process_cohort(&spec.target()), original);
        assert_normal_retry(spec, purpose, &source, &active);
        if let Some(partial) = partial.filter(|_| observation.path.exists()) {
            assert_eq!(process_proof(&observation.path), partial);
        }
        println!("PASS completion fault: purpose={purpose:?}, installed Probe::copy_file real partial copy+sync112, exact original restored, source retained and formal retry succeeded");
        fixture.cleanup();
    }
}

fn selected_labels(purpose: Purpose) -> (&'static str, &'static str) {
    match purpose {
        Purpose::ApplyPending => (
            "selected pending committed WAL",
            "selected pending extra asset",
        ),
        Purpose::StagePending | Purpose::RecoverCorrupt => (
            "selected source committed WAL",
            "selected source extra asset",
        ),
    }
}

#[test]
fn install_ready_real_rename_conflict_keeps_every_object_and_formal_entries_refuse() {
    for purpose in PURPOSES {
        let fixture = space_fixture(purpose);
        let spec = &fixture.spec;
        let original = process_cohort(&spec.target());
        let source = process_cohort(&spec.source());
        let active = process_cohort(&spec.active());
        let injected = Cell::new(false);
        let foreign = RefCell::new(None);
        let operation = operation_lock::acquire(&spec.data()).unwrap();
        // The fault reaches the same production transaction core used by the
        // outer entries. The following retries use those actual outer entries.
        let result = guarded_restore::replace(
            &spec.target(),
            &spec.data(),
            &spec.source(),
            &spec.data().join("recovery/completion-rename"),
            "actual-rename-conflict",
            match purpose {
                Purpose::ApplyPending => guarded_restore::RestorePurpose::ApplyPending,
                Purpose::StagePending => guarded_restore::RestorePurpose::StagePending,
                Purpose::RecoverCorrupt => guarded_restore::RestorePurpose::RecoverCorrupt,
            },
            |step| {
                if step == RestoreStep::InstallReady {
                    assert!(!injected.replace(true));
                    assert!(companions(&spec.target()).iter().all(|path| !path.exists()));
                    let conn = seed(&spec.target(), "foreign after every absent precheck");
                    pending_stage_insert_extra_asset(&conn, "foreign rename conflict extra asset");
                    drop(conn);
                    *foreign.borrow_mut() = Some(process_cohort(&spec.target()));
                }
                Ok(()) // No callback Err can stand in for the actual rename error.
            },
        );
        drop(operation);
        assert!(injected.get());
        let error = result.unwrap_err();
        assert!(
            [80, 183]
                .iter()
                .any(|code| error.contains(&format!("os error {code}"))),
            "the real no-replace Win32 rename must reject the occupied target: {error}"
        );
        assert_eq!(
            process_cohort(&spec.target()),
            foreign
                .into_inner()
                .expect("the valid foreign target must exist")
        );
        assert_business(
            &business_at(spec, &spec.target()),
            "foreign after every absent precheck",
            "foreign rename conflict extra asset",
        );
        assert_eq!(process_cohort(&spec.source()), source);
        if purpose == Purpose::StagePending {
            assert_eq!(process_cohort(&spec.active()), active);
        }
        let record = restore_journal::decode_prepared(
            &fs::read(spec.transaction().join("PREPARED.json")).unwrap(),
            spec.target_name(),
        )
        .unwrap();
        assert!(!spec.transaction().join("COMMITTED").exists());
        let staged = spec.transaction().join("staged");
        let (marker, extra) = selected_labels(purpose);
        assert_business(&business_at(spec, &staged), marker, extra);
        assert_eq!(
            WindowsExclusiveFile::lock_existing(&staged)
                .unwrap()
                .fingerprint()
                .unwrap(),
            record.staged
        );
        for (ordinal, role) in ["database", "wal", "shm"].into_iter().enumerate() {
            assert_eq!(
                process_proof(&spec.transaction().join(role)),
                *original[ordinal].as_ref().unwrap()
            );
            let raw = record.raw[ordinal].as_ref().unwrap();
            let path = spec.data().join(&raw.relative_path);
            assert_eq!(
                process_proof(&path).bytes,
                original[ordinal].as_ref().unwrap().bytes
            );
            assert_eq!(
                WindowsExclusiveFile::lock_existing(&path)
                    .unwrap()
                    .fingerprint()
                    .unwrap(),
                raw.fingerprint
            );
        }
        let before_reentry = scene(spec);
        for _ in 0..3 {
            assert!(run_formal_restore(spec, purpose).is_err());
            assert_eq!(scene(spec), before_reentry);
        }
        println!("PASS completion fault: purpose={purpose:?}, actual no-replace rename80/183 after all absent prechecks, foreign/staged/original/raw/source preserved and three outer entries refuse");
        fixture.cleanup();
    }
}

// The production proof primitive intentionally requests WRITE+DELETE access,
// so it cannot inspect an intentionally read-only file. This independent test
// oracle requests read access only while retaining an exclusive sharing mode.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
struct ReadOnlyFileId {
    volume_serial: u64,
    file_id: [u8; 16],
}

const _: () = assert!(size_of::<ReadOnlyFileId>() == 24);

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "GetFileInformationByHandleEx"]
    fn query_readonly_file_id(
        handle: *mut c_void,
        class: i32,
        information: *mut c_void,
        size: u32,
    ) -> i32;
}

fn readonly_file_id(file: &File) -> ReadOnlyFileId {
    let mut identity = ReadOnlyFileId {
        volume_serial: 0,
        file_id: [0; 16],
    };
    // SAFETY: File keeps this read-only handle alive for the synchronous call;
    // the initialized 24-byte buffer matches the Windows SDK FILE_ID_INFO.
    let success = unsafe {
        query_readonly_file_id(
            file.as_raw_handle(),
            18,
            (&mut identity as *mut ReadOnlyFileId).cast(),
            u32::try_from(size_of::<ReadOnlyFileId>()).unwrap(),
        )
    };
    assert_ne!(success, 0, "{}", std::io::Error::last_os_error());
    identity
}

#[derive(Debug, PartialEq, Eq)]
struct ReadOnlyProof {
    bytes: Vec<u8>,
    fingerprint: FileFingerprint,
}

fn readonly_proof(path: &Path) -> ReadOnlyProof {
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .custom_flags(0x0020_0000)
        .open(path)
        .unwrap();
    let before = file.metadata().unwrap();
    assert!(before.is_file());
    assert_eq!(before.file_attributes() & 0x400, 0);
    let identity = readonly_file_id(&file);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    let after = file.metadata().unwrap();
    assert_eq!(readonly_file_id(&file), identity);
    assert_eq!(before.file_size(), after.file_size());
    assert_eq!(before.last_write_time(), after.last_write_time());
    assert_eq!(bytes.len() as u64, after.file_size());
    let fingerprint = FileFingerprint {
        volume_serial: identity.volume_serial,
        file_id: identity.file_id,
        size: after.file_size(),
        modified: after.last_write_time(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    ReadOnlyProof { bytes, fingerprint }
}

struct ReadOnlyAttribute {
    path: PathBuf,
    original: fs::Permissions,
}

impl ReadOnlyAttribute {
    fn set(path: &Path) -> Self {
        let original = fs::metadata(path).unwrap().permissions();
        assert!(!original.readonly());
        let guard = Self {
            path: path.to_path_buf(),
            original: original.clone(),
        };
        let mut selected = original;
        selected.set_readonly(true);
        fs::set_permissions(path, selected).unwrap();
        assert!(fs::metadata(path).unwrap().permissions().readonly());
        guard
    }

    fn restore(self) {
        fs::set_permissions(&self.path, self.original.clone()).unwrap();
        assert!(!fs::metadata(&self.path).unwrap().permissions().readonly());
    }
}

impl Drop for ReadOnlyAttribute {
    fn drop(&mut self) {
        // Panic cleanup restores only this fixture's original attribute value.
        // No ACL, inherited rule, owner, system setting or other path is edited.
        let _ = fs::set_permissions(&self.path, self.original.clone());
    }
}

#[test]
fn readonly_target_rejects_real_write_and_formal_restore_then_retries_after_attribute_restore() {
    for purpose in PURPOSES {
        let fixture = space_fixture(purpose);
        let spec = &fixture.spec;
        let target = spec.target();
        let original = process_cohort(&target);
        let source = process_cohort(&spec.source());
        let active = process_cohort(&spec.active());
        assert_original_business(spec, purpose);
        let before = companions(&target).map(|path| readonly_proof(&path));
        drop(OpenOptions::new().write(true).open(&target).unwrap());
        let attribute = ReadOnlyAttribute::set(&target);
        let denied = OpenOptions::new().write(true).open(&target).unwrap_err();
        assert_eq!(denied.raw_os_error(), Some(5), "{denied}");
        let error = run_formal_restore(spec, purpose).unwrap_err();
        assert!(error.contains("os error 5"), "{error}");
        assert!(fs::metadata(&target).unwrap().permissions().readonly());
        assert_eq!(
            companions(&target).map(|path| readonly_proof(&path)),
            before
        );
        assert_eq!(process_cohort(&spec.source()), source);
        assert_no_transaction(spec);
        attribute.restore();
        drop(OpenOptions::new().write(true).open(&target).unwrap());
        assert_eq!(process_cohort(&target), original);
        assert_original_business(spec, purpose);
        assert_eq!(process_cohort(&target), original);
        assert_normal_retry(spec, purpose, &source, &active);
        println!("PASS completion fault: purpose={purpose:?}, fixture target readonly rejects real write/restore with Win32 error5, complete identity/bytes/mtime and business preserved, original attribute restored and formal retry succeeded");
        fixture.cleanup();
    }
}
