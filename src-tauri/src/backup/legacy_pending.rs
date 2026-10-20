//! Adopt one legacy previous cohort without deleting it or replaying it twice.
//! The caller holds OperationGuard throughout finish and the following operation.

use super::guarded_restore::{fingerprints, validate_bundle};
use super::owned_handle::{
    DirectoryGuard, DirectoryIdentity, ExclusiveBundle, FileFingerprint, WindowsExclusiveFile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const PREVIOUS: &str = "restore.pending.previous.sqlite3";
const PENDING: &str = "restore.pending.sqlite3";
const ADOPTED: &str = "restore.pending.legacy-adopted.json";
const MAX_BYTES: usize = 16 * 1024;
const FORMAT: u32 = 1;
const KIND: &str = "legacy-pending-adopted";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Adoption {
    data_directory: DirectoryIdentity,
    previous_name: String,
    previous: [Option<FileFingerprint>; 3],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: u32,
    kind: String,
    payload: Adoption,
    sha256: String,
}

fn io(error: std::io::Error) -> String {
    error.to_string()
}

fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate(payload: &Adoption) -> Result<(), String> {
    if payload.previous_name != PREVIOUS || payload.previous[0].is_none() {
        return Err("旧候选接管记录必须绑定固定 previous 主库及完整角色映射".into());
    }
    for fingerprint in payload.previous.iter().flatten() {
        if !digest_valid(&fingerprint.sha256) {
            return Err("旧候选接管记录包含无效摘要".into());
        }
    }
    Ok(())
}

fn checksum(payload: &Adoption) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(payload).map_err(|error| error.to_string())?)
    ))
}

fn encode(payload: Adoption) -> Result<Vec<u8>, String> {
    validate(&payload)?;
    let sha256 = checksum(&payload)?;
    let bytes = serde_json::to_vec(&Envelope {
        format: FORMAT,
        kind: KIND.into(),
        payload,
        sha256,
    })
    .map_err(|error| error.to_string())?;
    check_size(bytes.len())?;
    Ok(bytes)
}

fn check_size(size: usize) -> Result<(), String> {
    if size == 0 || size > MAX_BYTES {
        return Err("旧候选接管记录为空或超过长度上限".into());
    }
    Ok(())
}

fn decode(bytes: &[u8]) -> Result<Adoption, String> {
    check_size(bytes.len())?;
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|error| format!("旧候选接管记录无效：{error}"))?;
    if envelope.format != FORMAT || envelope.kind != KIND {
        return Err("旧候选接管记录版本或用途不受支持".into());
    }
    validate(&envelope.payload)?;
    if !digest_valid(&envelope.sha256) || checksum(&envelope.payload)? != envelope.sha256 {
        return Err("旧候选接管记录校验失败，已保留全部文件".into());
    }
    Ok(envelope.payload)
}

fn optional_marker(path: &Path) -> Result<Option<WindowsExclusiveFile>, String> {
    match WindowsExclusiveFile::lock_existing(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.raw_os_error() == Some(2) => Ok(None),
        Err(error) => Err(io(error)),
    }
}

fn require_main_or_empty(
    fingerprints: &[Option<FileFingerprint>; 3],
    role: &str,
) -> Result<(), String> {
    if fingerprints[0].is_none() && fingerprints[1..].iter().any(Option::is_some) {
        return Err(format!("{role} 缺少主库但存在孤立侧车，已停止并保留"));
    }
    Ok(())
}

