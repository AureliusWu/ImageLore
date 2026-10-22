use super::super::{owned_handle::WindowsExclusiveFile, restore_journal, space_fault};
use super::restore_process_exit_tests::{
    assert_business, business_at, corrupt_business_at, process_business, process_cohort,
    process_corrupt_business, process_proof, seed_process_wal, Point, ProcessFixture, ProcessProof,
    ProcessSpec, Purpose,
};
use super::*;
use space_fault::{IoKind, IoObservation};
use std::collections::BTreeMap;

const PURPOSES: [Purpose; 3] = [
    Purpose::ApplyPending,
    Purpose::StagePending,
    Purpose::RecoverCorrupt,
];
const METADATA_PREFIX: u64 = 17;
const RAW_PREFIX: u64 = 31;

fn preservation_directory(spec: &ProcessSpec, purpose: Purpose) -> PathBuf {
    match purpose {
        Purpose::ApplyPending => spec.data().join("backups"),
        Purpose::StagePending => spec.data().join("recovery/pending-stage"),
        Purpose::RecoverCorrupt => spec.data().join("recovery"),
    }
}

fn space_fixture(purpose: Purpose) -> ProcessFixture {
    // Point only satisfies the shared fixture constructor; these tests never
    // spawn a process or register any process-interruption hook.
    let fixture = ProcessFixture::new(purpose, Point::Prepared);
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
    fs::create_dir_all(preservation_directory(spec, purpose)).unwrap();
    assert!(process_cohort(&spec.target()).iter().all(Option::is_some));
    assert!(process_cohort(&spec.source()).iter().all(Option::is_some));
    fixture
}

fn run_formal_restore(spec: &ProcessSpec, purpose: Purpose) -> Result<(), String> {
    match purpose {
        Purpose::ApplyPending => {
            apply_pending_restore(&spec.active(), &spec.data(), &spec.data().join("backups"))
        }
        Purpose::StagePending => {
            // Staging does not use the live library connection. Keep AppState's
            // unused connection in memory, so the real active DB/WAL/SHM remain
            // closed and fingerprintable throughout the formal stage entry.
            let state = AppState {
                db: std::sync::Mutex::new(Connection::open_in_memory().unwrap()),
                backup_operation: std::sync::Mutex::new(()),
                data_dir: spec.data(),
                database_path: spec.active(),
                cache_dir: spec.data().join("cache"),
                backups_dir: spec.data().join("backups"),
                models_dir: spec.data().join("models"),
                jobs: std::sync::Mutex::new(std::collections::HashMap::new()),
                next_job_id: std::sync::atomic::AtomicU64::new(1),
                vision_api_key: std::sync::Mutex::new(String::new()),
            };
            stage_restore_at(&state, "selected.sqlite3").map(|staged| assert!(staged))
        }
        Purpose::RecoverCorrupt => recover_latest_valid_backup(
            &spec.active(),
            &spec.data(),
            &spec.data().join("backups"),
            "synthetic scoped capacity fault",
        )
        .map(|selected| {
            let selected = selected.expect("normal recovery must find its valid supported backup");
            assert_eq!(selected.name, "selected.sqlite3");
            assert_eq!(PathBuf::from(selected.path), spec.source());
        }),
    }
}

fn assert_original_business(spec: &ProcessSpec, purpose: Purpose) {
    let (marker, extra) = match purpose {
        Purpose::ApplyPending | Purpose::RecoverCorrupt => {
            ("old active committed WAL", "old active extra asset")
        }
        Purpose::StagePending => ("old pending committed WAL", "old pending extra asset"),
    };
    let business = if purpose == Purpose::RecoverCorrupt {
        corrupt_business_at(spec, &spec.target())
    } else {
        business_at(spec, &spec.target())
    };
    assert_business(&business, marker, extra);
}

fn assert_selected_business(spec: &ProcessSpec, purpose: Purpose) {
    let (marker, extra) = match purpose {
        Purpose::ApplyPending => (
            "selected pending committed WAL",
            "selected pending extra asset",
        ),
        Purpose::StagePending | Purpose::RecoverCorrupt => (
            "selected source committed WAL",
            "selected source extra asset",
        ),
    };
    assert_business(&business_at(spec, &spec.target()), marker, extra);
}

fn assert_no_transaction(spec: &ProcessSpec) {
    assert!(!spec.data().join("restore.transaction").exists());
    assert!(!spec.data().join("restore.pending.transaction").exists());
}

