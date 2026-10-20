//! Bounded, strictly validated recovery protocol bytes. No filesystem operations.
//! Raw records are evidence inputs, never authorization to delete their paths.

use super::owned_handle::{DirectoryIdentity, FileFingerprint};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) const MAX_JOURNAL_BYTES: usize = 128 * 1024;
const FORMAT: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub(super) enum Mode {
    Pending,
    Recovery,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRecord {
    pub(super) relative_path: String,
    pub(super) fingerprint: FileFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prepared {
    pub(super) token: String,
    pub(super) database_name: String,
    pub(super) data_directory: DirectoryIdentity,
    pub(super) transaction_directory: DirectoryIdentity,
    pub(super) mode: Mode,
    pub(super) originals: [Option<FileFingerprint>; 3],
    pub(super) staged: FileFingerprint,
    pub(super) pending: Option<[Option<FileFingerprint>; 3]>,
    pub(super) raw: [Option<RawRecord>; 3],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: u32,
    payload: Prepared,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Commit {
    format: u32,
    kind: String,
    token: String,
    prepared_sha256: String,
    payload: Prepared,
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.is_empty()
        || token.len() > 80
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err("Invalid recovery transaction token".into());
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<(), String> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("A recovery digest must be 64 lowercase hexadecimal bytes".into());
    }
    Ok(())
}

fn validate_component(component: &str) -> Result<(), String> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.ends_with('.')
        || component.ends_with(' ')
        || component.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
                )
        })
    {
        return Err("Recovery paths require ordinary relative name components".into());
    }
    let stem = component.split('.').next().unwrap_or("");
    let upper = stem.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper.strip_prefix(*prefix).is_some_and(|suffix| {
                (suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
                    || matches!(suffix, "¹" | "²" | "³")
            })
        })
    {
        return Err("Recovery paths cannot use Windows device names".into());
    }
    Ok(())
}

fn same_component(left: &str, right: &str) -> bool {
    // Conservative lexical exclusion. Actual directory/object identity is a
    // separate caller requirement; case folding cannot establish ownership.
    left.eq_ignore_ascii_case(right)
        || left.to_lowercase() == right.to_lowercase()
        || left.to_uppercase() == right.to_uppercase()
}

fn validate_raw_path(relative: &str, database_name: &str) -> Result<(), String> {
    let components: Vec<_> = relative.split(['/', '\\']).collect();
    for component in &components {
        validate_component(component)?;
        if same_component(component, "restore.transaction") {
            return Err("Raw evidence cannot point into the recovery transaction".into());
        }
    }
    let first = components
        .first()
        .ok_or_else(|| "A raw evidence path is required".to_string())?;
    for name in [
        database_name,
        "restore.pending.sqlite3",
        "restore.pending.previous.sqlite3",
    ] {
        let main = Path::new(name);
        for reserved in [
            main.to_path_buf(),
            main.with_extension("sqlite3-wal"),
            main.with_extension("sqlite3-shm"),
        ] {
            if same_component(first, &reserved.to_string_lossy()) {
                return Err("Raw evidence cannot point into active or pending paths".into());
            }
        }
    }
    Ok(())
}

fn validate_fingerprint(fingerprint: &FileFingerprint) -> Result<(), String> {
    validate_digest(&fingerprint.sha256)
}

fn validate_prepared(prepared: &Prepared, expected_database_name: &str) -> Result<(), String> {
    validate_token(&prepared.token)?;
    validate_component(expected_database_name)?;
    validate_component(&prepared.database_name)?;
    if prepared.database_name != expected_database_name {
        return Err("Recovery journal names a different database".into());
    }
    validate_fingerprint(&prepared.staged)?;
    match (&prepared.mode, &prepared.pending) {
        (Mode::Pending, Some(pending)) if pending[0].is_some() => {
            for fingerprint in pending.iter().flatten() {
                validate_fingerprint(fingerprint)?;
            }
        }
        (Mode::Recovery, None) => (),
        _ => return Err("Recovery journal mode and pending bundle disagree".into()),
    }
    if prepared.originals[0].is_none() && prepared.mode != Mode::Pending {
        return Err("Automatic recovery requires an original database".into());
    }
    for (original, raw) in prepared.originals.iter().zip(&prepared.raw) {
        match (original, raw) {
            (Some(original), Some(raw)) => {
                validate_fingerprint(original)?;
                validate_fingerprint(&raw.fingerprint)?;
                validate_raw_path(&raw.relative_path, &prepared.database_name)?;
                if raw.fingerprint.size != original.size
                    || raw.fingerprint.sha256 != original.sha256
                {
                    return Err("Raw evidence does not preserve its original bytes".into());
                }
            }
            (None, None) => (),
            _ => return Err("Original and raw evidence roles disagree".into()),
        }
    }
    Ok(())
}