fn publish(data_dir: &Path, payload: Adoption) -> Result<WindowsExclusiveFile, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let bytes = encode(payload)?;
    let temporary = data_dir.join(format!(
        "restore.pending.legacy-adopted.next-{:x}-{:x}-{:x}.json",
        std::process::id(),
        super::now_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = WindowsExclusiveFile::create_new(&temporary).map_err(io)?;
    file.write_created(&bytes).map_err(io)?;
    // A partial/unpublished temporary remains evidence. It is never interpreted
    // as an adoption, recursively cleaned, or used to overwrite a final record.
    file.rename_no_replace(&data_dir.join(ADOPTED))
        .map_err(io)?;
    Ok(file)
}

/// Read-only preflight for a caller that already holds the operation guard.
/// This does not adopt, promote, query pending, or authorize deleting previous.
pub(super) fn verify_adopted(data_dir: &Path) -> Result<bool, String> {
    let _ancestors = DirectoryGuard::lock_ancestors(data_dir).map_err(io)?;
    let directory = DirectoryGuard::lock_existing(data_dir).map_err(io)?;
    let data = std::fs::canonicalize(data_dir).map_err(io)?;
    let Some(mut marker) = optional_marker(&data.join(ADOPTED))? else {
        return Ok(false);
    };
    let adoption = decode(&marker.read_limited(MAX_BYTES).map_err(io)?)?;
    let mut previous = ExclusiveBundle::lock_optional_database(&data.join(PREVIOUS)).map_err(io)?;
    let actual = fingerprints(&mut previous)?;
    require_main_or_empty(&actual, "旧 previous")?;
    if adoption.data_directory != directory.identity().map_err(io)? || adoption.previous != actual {
        return Err("旧 previous 与接管记录的目录身份或完整角色内容不匹配，已停止并保留".into());
    }
    Ok(true)
}

pub(super) fn finish(
    data_dir: &Path,
    promote: impl FnOnce(&[Option<FileFingerprint>; 3]) -> Result<(), String>,
    missing_checked: impl FnOnce(),
) -> Result<bool, String> {
    // Hold namespace ancestry before the first marker/previous/pending query.
    let _ancestors = DirectoryGuard::lock_ancestors(data_dir).map_err(io)?;
    let directory = DirectoryGuard::lock_existing(data_dir).map_err(io)?;
    let data = std::fs::canonicalize(data_dir).map_err(io)?;
    let identity = directory.identity().map_err(io)?;
    let mut marker = optional_marker(&data.join(ADOPTED))?;
    let adoption = marker
        .as_mut()
        .map(|file| {
            file.read_limited(MAX_BYTES)
                .map_err(io)
                .and_then(|bytes| decode(&bytes))
        })
        .transpose()?;
    let mut previous = ExclusiveBundle::lock_optional_database(&data.join(PREVIOUS)).map_err(io)?;
    let expected = fingerprints(&mut previous)?;
    require_main_or_empty(&expected, "旧 previous")?;
    if let Some(adoption) = adoption {
        if adoption.data_directory != identity || adoption.previous != expected {
            return Err(
                "旧 previous 与接管记录的目录身份或完整角色内容不匹配，已停止并保留".into(),
            );
        }
        // A matching permanent record suppresses replay even after pending was
        // consumed. Do not probe, promote, remove, or rewrite previous/marker.
        return Ok(true);
    }
    if expected[0].is_none() {
        return Ok(false);
    }

    let mut pending = ExclusiveBundle::lock_optional_database(&data.join(PENDING)).map_err(io)?;
    let present = fingerprints(&mut pending)?;
    require_main_or_empty(&present, "pending")?;
    if present[0].is_none() {
        // Core must reacquire the source and compare ALL expected roles, then
        // enforce final missing-only target policy before Prepared/install.
        drop(pending);
        drop(previous);
        missing_checked();
        promote(&expected)?;
        previous = ExclusiveBundle::lock_database(&data.join(PREVIOUS)).map_err(io)?;
        if fingerprints(&mut previous)? != expected {
            return Err("接管过程中旧 previous 已改变，已保留候选和旧副本".into());
        }
        pending = ExclusiveBundle::lock_database(&data.join(PENDING)).map_err(io)?;
    }
    // Keep both real cohorts continuously locked through validation and atomic
    // publication. Only the helper's owned disposable copy is opened by SQLite.
    validate_bundle(&data, &mut pending)?;
    let checked_pending = fingerprints(&mut pending)?;
    if checked_pending[0].is_none() || fingerprints(&mut previous)? != expected {
        return Err("旧候选接管前的角色复核失败，已保留全部文件".into());
    }
    let _published = publish(
        &data,
        Adoption {
            data_directory: identity,
            previous_name: PREVIOUS.into(),
            previous: expected.clone(),
        },
    )?;
    if fingerprints(&mut previous)? != expected || fingerprints(&mut pending)? != checked_pending {
        return Err("旧候选接管发布后的角色复核失败，已保留全部文件".into());
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, OpenFlags};
    use std::{cell::Cell, fs, path::PathBuf};

    fn fixture(label: &str) -> PathBuf {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = temporary.join(format!(
            "imagelore-adoption-{label}-{}-{}",
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

    fn cleanup(root: PathBuf) {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("imagelore-adoption-"));
        assert_eq!(fs::canonicalize(&root).unwrap(), root);
        assert_eq!(
            fs::symlink_metadata(&root).unwrap().file_attributes() & 0x400,
            0
        );
        assert_eq!(
            fs::read(root.join("fixture.marker")).unwrap(),
            root.to_string_lossy().as_bytes()
        );
        for entry in fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let name = name.to_str().unwrap();
            assert!(
                name == "fixture.marker"
                    || name == "restore.operation.lock"
                    || name == ADOPTED
                    || name.starts_with("restore.pending.legacy-adopted.next-")
                    || super::super::companions(&root.join(PREVIOUS)).contains(&entry.path())
                    || super::super::companions(&root.join(PENDING)).contains(&entry.path())
            );
            assert!(entry.file_type().unwrap().is_file());
            assert_eq!(
                fs::symlink_metadata(entry.path())
                    .unwrap()
                    .file_attributes()
                    & 0x400,
                0
            );
            fs::remove_file(entry.path()).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    fn seed(path: &Path, marker: &str) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(include_str!("../../schema.sql"))
            .unwrap();
        conn.execute_batch("CREATE TABLE marker(value TEXT NOT NULL);")
            .unwrap();
        conn.execute("INSERT INTO marker VALUES(?1)", [marker])
            .unwrap();
        conn.execute("INSERT INTO assets(id,path,name,portable_id,created_at,updated_at) VALUES(1,'synthetic/image.png','sentinel','sentinel-id',1,1)", []).unwrap();
    }

    fn marker(path: &Path) -> String {
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
            .query_row("SELECT value FROM marker", [], |row| row.get(0))
            .unwrap()
    }

    fn captured(path: &Path) -> [Option<FileFingerprint>; 3] {
        fingerprints(&mut ExclusiveBundle::lock_optional_database(path).unwrap()).unwrap()
    }

    #[test]
    fn matching_adoption_keeps_previous_and_does_not_replay_after_consumption() {
        let root = fixture("once");
        let previous = root.join(PREVIOUS);
        let pending = root.join(PENDING);
        seed(&previous, "permanent old evidence");
        let before = captured(&previous);
        let calls = Cell::new(0);
        let held = super::super::operation_lock::acquire(&root).unwrap();
        assert!(!verify_adopted(&root).unwrap());
        finish(
            &root,
            |expected| {
                calls.set(calls.get() + 1);
                assert_eq!(*expected, before);
                assert_eq!(captured(&previous), before);
                fs::copy(&previous, &pending).unwrap();
                Ok(())
            },
            || {},
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert!(verify_adopted(&root).unwrap());
        assert_eq!(captured(&previous), before);
        assert_eq!(marker(&pending), "permanent old evidence");
        let adoption_before = captured(&root.join(ADOPTED));
        // Simulate a successful consumer after adoption, then another startup.
        fs::remove_file(&pending).unwrap();
        assert!(verify_adopted(&root).unwrap());
        finish(
            &root,
            |_| panic!("adopted previous must never replay"),
            || panic!("adopted previous must not query missing pending"),
        )
        .unwrap();
        assert!(!pending.exists());
        assert_eq!(captured(&previous), before);
        assert_eq!(captured(&root.join(ADOPTED)), adoption_before);
        drop(held);
        cleanup(root);
    }

    #[test]
    fn existing_valid_pending_is_adopted_without_changing_previous() {
        let root = fixture("both");
        let previous = root.join(PREVIOUS);
        let pending = root.join(PENDING);
        seed(&previous, "previous evidence");
        seed(&pending, "current selected candidate");
        fs::write(
            super::super::companions(&previous)[2].clone(),
            b"raw previous SHM sentinel",
        )
        .unwrap();
        let original = captured(&previous);
        let candidate = captured(&pending);
        let held = super::super::operation_lock::acquire(&root).unwrap();
        finish(
            &root,
            |_| panic!("both-present must not promote"),
            || panic!("both-present must not invoke missing hook"),
        )
        .unwrap();
        assert_eq!(captured(&previous), original);
        assert_eq!(captured(&pending), candidate);
        assert_eq!(marker(&pending), "current selected candidate");
        drop(held);
        cleanup(root);
    }

    #[test]
    fn structurally_incomplete_but_readable_pending_is_not_adopted() {
        let root = fixture("schema");
        let previous = root.join(PREVIOUS);
        let pending = root.join(PENDING);
        seed(&previous, "previous evidence");
        seed(&pending, "readable incomplete candidate");
        let conn =
            Connection::open_with_flags(&pending, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
        conn.execute_batch("DROP TABLE prompt_state;").unwrap();
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        drop(conn);
        let before = [captured(&previous), captured(&pending)];
        let held = super::super::operation_lock::acquire(&root).unwrap();
        assert!(finish(
            &root,
            |_| panic!("existing candidate must not promote"),
            || unreachable!()
        )
        .is_err());
        assert_eq!([captured(&previous), captured(&pending)], before);
        assert!(!root.join(ADOPTED).exists());
        drop(held);
        cleanup(root);
    }

    #[test]
    fn damaged_or_changed_adoption_and_orphaned_sidecars_stop_without_deletion() {
        for alteration in [
            "truncated",
            "checksum",
            "unknown-field",
            "future",
            "changed-previous",
            "new-sidecar",
            "directory-identity",
        ] {
            let root = fixture(alteration);
            let previous = root.join(PREVIOUS);
            let pending = root.join(PENDING);
            seed(&previous, "previous");
            seed(&pending, "pending");
            let held = super::super::operation_lock::acquire(&root).unwrap();
            finish(&root, |_| unreachable!(), || unreachable!()).unwrap();
            let record_path = root.join(ADOPTED);
            if alteration == "changed-previous" {
                Connection::open_with_flags(&previous, OpenFlags::SQLITE_OPEN_READ_WRITE)
                    .unwrap()
                    .execute("UPDATE marker SET value='real new commit'", [])
                    .unwrap();
            } else if alteration == "new-sidecar" {
                fs::write(
                    super::super::companions(&previous)[1].clone(),
                    b"new raw WAL evidence",
                )
                .unwrap();
            } else if alteration == "truncated" {
                fs::write(&record_path, b"{\"format\":1").unwrap();
            } else {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
                match alteration {
                    "checksum" => value["sha256"] = "0".repeat(64).into(),
                    "unknown-field" => value["unexpected"] = true.into(),
                    "future" => value["format"] = 2.into(),
                    "directory-identity" => {
                        let old = value["payload"]["data_directory"]["file_id"][0]
                            .as_u64()
                            .unwrap();
                        value["payload"]["data_directory"]["file_id"][0] = (old ^ 1).into();
                        let payload: Adoption =
                            serde_json::from_value(value["payload"].clone()).unwrap();
                        value["sha256"] = checksum(&payload).unwrap().into();
                    }
                    _ => unreachable!(),
                }
                fs::write(&record_path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            let before = [
                captured(&previous),
                captured(&pending),
                captured(&record_path),
            ];
            assert!(verify_adopted(&root).is_err());
            assert!(finish(
                &root,
                |_| panic!("invalid adoption must not promote"),
                || unreachable!()
            )
            .is_err());
            assert_eq!(
                [
                    captured(&previous),
                    captured(&pending),
                    captured(&record_path)
                ],
                before
            );
            drop(held);
            cleanup(root);
        }
        for orphan in [PREVIOUS, PENDING] {
            let root = fixture("orphan");
            let previous = root.join(PREVIOUS);
            if orphan == PENDING {
                seed(&previous, "previous");
            }
            let sidecar = super::super::companions(&root.join(orphan))[1].clone();
            fs::write(&sidecar, b"orphan evidence").unwrap();
            let held = super::super::operation_lock::acquire(&root).unwrap();
            assert!(finish(
                &root,
                |_| panic!("orphan must not promote"),
                || panic!("orphan must not reach missing hook")
            )
            .is_err());
            assert_eq!(fs::read(&sidecar).unwrap(), b"orphan evidence");
            assert!(!root.join(ADOPTED).exists());
            drop(held);
            cleanup(root);
        }
    }

    #[test]
    fn unrelated_unpublished_temporary_does_not_claim_adoption() {
        let root = fixture("unpublished");
        let previous = root.join(PREVIOUS);
        let pending = root.join(PENDING);
        seed(&previous, "previous");
        seed(&pending, "pending");
        let temporary = root.join("restore.pending.legacy-adopted.next-unpublished.json");
        fs::write(&temporary, b"{truncated temporary evidence").unwrap();
        let before = fs::read(&temporary).unwrap();
        let held = super::super::operation_lock::acquire(&root).unwrap();
        finish(&root, |_| unreachable!(), || unreachable!()).unwrap();
        assert_eq!(fs::read(&temporary).unwrap(), before);
        assert!(root.join(ADOPTED).exists());
        drop(held);
        cleanup(root);
    }

    #[test]
    fn hard_linked_pending_cannot_escape_the_previous_exclusive_guard() {
        let root = fixture("hardlink");
        let previous = root.join(PREVIOUS);
        let pending = root.join(PENDING);
        seed(&previous, "one object with two names");
        fs::hard_link(&previous, &pending).unwrap();
        let before = captured(&previous);
        assert_eq!(captured(&pending), before);
        let held = super::super::operation_lock::acquire(&root).unwrap();
        assert!(finish(
            &root,
            |_| panic!("hard-linked target must not promote"),
            || panic!("hard-linked target is present")
        )
        .is_err());
        assert_eq!(captured(&previous), before);
        assert_eq!(captured(&pending), before);
        assert!(!root.join(ADOPTED).exists());
        drop(held);
        cleanup(root);
    }

    #[test]
    fn adoption_codec_rejects_missing_roles_unknown_fields_and_unbounded_bytes() {
        let root = fixture("codec");
        let previous = root.join(PREVIOUS);
        seed(&previous, "codec evidence");
        let directory = DirectoryGuard::lock_existing(&root).unwrap();
        let payload = Adoption {
            data_directory: directory.identity().unwrap(),
            previous_name: PREVIOUS.into(),
            previous: captured(&previous),
        };
        let bytes = encode(payload.clone()).unwrap();
        assert_eq!(decode(&bytes).unwrap(), payload);
        for alteration in [
            "wrong-kind",
            "wrong-name",
            "missing-main",
            "missing-role",
            "extra-role",
            "fingerprint-field",
            "missing-fingerprint-field",
            "duplicate-field",
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            match alteration {
                "wrong-kind" => value["kind"] = "another-purpose".into(),
                "wrong-name" => value["payload"]["previous_name"] = "another.sqlite3".into(),
                "missing-main" => value["payload"]["previous"][0] = serde_json::Value::Null,
                "missing-role" => {
                    value["payload"]["previous"].as_array_mut().unwrap().pop();
                }
                "extra-role" => value["payload"]["previous"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::Value::Null),
                "fingerprint-field" => value["payload"]["previous"][0]["unknown"] = true.into(),
                "missing-fingerprint-field" => {
                    value["payload"]["previous"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("modified");
                }
                "duplicate-field" => (),
                _ => unreachable!(),
            }
            let altered = if alteration == "duplicate-field" {
                String::from_utf8(bytes.clone())
                    .unwrap()
                    .replacen("\"format\":1", "\"format\":1,\"format\":1", 1)
                    .into_bytes()
            } else {
                serde_json::to_vec(&value).unwrap()
            };
            assert!(decode(&altered).is_err(), "accepted {alteration}");
        }
        assert!(decode(b"").is_err());
        assert!(decode(&vec![b' '; MAX_BYTES + 1]).is_err());
        drop(directory);
        cleanup(root);
    }
}