fn assert_io_receipt(observation: &IoObservation, path: &Path, kind: IoKind, prefix: u64) {
    assert_eq!(observation.path, path);
    assert_eq!(observation.kind, kind);
    assert!(observation.requested_bytes > prefix);
    assert_eq!(observation.written_bytes, prefix);
    assert!(observation.flushed);
    assert_eq!(observation.raw_os_error, 112);
}

fn backup_names(spec: &ProcessSpec) -> Vec<String> {
    records(&spec.data().join("backups"))
        .unwrap()
        .into_iter()
        .map(|record| record.name)
        .collect()
}

fn scene(spec: &ProcessSpec) -> BTreeMap<PathBuf, Option<ProcessProof>> {
    fn visit(root: &Path, path: &Path, output: &mut BTreeMap<PathBuf, Option<ProcessProof>>) {
        use std::os::windows::fs::MetadataExt;
        let metadata = fs::symlink_metadata(path).unwrap();
        assert_eq!(metadata.file_attributes() & 0x400, 0);
        let relative = path.strip_prefix(root).unwrap().to_path_buf();
        if metadata.is_dir() {
            output.insert(relative, None);
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), output);
            }
        } else {
            assert!(metadata.is_file());
            output.insert(relative, Some(process_proof(path)));
        }
    }
    let mut output = BTreeMap::new();
    visit(&spec.data(), &spec.data(), &mut output);
    output
}

fn assert_normal_retry(
    spec: &ProcessSpec,
    purpose: Purpose,
    source: &[Option<ProcessProof>; 3],
    active: &[Option<ProcessProof>; 3],
) {
    run_formal_restore(spec, purpose).unwrap();
    assert_selected_business(spec, purpose);
    assert_no_transaction(spec);
    if purpose == Purpose::ApplyPending {
        assert!(process_cohort(&spec.source()).iter().all(Option::is_none));
    } else {
        assert_eq!(&process_cohort(&spec.source()), source);
    }
    if purpose == Purpose::StagePending {
        assert_eq!(&process_cohort(&spec.active()), active);
    }
}

#[test]
fn space_failure_prepared_partial_write_cleans_owned_metadata_and_retries() {
    for purpose in PURPOSES {
        let fixture = space_fixture(purpose);
        let spec = &fixture.spec;
        let original = process_cohort(&spec.target());
        let source = process_cohort(&spec.source());
        let active = process_cohort(&spec.active());
        let prepared = spec.transaction().join("PREPARED.json");
        let (result, observation) =
            space_fault::with_io_fault(&prepared, IoKind::WriteCreated, METADATA_PREFIX, || {
                run_formal_restore(spec, purpose)
            });
        let error = result.unwrap_err();
        assert!(error.contains("112"), "{error}");
        let observation =
            observation.expect("formal restore must perform the exact metadata prefix write");
        assert_io_receipt(
            &observation,
            &prepared,
            IoKind::WriteCreated,
            METADATA_PREFIX,
        );
        assert!(
            !prepared.exists(),
            "only our unrecorded partial metadata is cleaned"
        );
        assert_no_transaction(spec);
        assert_eq!(process_cohort(&spec.target()), original);
        assert_eq!(process_cohort(&spec.source()), source);
        assert_original_business(spec, purpose);
        assert_eq!(process_cohort(&spec.target()), original);
        assert_normal_retry(spec, purpose, &source, &active);
        println!("PASS scoped capacity fault: purpose={purpose:?}, PREPARED actual partial write+flush={}, error={}, originals/source preserved, normal retry", observation.written_bytes, observation.raw_os_error);
        fixture.cleanup();
    }
}