fn check_size(size: usize) -> Result<(), String> {
    if size == 0 || size > MAX_JOURNAL_BYTES {
        return Err("Recovery journal is empty or exceeds its byte limit".into());
    }
    Ok(())
}

pub(super) fn checksum_prepared(prepared: &Prepared) -> Result<String, String> {
    validate_prepared(prepared, &prepared.database_name)?;
    let bytes = serde_json::to_vec(prepared)
        .map_err(|_| "Cannot encode recovery journal payload".to_string())?;
    check_size(bytes.len())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(super) fn encode_prepared(prepared: &Prepared) -> Result<Vec<u8>, String> {
    let sha256 = checksum_prepared(prepared)?;
    let bytes = serde_json::to_vec(&Envelope {
        format: FORMAT,
        payload: prepared.clone(),
        sha256,
    })
    .map_err(|_| "Cannot encode recovery journal envelope".to_string())?;
    check_size(bytes.len())?;
    Ok(bytes)
}

pub(super) fn decode_prepared(
    bytes: &[u8],
    expected_database_name: &str,
) -> Result<Prepared, String> {
    check_size(bytes.len())?;
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|_| "Malformed or truncated recovery journal".to_string())?;
    if envelope.format != FORMAT {
        return Err("Unsupported recovery journal format".into());
    }
    validate_digest(&envelope.sha256)?;
    validate_prepared(&envelope.payload, expected_database_name)?;
    if envelope.sha256 != checksum_prepared(&envelope.payload)? {
        return Err("Recovery journal payload checksum mismatch".into());
    }
    Ok(envelope.payload)
}

pub(super) fn encode_commit(prepared: &Prepared) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(&Commit {
        format: FORMAT,
        kind: "committed".into(),
        token: prepared.token.clone(),
        prepared_sha256: checksum_prepared(prepared)?,
        payload: prepared.clone(),
    })
    .map_err(|_| "Cannot encode recovery commit record".to_string())?;
    check_size(bytes.len())?;
    Ok(bytes)
}

pub(super) fn decode_commit(
    bytes: &[u8],
    expected_database_name: &str,
) -> Result<Prepared, String> {
    check_size(bytes.len())?;
    let commit: Commit = serde_json::from_slice(bytes)
        .map_err(|_| "Malformed or truncated recovery commit record".to_string())?;
    if commit.format != FORMAT || commit.kind != "committed" {
        return Err("Unsupported recovery commit format".into());
    }
    validate_token(&commit.token)?;
    validate_digest(&commit.prepared_sha256)?;
    validate_prepared(&commit.payload, expected_database_name)?;
    if commit.token != commit.payload.token
        || commit.prepared_sha256 != checksum_prepared(&commit.payload)?
    {
        return Err("Recovery commit does not bind this prepared transaction".into());
    }
    Ok(commit.payload)
}

