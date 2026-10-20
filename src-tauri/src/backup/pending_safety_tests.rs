use super::super::owned_handle::{
    DirectoryGuard as SafetyDirectory, FileFingerprint as SafetyFingerprint,
    WindowsExclusiveFile as SafetyFile,
};
use super::super::{guarded_restore, operation_lock, restore_journal};
use super::*;
use std::{
    collections::BTreeMap,
    os::windows::fs::MetadataExt,
    sync::atomic::{AtomicU64, Ordering},
};

struct SafetyFixture {
    root: PathBuf,
    temporary: PathBuf,
    ownership: Vec<u8>,
}

impl SafetyFixture {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        let token = format!(
            "{}-{}-{}",
            std::process::id(),
            now_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let root = temporary.join(format!("imagelore-pending-safety-{label}-{token}"));
        fs::create_dir(&root).unwrap();
        let ownership = format!("synthetic pending safety fixture\n{token}\n").into_bytes();
        safety_write(&root.join("fixture.marker"), &ownership);
        fs::create_dir(root.join("backups")).unwrap();
        // Include the persistent protocol lock in all subsequent no-change
        // snapshots, rather than confusing its first creation with data loss.
        drop(operation_lock::acquire(&root).unwrap());
        Self {
            root,
            temporary,
            ownership,
        }
    }

    fn active(&self) -> PathBuf {
        self.root.join("library.sqlite3")
    }

    fn pending(&self) -> PathBuf {
        self.root.join("restore.pending.sqlite3")
    }

    fn previous(&self) -> PathBuf {
        self.root.join("restore.pending.previous.sqlite3")
    }

    fn adoption(&self) -> PathBuf {
        self.root.join("restore.pending.legacy-adopted.json")
    }

    fn active_transaction(&self) -> PathBuf {
        self.root.join("restore.transaction")
    }

    fn pending_transaction(&self) -> PathBuf {
        self.root.join("restore.pending.transaction")
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
        let metadata = fs::symlink_metadata(&self.root).unwrap();
        assert!(metadata.is_dir());
        assert_eq!(metadata.file_attributes() & 0x400, 0);
        assert_eq!(
            fs::canonicalize(std::env::temp_dir()).unwrap(),
            self.temporary
        );
        assert_eq!(fs::canonicalize(&self.root).unwrap(), self.root);
        assert_eq!(self.root.parent(), Some(self.temporary.as_path()));
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("imagelore-pending-safety-"));
        assert_eq!(
            fs::read(self.root.join("fixture.marker")).unwrap(),
            self.ownership
        );
        ordinary_tree(&self.root);
        assert_eq!(fs::canonicalize(&self.root).unwrap(), self.root);
        fs::remove_dir_all(self.root).unwrap();
    }
}