#[test]
fn space_failure_raw_partial_copy_keeps_evidence_out_of_backup_candidates() {
    for purpose in PURPOSES {
        for ordinal in 0..3 {
            let fixture = space_fixture(purpose);
            let spec = &fixture.spec;
            let original = process_cohort(&spec.target());
            let source = process_cohort(&spec.source());
            let active = process_cohort(&spec.active());
            let candidates = backup_names(spec);
            let parent = preservation_directory(spec, purpose);
            let (result, observation) =
                space_fault::with_raw_copy_fault(&parent, ordinal, RAW_PREFIX, || {
                    run_formal_restore(spec, purpose)
                });
            let error = result.unwrap_err();
            assert!(error.contains("112"), "{error}");
            let observation =
                observation.expect("the exact original role must perform a partial forensic copy");
            assert_io_receipt(&observation, &observation.path, IoKind::CopyTo, RAW_PREFIX);
            assert_eq!(observation.path.parent(), Some(parent.as_path()));
            assert_eq!(
                observation.path.extension().and_then(|name| name.to_str()),
                Some("raw")
            );
            assert_eq!(
                observation.requested_bytes,
                original[ordinal].as_ref().unwrap().bytes.len() as u64
            );
            let partial = process_proof(&observation.path);
            assert_eq!(
                partial.bytes,
                original[ordinal].as_ref().unwrap().bytes[..RAW_PREFIX as usize]
            );
            assert_no_transaction(spec);
            assert_eq!(process_cohort(&spec.target()), original);
            assert_eq!(process_cohort(&spec.source()), source);
            assert_original_business(spec, purpose);
            assert_eq!(backup_names(spec), candidates);
            assert!(records(&spec.data().join("backups"))
                .unwrap()
                .iter()
                .all(|candidate| Path::new(&candidate.path) != observation.path.as_path()));
            assert_eq!(process_proof(&observation.path), partial);
            assert_normal_retry(spec, purpose, &source, &active);
            assert_eq!(process_proof(&observation.path), partial, "a later successful operation must retain the partial raw with its exact identity and mtime");
            println!("PASS scoped capacity fault: purpose={purpose:?}, raw_role={ordinal}, actual partial copy+flush={}, error={}, evidence excluded from backup candidates and retained after retry", observation.written_bytes, observation.raw_os_error);
            fixture.cleanup();
        }
    }
}

#[test]
fn space_failure_partial_commit_rejects_restarts_and_preserves_the_complete_scene() {
    for purpose in PURPOSES {
        let fixture = space_fixture(purpose);
        let spec = &fixture.spec;
        let original = process_cohort(&spec.target());
        let source = process_cohort(&spec.source());
        let active = process_cohort(&spec.active());
        let commit = spec.transaction().join("COMMITTED");
        let (result, observation) =
            space_fault::with_io_fault(&commit, IoKind::WriteCreated, METADATA_PREFIX, || {
                run_formal_restore(spec, purpose)
            });
        let error = result.unwrap_err();
        assert!(error.contains("112"), "{error}");
        let observation =
            observation.expect("formal restore must write the exact commit file before failing");
        assert_io_receipt(&observation, &commit, IoKind::WriteCreated, METADATA_PREFIX);
        let record = restore_journal::decode_prepared(
            &fs::read(spec.transaction().join("PREPARED.json")).unwrap(),
            spec.target_name(),
        )
        .unwrap();
        let expected_commit = restore_journal::encode_commit(&record).unwrap();
        assert_eq!(observation.requested_bytes, expected_commit.len() as u64);
        assert_eq!(
            fs::read(&commit).unwrap(),
            expected_commit[..METADATA_PREFIX as usize]
        );
        assert!(
            restore_journal::decode_commit(&fs::read(&commit).unwrap(), spec.target_name())
                .is_err()
        );
        assert_eq!(process_cohort(&spec.source()), source);
        if purpose == Purpose::StagePending {
            assert_eq!(process_cohort(&spec.active()), active);
        }
        assert_selected_business(spec, purpose);
        let reserved_paths =
            ["database", "wal", "shm"].map(|role| Some(spec.transaction().join(role)));
        let raw_paths = record
            .raw
            .each_ref()
            .map(|raw| raw.as_ref().map(|raw| spec.data().join(&raw.relative_path)));
        for ordinal in 0..3 {
            assert_eq!(
                process_proof(reserved_paths[ordinal].as_ref().unwrap()),
                *original[ordinal].as_ref().unwrap()
            );
            assert_eq!(
                process_proof(raw_paths[ordinal].as_ref().unwrap()).bytes,
                original[ordinal].as_ref().unwrap().bytes
            );
            assert_eq!(
                WindowsExclusiveFile::lock_existing(raw_paths[ordinal].as_ref().unwrap())
                    .unwrap()
                    .fingerprint()
                    .unwrap(),
                record.raw[ordinal].as_ref().unwrap().fingerprint
            );
        }
        let (marker, extra) = match purpose {
            Purpose::ApplyPending | Purpose::RecoverCorrupt => {
                ("old active committed WAL", "old active extra asset")
            }
            Purpose::StagePending => ("old pending committed WAL", "old pending extra asset"),
        };
        for paths in [&reserved_paths, &raw_paths] {
            let business = if purpose == Purpose::RecoverCorrupt {
                process_corrupt_business(spec, paths)
            } else {
                process_business(spec, paths)
            };
            assert_business(&business, marker, extra);
        }
        let before_restart = scene(spec);
        for _ in 0..3 {
            assert!(run_formal_restore(spec, purpose).is_err(), "a torn commit cannot be guessed successful or rolled back by deleting unknown metadata");
            assert_eq!(scene(spec), before_restart);
            assert_eq!(process_cohort(&spec.source()), source);
        }
        println!("PASS scoped capacity fault: purpose={purpose:?}, COMMITTED actual partial write+flush={}, error={}, three subsequent formal entries refuse and keep every file", observation.written_bytes, observation.raw_os_error);
        fixture.cleanup();
    }
}