#[cfg(test)]
pub(super) fn validate_commit(bytes: &[u8], prepared: &Prepared) -> Result<(), String> {
    if &decode_commit(bytes, &prepared.database_name)? != prepared {
        return Err("Recovery commit payload differs from this prepared transaction".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(id: u8) -> FileFingerprint {
        FileFingerprint {
            volume_serial: 1,
            file_id: [id; 16],
            size: 17,
            modified: 123,
            sha256: "a".repeat(64),
        }
    }

    fn prepared() -> Prepared {
        Prepared {
            token: "aaaa-bbbb".into(),
            database_name: "library.sqlite3".into(),
            data_directory: DirectoryIdentity {
                volume_serial: 1,
                file_id: [10; 16],
            },
            transaction_directory: DirectoryIdentity {
                volume_serial: 1,
                file_id: [11; 16],
            },
            mode: Mode::Recovery,
            originals: [Some(fingerprint(1)), None, None],
            staged: fingerprint(2),
            pending: None,
            raw: [
                Some(RawRecord {
                    relative_path: "recovery/original.sqlite3.raw".into(),
                    fingerprint: fingerprint(3),
                }),
                None,
                None,
            ],
        }
    }

    fn unchecked_envelope(prepared: &Prepared) -> Vec<u8> {
        serde_json::to_vec(&Envelope {
            format: FORMAT,
            payload: prepared.clone(),
            sha256: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(prepared).unwrap())
            ),
        })
        .unwrap()
    }

    #[test]
    fn recovery_journal_roundtrips_and_commit_binds_payload() {
        let mut value = prepared();
        for mode in [Mode::Recovery, Mode::Pending] {
            value.mode = mode.clone();
            value.pending = (mode == Mode::Pending).then(|| [Some(fingerprint(4)), None, None]);
            let bytes = encode_prepared(&value).unwrap();
            assert_eq!(decode_prepared(&bytes, "library.sqlite3").unwrap(), value);
            let commit = encode_commit(&value).unwrap();
            validate_commit(&commit, &value).unwrap();
            let mut other = value.clone();
            other.staged.size += 1;
            assert!(validate_commit(&commit, &other).is_err());
            other = value.clone();
            other.token = "beef-abcd".into();
            assert!(validate_commit(&commit, &other).is_err());
        }
        value.originals = [None, None, None];
        value.raw = [None, None, None];
        assert!(encode_prepared(&value).is_ok());
        value.mode = Mode::Recovery;
        value.pending = None;
        assert!(encode_prepared(&value).is_err());
    }

    #[test]
    fn recovery_journal_rejects_bounds_truncation_future_unknown_and_checksum() {
        let value = prepared();
        let bytes = encode_prepared(&value).unwrap();
        assert!(decode_prepared(&bytes[..bytes.len() - 1], "library.sqlite3").is_err());
        assert!(decode_prepared(&[], "library.sqlite3").is_err());
        assert!(decode_prepared(&vec![b' '; MAX_JOURNAL_BYTES + 1], "library.sqlite3").is_err());
        assert!(decode_prepared(&bytes, "other.sqlite3").is_err());
        for mutation in ["future", "unknown", "nested", "checksum", "uppercase"] {
            let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            match mutation {
                "future" => json["format"] = 2.into(),
                "unknown" => json["extra"] = true.into(),
                "nested" => json["payload"]["staged"]["extra"] = true.into(),
                "checksum" => json["sha256"] = "0".repeat(64).into(),
                "uppercase" => json["sha256"] = "A".repeat(64).into(),
                _ => unreachable!(),
            }
            assert!(
                decode_prepared(&serde_json::to_vec(&json).unwrap(), "library.sqlite3").is_err()
            );
        }
    }

    #[test]
    fn recovery_journal_rejects_invalid_roles_even_with_matching_checksum() {
        for mutation in [
            "missing_raw",
            "extra_raw",
            "raw_size",
            "raw_sha",
            "pending",
            "pending_main",
            "digest",
            "token",
        ] {
            let mut value = prepared();
            match mutation {
                "missing_raw" => value.raw[0] = None,
                "extra_raw" => value.raw[1] = value.raw[0].clone(),
                "raw_size" => value.raw[0].as_mut().unwrap().fingerprint.size += 1,
                "raw_sha" => value.raw[0].as_mut().unwrap().fingerprint.sha256 = "b".repeat(64),
                "pending" => value.pending = Some([Some(fingerprint(4)), None, None]),
                "pending_main" => {
                    value.mode = Mode::Pending;
                    value.pending = Some([None, Some(fingerprint(4)), None]);
                }
                "digest" => value.staged.sha256 = "A".repeat(64),
                "token" => value.token = "../outside".into(),
                _ => unreachable!(),
            }
            assert!(encode_prepared(&value).is_err());
            assert!(decode_prepared(&unchecked_envelope(&value), "library.sqlite3").is_err());
        }
    }

    #[test]
    fn recovery_journal_rejects_raw_path_escape_and_protocol_aliases() {
        for path in [
            "",
            ".",
            "../raw",
            "nested/../raw",
            "nested/./raw",
            "/absolute",
            "\\absolute",
            "C:relative",
            "\\\\server\\share\\raw",
            "a//raw",
            "a\\\\raw",
            "raw\0name",
            "raw.",
            "raw ",
            "NUL.txt",
            "restore.transaction/raw",
            "nested/RESTORE.TRANSACTION/raw",
            "LIBRARY.SQLITE3",
            "library.sqlite3-wal",
            "library.sqlite3-shm",
            "restore.pending.sqlite3",
            "restore.pending.sqlite3-wal",
            "restore.pending.previous.sqlite3-shm",
        ] {
            let mut value = prepared();
            value.raw[0].as_mut().unwrap().relative_path = path.into();
            assert!(
                encode_prepared(&value).is_err(),
                "Rejected raw path: {path:?}"
            );
            assert!(decode_prepared(&unchecked_envelope(&value), "library.sqlite3").is_err());
        }
        for database in [
            "",
            ".",
            "..",
            "../library.sqlite3",
            "C:library.sqlite3",
            "a/b",
            "a\\b",
            "a\0b",
            "NUL.sqlite3",
        ] {
            let mut value = prepared();
            value.database_name = database.into();
            assert!(decode_prepared(&unchecked_envelope(&value), database).is_err());
        }
    }

    #[test]
    fn recovery_commit_is_self_contained_without_prepared_bytes() {
        let commit = {
            let original = prepared();
            let prepared_bytes = encode_prepared(&original).unwrap();
            let commit = encode_commit(&original).unwrap();
            // Pure codec test: discard both the original value and its bytes.
            // Recovery below receives only COMMITTED, not prepared.json input.
            drop(prepared_bytes);
            commit
        };
        let recovered = decode_commit(&commit, "library.sqlite3").unwrap();
        assert_eq!(recovered, prepared());
        validate_commit(&commit, &recovered).unwrap();
        assert_eq!(
            decode_prepared(&encode_prepared(&recovered).unwrap(), "library.sqlite3").unwrap(),
            recovered
        );
        assert!(decode_commit(&commit, "another.sqlite3").is_err());
    }

    #[test]
    fn recovery_commit_rejects_unbound_and_malformed_records() {
        let value = prepared();
        let bytes = encode_commit(&value).unwrap();
        assert!(validate_commit(&bytes[..bytes.len() - 1], &value).is_err());
        assert!(validate_commit(&vec![b' '; MAX_JOURNAL_BYTES + 1], &value).is_err());
        for mutation in [
            "future",
            "kind",
            "unknown",
            "nested",
            "token",
            "payload_token",
            "payload_size",
            "checksum",
            "uppercase",
            "missing_payload",
        ] {
            let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            match mutation {
                "future" => json["format"] = 2.into(),
                "kind" => json["kind"] = "prepared".into(),
                "unknown" => json["extra"] = true.into(),
                "nested" => json["payload"]["staged"]["extra"] = true.into(),
                "token" => json["token"] = "1234-abce".into(),
                "payload_token" => json["payload"]["token"] = "1234-abce".into(),
                "payload_size" => json["payload"]["staged"]["size"] = 18.into(),
                "checksum" => json["prepared_sha256"] = "0".repeat(64).into(),
                "uppercase" => json["prepared_sha256"] = "A".repeat(64).into(),
                "missing_payload" => {
                    json.as_object_mut().unwrap().remove("payload");
                }
                _ => unreachable!(),
            }
            let changed = serde_json::to_vec(&json).unwrap();
            assert!(decode_commit(&changed, "library.sqlite3").is_err());
            assert!(validate_commit(&changed, &value).is_err());
        }
    }
}