fn safety_write(path: &Path, bytes: &[u8]) {
    let mut file = SafetyFile::create_new(path).unwrap();
    file.write_created(bytes).unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SafetyProof {
    bytes: Vec<u8>,
    modified: SystemTime,
    fingerprint: SafetyFingerprint,
}

fn safety_proof(path: &Path) -> SafetyProof {
    SafetyProof {
        bytes: fs::read(path).unwrap(),
        modified: fs::metadata(path).unwrap().modified().unwrap(),
        fingerprint: SafetyFile::lock_existing(path)
            .unwrap()
            .fingerprint()
            .unwrap(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SafetyEntry {
    Directory,
    File(SafetyProof),
}

fn safety_tree(root: &Path) -> BTreeMap<PathBuf, SafetyEntry> {
    fn collect(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, SafetyEntry>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert_eq!(metadata.file_attributes() & 0x400, 0);
        let relative = path.strip_prefix(root).unwrap().to_path_buf();
        if metadata.is_dir() {
            entries.insert(relative, SafetyEntry::Directory);
            for entry in fs::read_dir(path).unwrap() {
                collect(root, &entry.unwrap().path(), entries);
            }
        } else {
            assert!(metadata.is_file());
            entries.insert(relative, SafetyEntry::File(safety_proof(path)));
        }
    }
    let mut entries = BTreeMap::new();
    collect(root, root, &mut entries);
    entries
}

fn safety_extra(path: &Path) -> String {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    conn.query_row("SELECT name FROM assets WHERE id=2", [], |row| row.get(0))
        .unwrap()
}

fn safety_seed_candidate(path: &Path, marker: &str, extra: &str) {
    let conn = seed(path, marker);
    pending_stage_insert_extra_asset(&conn, extra);
    drop(conn);
    validate_typed(path).unwrap();
}

/// A real supported SQLite cohort arranged at the precise durable protocol
/// boundary of an interrupted apply. All fingerprints come from actual files;
/// both installed and pending databases retain their business records.
fn safety_active_interruption(
    fixture: &SafetyFixture,
    committed: bool,
) -> restore_journal::Prepared {
    use restore_journal::{Mode, Prepared, RawRecord};
    let transaction = fixture.active_transaction();
    fs::create_dir(&transaction).unwrap();
    fs::create_dir_all(fixture.root.join("recovery")).unwrap();
    let raw_path = fixture.root.join("recovery/interrupted-original.raw");
    let mut original = SafetyFile::lock_existing(&fixture.active()).unwrap();
    let original_fingerprint = original.fingerprint().unwrap();
    drop(original.copy_to(&raw_path).unwrap());
    original
        .rename_no_replace(&transaction.join("database"))
        .unwrap();
    assert_eq!(original.fingerprint().unwrap(), original_fingerprint);
    drop(original);
    let mut pending = SafetyFile::lock_existing(&fixture.pending()).unwrap();
    let pending_fingerprint = pending.fingerprint().unwrap();
    drop(pending.copy_to(&fixture.active()).unwrap());
    drop(pending);
    let staged = safety_proof(&fixture.active()).fingerprint;
    let raw = safety_proof(&raw_path).fingerprint;
    assert_eq!(raw.sha256, original_fingerprint.sha256);
    assert_eq!(raw.size, original_fingerprint.size);
    let record = Prepared {
        token: "aaaa-bbbb".into(),
        database_name: "library.sqlite3".into(),
        data_directory: SafetyDirectory::lock_existing(&fixture.root)
            .unwrap()
            .identity()
            .unwrap(),
        transaction_directory: SafetyDirectory::lock_existing(&transaction)
            .unwrap()
            .identity()
            .unwrap(),
        mode: Mode::Pending,
        originals: [Some(original_fingerprint), None, None],
        staged,
        pending: Some([Some(pending_fingerprint), None, None]),
        raw: [
            Some(RawRecord {
                relative_path: "recovery/interrupted-original.raw".into(),
                fingerprint: raw,
            }),
            None,
            None,
        ],
    };
    safety_write(
        &transaction.join("PREPARED.json"),
        &restore_journal::encode_prepared(&record).unwrap(),
    );
    if committed {
        safety_write(
            &transaction.join("COMMITTED"),
            &restore_journal::encode_commit(&record).unwrap(),
        );
    }
    assert_eq!(read_marker(&fixture.active()), "interrupted candidate");
    assert_eq!(safety_extra(&fixture.active()), "candidate committed asset");
    assert_eq!(
        read_marker(&transaction.join("database")),
        "original active"
    );
    record
}

#[test]
fn pending_safety_active_committed_with_unadopted_previous_preserves_every_file() {
    let fixture = SafetyFixture::new("active-legacy-mixed");
    drop(seed(&fixture.active(), "original active"));
    safety_seed_candidate(
        &fixture.pending(),
        "interrupted candidate",
        "candidate committed asset",
    );
    drop(seed(&fixture.previous(), "legacy previous must not replay"));
    safety_active_interruption(&fixture, true);
    assert!(!fixture.adoption().exists());
    let before = safety_tree(&fixture.root);
    for _ in 0..2 {
        let error = apply_pending_restore(
            &fixture.active(),
            &fixture.root,
            &fixture.root.join("backups"),
        )
        .unwrap_err();
        assert!(error.contains("未标注旧候选"), "{error}");
        assert_eq!(safety_tree(&fixture.root), before);
        assert_eq!(read_marker(&fixture.active()), "interrupted candidate");
        assert_eq!(safety_extra(&fixture.active()), "candidate committed asset");
        assert_eq!(read_marker(&fixture.pending()), "interrupted candidate");
        assert_eq!(
            read_marker(&fixture.previous()),
            "legacy previous must not replay"
        );
        assert!(fixture.active_transaction().join("COMMITTED").exists());
        assert!(!fixture.adoption().exists());
    }
    fixture.cleanup();
}

#[test]
fn pending_safety_dual_transaction_occupancy_stops_before_either_resume() {
    for populated in [false, true] {
        let fixture = SafetyFixture::new(if populated {
            "dual-journal"
        } else {
            "dual-empty"
        });
        drop(seed(&fixture.active(), "original active"));
        safety_seed_candidate(
            &fixture.pending(),
            "interrupted candidate",
            "candidate committed asset",
        );
        if populated {
            safety_active_interruption(&fixture, true);
        } else {
            fs::create_dir(fixture.active_transaction()).unwrap();
        }
        fs::create_dir(fixture.pending_transaction()).unwrap();
        if populated {
            safety_write(
                &fixture.pending_transaction().join("unrecognized-evidence"),
                b"second transaction must remain intact\0\xff",
            );
        }
        let before = safety_tree(&fixture.root);
        let error = apply_pending_restore(
            &fixture.active(),
            &fixture.root,
            &fixture.root.join("backups"),
        )
        .unwrap_err();
        assert!(error.contains("两个恢复事务"), "{error}");
        assert_eq!(safety_tree(&fixture.root), before);
        let error = recover_latest_valid_backup(
            &fixture.active(),
            &fixture.root,
            &fixture.root.join("backups"),
            "synthetic mixed-state retry",
        )
        .unwrap_err();
        assert!(error.contains("两个恢复事务"), "{error}");
        assert_eq!(safety_tree(&fixture.root), before);
        assert_eq!(read_marker(&fixture.pending()), "interrupted candidate");
        assert_eq!(
            safety_extra(&fixture.pending()),
            "candidate committed asset"
        );
        assert_eq!(
            read_marker(&fixture.active()),
            if populated {
                "interrupted candidate"
            } else {
                "original active"
            }
        );
        fixture.cleanup();
    }
}

#[test]
fn pending_safety_matching_adoption_allows_both_active_resume_outcomes() {
    for committed in [false, true] {
        let fixture = SafetyFixture::new(if committed {
            "adopted-committed"
        } else {
            "adopted-prepared"
        });
        drop(seed(&fixture.active(), "original active"));
        drop(seed(&fixture.previous(), "permanent legacy evidence"));
        safety_seed_candidate(
            &fixture.pending(),
            "interrupted candidate",
            "candidate committed asset",
        );
        finish_pending_stage(&fixture.root).unwrap();
        let previous = safety_proof(&fixture.previous());
        let adoption = safety_proof(&fixture.adoption());
        let record = safety_active_interruption(&fixture, committed);
        finish_or_rollback(&fixture.active(), &fixture.root).unwrap();
        assert!(!fixture.active_transaction().exists());
        if committed {
            assert_eq!(safety_proof(&fixture.active()).fingerprint, record.staged);
            assert!(!fixture.pending().exists());
        } else {
            assert_eq!(read_marker(&fixture.active()), "original active");
            assert_eq!(
                safety_proof(&fixture.active()).fingerprint,
                record.originals[0].clone().unwrap()
            );
            assert_eq!(read_marker(&fixture.pending()), "interrupted candidate");
            apply_pending_restore(
                &fixture.active(),
                &fixture.root,
                &fixture.root.join("backups"),
            )
            .unwrap();
        }
        assert_eq!(read_marker(&fixture.active()), "interrupted candidate");
        assert_eq!(safety_extra(&fixture.active()), "candidate committed asset");
        assert_eq!(
            read_marker(&fixture.root.join("recovery/interrupted-original.raw")),
            "original active"
        );
        assert!(!fixture.pending().exists());
        let installed = safety_proof(&fixture.active());
        for _ in 0..3 {
            finish_pending_stage(&fixture.root).unwrap();
            apply_pending_restore(
                &fixture.active(),
                &fixture.root,
                &fixture.root.join("backups"),
            )
            .unwrap();
            assert!(!fixture.pending().exists());
            assert!(!fixture.active_transaction().exists());
            assert_eq!(safety_proof(&fixture.active()), installed);
            assert_eq!(safety_proof(&fixture.previous()), previous);
            assert_eq!(safety_proof(&fixture.adoption()), adoption);
        }
        fixture.cleanup();
    }
}

#[test]
fn pending_safety_legacy_only_apply_and_repeated_startup_never_replays_previous() {
    let fixture = SafetyFixture::new("legacy-once");
    drop(seed(&fixture.active(), "original active"));
    safety_seed_candidate(
        &fixture.previous(),
        "legacy selected library",
        "legacy selected extra asset",
    );
    let previous = safety_proof(&fixture.previous());
    assert!(!fixture.pending().exists());
    finish_pending_stage(&fixture.root).unwrap();
    assert_eq!(read_marker(&fixture.pending()), "legacy selected library");
    assert_eq!(
        safety_extra(&fixture.pending()),
        "legacy selected extra asset"
    );
    assert_eq!(safety_proof(&fixture.previous()), previous);
    let adoption = safety_proof(&fixture.adoption());
    apply_pending_restore(
        &fixture.active(),
        &fixture.root,
        &fixture.root.join("backups"),
    )
    .unwrap();
    assert_eq!(read_marker(&fixture.active()), "legacy selected library");
    assert_eq!(
        safety_extra(&fixture.active()),
        "legacy selected extra asset"
    );
    assert!(!fixture.pending().exists());
    let installed = safety_proof(&fixture.active());
    for _ in 0..3 {
        finish_pending_stage(&fixture.root).unwrap();
        apply_pending_restore(
            &fixture.active(),
            &fixture.root,
            &fixture.root.join("backups"),
        )
        .unwrap();
        assert!(!fixture.pending().exists());
        assert!(!fixture.active_transaction().exists());
        assert!(!fixture.pending_transaction().exists());
        assert_eq!(safety_proof(&fixture.active()), installed);
        assert_eq!(safety_proof(&fixture.previous()), previous);
        assert_eq!(safety_proof(&fixture.adoption()), adoption);
        assert_eq!(read_marker(&fixture.active()), "legacy selected library");
        assert_eq!(
            safety_extra(&fixture.active()),
            "legacy selected extra asset"
        );
    }
    fixture.cleanup();
}

#[test]
fn pending_safety_reserved_same_id_commit_after_release_is_preserved_in_both_finish_paths() {
    for committed in [false, true] {
        let fixture = SafetyFixture::new(if committed {
            "stage-release-committed"
        } else {
            "stage-release-prepared"
        });
        drop(seed(&fixture.active(), "active library never staged"));
        drop(seed(&fixture.pending(), "old pending before release"));
        let source = fixture.root.join("backups/selected.sqlite3");
        safety_seed_candidate(
            &source,
            "selected new pending",
            "selected candidate extra asset",
        );
        let active_before = safety_proof(&fixture.active());
        let source_before = safety_proof(&source);
        let captured = std::cell::RefCell::new(None);
        let operation = operation_lock::acquire(&fixture.root).unwrap();
        let result = guarded_restore::replace(
            &fixture.pending(),
            &fixture.root,
            &source,
            &fixture.root.join("recovery/pending-stage"),
            "release-race",
            guarded_restore::RestorePurpose::StagePending,
            |step| {
                if step == RestoreStep::Installed && !committed {
                    return Err("injected StagePending failure before commit".into());
                }
                if step == RestoreStep::Released {
                    let transaction = fixture.pending_transaction();
                    assert_eq!(transaction.join("COMMITTED").exists(), committed);
                    let record = restore_journal::decode_prepared(
                        &fs::read(transaction.join("PREPARED.json")).unwrap(),
                        "restore.pending.sqlite3",
                    )
                    .unwrap();
                    assert_eq!(record.mode, restore_journal::Mode::StagePending);
                    let reserved = transaction.join("database");
                    let conn =
                        Connection::open_with_flags(&reserved, OpenFlags::SQLITE_OPEN_READ_WRITE)
                            .unwrap();
                    conn.execute_batch("BEGIN IMMEDIATE;").unwrap();
                    conn.execute(
                        "UPDATE marker SET value='reserved real commit after release'",
                        [],
                    )
                    .unwrap();
                    pending_stage_insert_extra_asset(
                        &conn,
                        "reserved post-release committed asset",
                    );
                    conn.execute_batch("COMMIT;").unwrap();
                    drop(conn);
                    let actual = safety_proof(&reserved);
                    let expected = record.originals[0].as_ref().unwrap();
                    assert_eq!(actual.fingerprint.volume_serial, expected.volume_serial);
                    assert_eq!(actual.fingerprint.file_id, expected.file_id);
                    assert_ne!(actual.fingerprint.sha256, expected.sha256);
                    *captured.borrow_mut() =
                        Some((record, actual, safety_proof(&fixture.pending())));
                }
                Ok(())
            },
        );
        drop(operation);
        let (record, changed_reserved, installed_pending) = captured
            .into_inner()
            .expect("real SQLite commit must run at Released");
        assert!(result.is_err());
        let reserved = fixture.pending_transaction().join("database");
        assert_eq!(read_marker(&reserved), "reserved real commit after release");
        assert_eq!(
            safety_extra(&reserved),
            "reserved post-release committed asset"
        );
        assert_eq!(safety_proof(&reserved), changed_reserved);
        assert_eq!(read_marker(&fixture.pending()), "selected new pending");
        assert_eq!(
            safety_extra(&fixture.pending()),
            "selected candidate extra asset"
        );
        assert_eq!(safety_proof(&fixture.pending()), installed_pending);
        assert_eq!(safety_proof(&fixture.active()), active_before);
        assert_eq!(safety_proof(&source), source_before);
        let raw = fixture
            .root
            .join(&record.raw[0].as_ref().unwrap().relative_path);
        assert_eq!(read_marker(&raw), "old pending before release");
        assert_eq!(
            safety_proof(&raw).fingerprint,
            record.raw[0].as_ref().unwrap().fingerprint
        );
        let before_retry = safety_tree(&fixture.root);
        for _ in 0..2 {
            assert!(finish_pending_stage(&fixture.root).is_err());
            assert_eq!(safety_tree(&fixture.root), before_retry);
            assert!(apply_pending_restore(
                &fixture.active(),
                &fixture.root,
                &fixture.root.join("backups")
            )
            .is_err());
            assert_eq!(safety_tree(&fixture.root), before_retry);
            assert_eq!(read_marker(&reserved), "reserved real commit after release");
            assert_eq!(
                safety_extra(&reserved),
                "reserved post-release committed asset"
            );
        }
        fixture.cleanup();
    }
}