#[test]
fn space_failure_real_sqlite_full_preserves_originals_and_sources_and_retries() {
    for purpose in PURPOSES {
        let fixture = space_fixture(purpose);
        let spec = &fixture.spec;
        let original = process_cohort(&spec.target());
        let source = process_cohort(&spec.source());
        let active = process_cohort(&spec.active());
        let before = scene(spec);
        let staged = spec.transaction().join("staged");
        let (result, observation) =
            space_fault::with_sqlite_full(&staged, || run_formal_restore(spec, purpose));
        let error = result.unwrap_err();
        let observation =
            observation.expect("actual SQLite VACUUM must observe FULL in the scoped bounded VFS");
        assert_eq!(observation.staged_path, staged);
        assert_eq!(observation.effective_cap, 8192);
        assert_eq!(observation.sqlite_code, rusqlite::ErrorCode::DiskFull);
        assert_eq!(observation.extended_code, rusqlite::ffi::SQLITE_FULL);
        assert!(!observation.error_message.is_empty());
        assert!(
            error.contains(&observation.error_message),
            "actual SQLite cause must be retained: {error}"
        );
        assert_no_transaction(spec);
        assert_eq!(scene(spec), before);
        assert_eq!(process_cohort(&spec.target()), original);
        assert_eq!(process_cohort(&spec.source()), source);
        assert_original_business(spec, purpose);
        assert_eq!(process_cohort(&spec.target()), original);
        assert_normal_retry(spec, purpose, &source, &active);
        println!("PASS scoped capacity fault: purpose={purpose:?}, actual SQLite code={}, typed={:?}, bounded memdb cap={}, original/source DB-WAL-SHM preserved and formal retry succeeded", observation.extended_code, observation.sqlite_code, observation.effective_cap);
        fixture.cleanup();
    }
}

#[test]
fn space_failure_io_scope_matches_exact_path_and_kind_and_resets_after_unwind() {
    let fixture = ProcessFixture::new(Purpose::ApplyPending, Point::Prepared);
    let spec = &fixture.spec;
    let source_path = spec.data().join("scope-source.bin");
    let other_path = spec.data().join("scope-other.bin");
    let target_path = spec.data().join("scope-target.bin");
    let bytes = [7_u8; 128];
    let mut source = WindowsExclusiveFile::create_new(&source_path).unwrap();
    source.write_created(&bytes).unwrap();
    let ((result, mut target), observation) =
        space_fault::with_io_fault(&target_path, IoKind::WriteCreated, METADATA_PREFIX, || {
            let mut other = WindowsExclusiveFile::create_new(&other_path).unwrap();
            other.write_created(&bytes).unwrap(); // Different path cannot consume it.
            drop(other);
            let mut target = source.copy_to(&target_path).unwrap(); // Different kind cannot consume it.
            let result = target.write_created(&bytes);
            (result, target)
        });
    assert_eq!(result.unwrap_err().raw_os_error(), Some(112));
    assert_io_receipt(
        &observation.unwrap(),
        &target_path,
        IoKind::WriteCreated,
        METADATA_PREFIX,
    );
    target.write_created(&bytes).unwrap(); // Leaving scope removes the fault.
    drop(target);
    assert_eq!(fs::read(&other_path).unwrap(), bytes);
    assert_eq!(fs::read(&target_path).unwrap(), bytes);
    let unwind_path = spec.data().join("scope-after-unwind.bin");
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: ((), Option<IoObservation>) =
            space_fault::with_io_fault(&unwind_path, IoKind::CopyTo, RAW_PREFIX, || {
                panic!("intentional scope-unwind control")
            });
    }));
    assert!(unwind.is_err());
    drop(source.copy_to(&unwind_path).unwrap());
    drop(source);
    assert_eq!(fs::read(&source_path).unwrap(), bytes);
    assert_eq!(fs::read(&unwind_path).unwrap(), bytes);
    fixture.cleanup();
}
