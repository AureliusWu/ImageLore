//! Process-local Windows primitives for an unconnected restore prototype.
//!
//! Present objects remain exclusive while their handles live. Missing sidecar
//! names and parent directory namespaces are not locked. There is no restart,
//! COMMITTED, FileID-based ownership, installation or rollback authorization here.

use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    mem::{offset_of, size_of},
    os::windows::{ffi::OsStrExt, fs::MetadataExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};

const GENERIC_READ_WRITE_DELETE: u32 = 0xc001_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_RENAME_INFO_CLASS: i32 = 3;
const FILE_DISPOSITION_INFO_CLASS: i32 = 4;

// Windows SDK winbase.h: the BOOLEAN/Flags union occupies one DWORD on current
// Windows, followed by HANDLE, DWORD byte length, and a variable WCHAR array.
#[repr(C)]
struct RenameInfo {
    flags: u32,
    root_directory: *mut c_void,
    file_name_length: u32,
    file_name: [u16; 1],
}

const _: () = assert!(std::mem::align_of::<RenameInfo>() <= std::mem::align_of::<u64>());

#[repr(C)]
struct DispositionInfo {
    delete_file: u8,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "SetFileInformationByHandle"]
    fn set_file_information_by_handle(
        handle: *mut c_void,
        class: i32,
        information: *const c_void,
        size: u32,
    ) -> i32;
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn check_parent_paths(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(invalid_input("An absolute file path is required"));
    }
    for ancestor in path.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(ancestor)?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_input(
                "Reparse or non-directory parents are unsupported",
            ));
        }
    }
    // These checks do not retain directory handles or claim namespace isolation.
    Ok(())
}

#[derive(Debug)]
struct WindowsExclusiveFile {
    file: File,
    path: PathBuf,
    created_here: bool,
}

impl WindowsExclusiveFile {
    fn open(path: &Path, create_new: bool) -> io::Result<Self> {
        check_parent_paths(path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .access_mode(GENERIC_READ_WRITE_DELETE)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .create_new(create_new)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_input("An ordinary non-reparse file is required"));
        }
        Ok(Self {
            file,
            path: path.to_path_buf(),
            created_here: create_new,
        })
    }

    fn lock_existing(path: &Path) -> io::Result<Self> {
        Self::open(path, false)
    }

    fn create_new(path: &Path) -> io::Result<Self> {
        Self::open(path, true)
    }

    fn write_created(&mut self, bytes: &[u8]) -> io::Result<()> {
        if !self.created_here {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The primitive may write only an object it created",
            ));
        }
        self.file.seek(SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        self.file.write_all(bytes)?;
        self.file.sync_all()
    }

    fn clone_at_start(&mut self) -> io::Result<File> {
        // try_clone duplicates the same handle/object and shares its cursor.
        // Every operation requires &mut self and is strictly sequential.
        let mut input = self.file.try_clone()?;
        input.seek(SeekFrom::Start(0))?;
        Ok(input)
    }

    fn read_bytes(&mut self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.clone_at_start()?.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn copy_to(&mut self, target: &Path) -> io::Result<Self> {
        let mut output = Self::create_new(target)?;
        io::copy(&mut self.clone_at_start()?, &mut output.file)?;
        output.file.sync_all()?;
        // A failed copy leaves its create-new output for inspection, never
        // removes a path that might later refer to an unrelated object.
        Ok(output)
    }

    fn rename_no_replace(&mut self, target: &Path) -> io::Result<()> {
        check_parent_paths(target)?;
        let parent = fs::canonicalize(
            target
                .parent()
                .ok_or_else(|| invalid_input("The rename target must have an existing parent"))?,
        )?;
        let target = parent.join(
            target
                .file_name()
                .ok_or_else(|| invalid_input("The rename target must name a file"))?,
        );
        let name: Vec<u16> = target.as_os_str().encode_wide().collect();
        if name.is_empty() || name.contains(&0) {
            return Err(invalid_input(
                "The rename target contains an empty or NUL name",
            ));
        }
        let name_bytes = name
            .len()
            .checked_mul(size_of::<u16>())
            .ok_or_else(|| invalid_input("The rename name length overflows"))?;
        let name_length = u32::try_from(name_bytes)
            .map_err(|_| invalid_input("The rename name exceeds the Windows buffer length"))?;
        let offset = offset_of!(RenameInfo, file_name);
        let size = offset
            .checked_add(name_bytes)
            .ok_or_else(|| invalid_input("The rename buffer length overflows"))?
            .max(size_of::<RenameInfo>());
        let buffer_size = u32::try_from(size)
            .map_err(|_| invalid_input("The rename buffer exceeds the Windows API length"))?;
        let mut aligned = vec![0_u64; size.div_ceil(size_of::<u64>())];
        let info = aligned.as_mut_ptr().cast::<RenameInfo>();
        // SAFETY: u64 storage has enough alignment for the SDK structure on
        // supported Windows targets. Checked lengths cover the variable WCHAR
        // payload. All-zero flags mean ReplaceIfExists=false, and a null root
        // accompanies the absolute canonical-parent target. The File owns a
        // live DELETE-capable handle throughout the synchronous call.
        unsafe {
            (*info).flags = 0;
            (*info).root_directory = std::ptr::null_mut();
            (*info).file_name_length = name_length;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                aligned.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
                name.len(),
            );
            if set_file_information_by_handle(
                self.file.as_raw_handle(),
                FILE_RENAME_INFO_CLASS,
                info.cast(),
                buffer_size,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        self.path = target;
        Ok(())
    }

    fn delete_owned(self) -> io::Result<()> {
        if !self.created_here {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "A pre-existing object has no primitive cleanup authorization",
            ));
        }
        let information = DispositionInfo { delete_file: 1 };
        // SAFETY: the SDK FILE_DISPOSITION_INFO contains a single BOOLEAN. The
        // DELETE-capable owned handle is still exclusive; no path is reopened,
        // deleted or compared to infer ownership. Drop closes this exact object.
        let success = unsafe {
            set_file_information_by_handle(
                self.file.as_raw_handle(),
                FILE_DISPOSITION_INFO_CLASS,
                (&information as *const DispositionInfo).cast(),
                size_of::<DispositionInfo>() as u32,
            )
        };
        if success == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ExclusiveBundle {
    role_paths: [PathBuf; 3],
    files: [Option<WindowsExclusiveFile>; 3],
}

impl ExclusiveBundle {
    fn lock_database(database: &Path) -> io::Result<Self> {
        Self::lock_mapped(super::companions(database))
    }

    fn lock_mapped(role_paths: [PathBuf; 3]) -> io::Result<Self> {
        let database = WindowsExclusiveFile::lock_existing(&role_paths[0])?;
        let mut files = [Some(database), None, None];
        for ordinal in 1..3 {
            files[ordinal] = match WindowsExclusiveFile::lock_existing(&role_paths[ordinal]) {
                Ok(file) => Some(file),
                // Only FILE_NOT_FOUND represents an optional missing object;
                // PATH_NOT_FOUND, sharing, permission and other errors abort.
                Err(error) if error.raw_os_error() == Some(2) => None,
                Err(error) => return Err(error),
            };
        }
        Ok(Self { role_paths, files })
    }

    fn missing_sidecars(&self) -> Vec<PathBuf> {
        (1..3)
            .filter(|ordinal| self.files[*ordinal].is_none())
            .map(|ordinal| self.role_paths[ordinal].clone())
            .collect()
    }

    fn check_missing_sidecars(&self) -> io::Result<()> {
        for path in self.missing_sidecars() {
            match fs::symlink_metadata(path) {
                Err(error) if error.raw_os_error() == Some(2) => (),
                Err(error) => return Err(error),
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "An unowned sidecar appeared; preserve it and stop",
                    ));
                }
            }
        }
        Ok(())
    }

    fn copy_probe_into(&mut self, directory: &Path) -> io::Result<PathBuf> {
        self.check_missing_sidecars()?;
        check_parent_paths(directory)?;
        // The probe directory must not exist, including as someone else's
        // apparently empty directory. Its parent is an explicit caller input.
        // Validate that parent before atomically acquiring directory ownership.
        fs::create_dir(directory)?;
        let database = directory.join("library.sqlite3");
        let targets = [database.clone(), database.with_extension("sqlite3-wal")];
        let mut copies = Vec::new();
        for (source, target) in self.files.iter_mut().take(2).zip(targets) {
            if let Some(source) = source {
                copies.push(source.copy_to(&target)?);
            }
        }
        self.check_missing_sidecars()?;
        // SQLite may rebuild SHM only in this disposable copy. Releasing the
        // completed output handles does not release any original bundle handle.
        drop(copies);
        Ok(database)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, OpenFlags};

    struct Fixture {
        root: PathBuf,
        temporary: PathBuf,
        marker: Vec<u8>,
    }

    impl std::ops::Deref for Fixture {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            &self.root
        }
    }

    fn fixture(label: &str) -> Fixture {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let token = format!(
            "{}-{}-{}",
            std::process::id(),
            super::super::now_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = temporary.join(format!("imagelore-owned-handles-{label}-{token}"));
        fs::create_dir(&root).unwrap();
        let marker =
            format!("owned synthetic Windows handle primitive fixture\n{token}\n").into_bytes();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("fixture.marker"))
            .unwrap()
            .write_all(&marker)
            .unwrap();
        Fixture {
            root: fs::canonicalize(root).unwrap(),
            temporary,
            marker,
        }
    }

    fn check_cleanup_root(fixture: &Fixture) -> io::Result<()> {
        // Inspect the named root without following it before canonicalization.
        let metadata = fs::symlink_metadata(&fixture.root)?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_input(
                "Fixture cleanup refuses a reparse or non-directory root",
            ));
        }
        let resolved = fs::canonicalize(&fixture.root)?;
        let temporary = fs::canonicalize(std::env::temp_dir())?;
        if resolved != fixture.root
            || temporary != fixture.temporary
            || resolved.parent() != Some(temporary.as_path())
        {
            return Err(invalid_input(
                "Fixture cleanup requires its exact original TEMP child",
            ));
        }
        Ok(())
    }

    fn check_cleanup_tree(path: &Path) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_input(
                "Fixture cleanup refuses any subtree reparse point",
            ));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                check_cleanup_tree(&entry?.path())?;
            }
        } else if !metadata.is_file() {
            return Err(invalid_input(
                "Fixture cleanup refuses an unknown file type",
            ));
        }
        Ok(())
    }

    fn remove_fixture(fixture: &Fixture) -> io::Result<()> {
        check_cleanup_root(fixture)?;
        check_cleanup_tree(&fixture.root)?;
        if fs::read(fixture.join("fixture.marker"))? != fixture.marker {
            return Err(invalid_input(
                "Fixture cleanup marker does not match its caller",
            ));
        }
        check_cleanup_root(fixture)?;
        fs::remove_dir_all(&fixture.root)
    }

    fn make_junction(junction: &Path, target: &Path) {
        use std::os::windows::process::CommandExt;

        let command = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
        let result = std::process::Command::new(command)
            .args(["/D", "/C", "mklink", "/J"])
            .arg(junction)
            .arg(target)
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "Owned fixture junction creation failed"
        );
    }

    fn sharing_denied(error: io::Error) {
        assert!(
            matches!(error.raw_os_error(), Some(5 | 32 | 33)),
            "Expected Windows sharing/access denial: {error}"
        );
    }

    fn existing_target(error: io::Error) {
        assert!(
            matches!(error.raw_os_error(), Some(80 | 183)),
            "Expected an atomic existing-target conflict: {error}"
        );
    }

    #[test]
    fn held_file_blocks_real_reopen_write_delete_and_drop_releases() {
        let root = fixture("exclusion");
        let source = root.join("original.sqlite3");
        fs::write(&source, b"original durable bytes").unwrap();
        let mut held = WindowsExclusiveFile::lock_existing(&source).unwrap();
        sharing_denied(File::open(&source).unwrap_err());
        sharing_denied(fs::write(&source, b"must never truncate").unwrap_err());
        sharing_denied(fs::remove_file(&source).unwrap_err());
        assert_eq!(held.read_bytes().unwrap(), b"original durable bytes");
        drop(held);
        assert_eq!(fs::read(&source).unwrap(), b"original durable bytes");
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn same_handle_rename_retains_exclusion_and_both_conflicts_preserve_bytes() {
        let root = fixture("rename");
        let source = root.join("原件.sqlite3");
        let reserved = root.join("预留.sqlite3");
        let occupied = root.join("occupied.sqlite3");
        fs::write(&source, b"original rename bytes").unwrap();
        fs::write(&occupied, b"foreign target bytes").unwrap();
        let mut held = WindowsExclusiveFile::lock_existing(&source).unwrap();
        existing_target(held.rename_no_replace(&occupied).unwrap_err());
        assert_eq!(held.read_bytes().unwrap(), b"original rename bytes");
        assert_eq!(fs::read(&occupied).unwrap(), b"foreign target bytes");
        held.rename_no_replace(&reserved).unwrap();
        assert_eq!(held.path, reserved); // Informational path, never cleanup authority.
        sharing_denied(File::open(&reserved).unwrap_err());
        sharing_denied(fs::write(&reserved, b"must stay exclusive").unwrap_err());
        sharing_denied(fs::remove_file(&reserved).unwrap_err());
        fs::write(&source, b"newly recreated active bytes").unwrap();
        existing_target(held.rename_no_replace(&source).unwrap_err());
        assert_eq!(held.read_bytes().unwrap(), b"original rename bytes");
        assert_eq!(fs::read(&source).unwrap(), b"newly recreated active bytes");
        drop(held);
        assert_eq!(fs::read(&reserved).unwrap(), b"original rename bytes");
        assert_eq!(fs::read(&occupied).unwrap(), b"foreign target bytes");
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn same_handle_copy_resets_shared_cursor_and_never_replaces_output() {
        let root = fixture("copy");
        let source = root.join("source.sqlite3");
        let target = root.join("copy.sqlite3");
        fs::write(&source, b"complete stream including its beginning").unwrap();
        let mut held = WindowsExclusiveFile::lock_existing(&source).unwrap();
        let expected = held.read_bytes().unwrap(); // Leaves the shared cursor at EOF.
        let mut copied = held.copy_to(&target).unwrap();
        assert_eq!(copied.read_bytes().unwrap(), expected);
        sharing_denied(File::open(&target).unwrap_err());
        assert!(held.copy_to(&target).is_err());
        assert_eq!(copied.read_bytes().unwrap(), expected);
        drop(copied);
        assert_eq!(fs::read(&target).unwrap(), expected);
        drop(held);
        assert_eq!(fs::read(&source).unwrap(), expected);
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn owned_cleanup_uses_renamed_handle_and_refuses_borrowed_objects() {
        let root = fixture("cleanup");
        let old_name = root.join("staged.sqlite3");
        let renamed = root.join("renamed.sqlite3");
        let mut owned = WindowsExclusiveFile::create_new(&old_name).unwrap();
        owned.write_created(b"owned temporary bytes").unwrap();
        owned.rename_no_replace(&renamed).unwrap();
        fs::write(&old_name, b"foreign object now at old name").unwrap();
        owned.delete_owned().unwrap();
        assert!(!renamed.try_exists().unwrap());
        assert_eq!(
            fs::read(&old_name).unwrap(),
            b"foreign object now at old name"
        );
        let borrowed = WindowsExclusiveFile::lock_existing(&old_name).unwrap();
        assert_eq!(
            borrowed.delete_owned().unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            fs::read(&old_name).unwrap(),
            b"foreign object now at old name"
        );
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn explicit_reserved_roles_copy_real_committed_wal_and_rebuild_probe_shm() {
        let root = fixture("wal-mapping");
        let writer_path = root.join("writer.sqlite3");
        let writer = Connection::open(&writer_path).unwrap();
        let mode: String = writer
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        writer
            .execute_batch("PRAGMA wal_autocheckpoint=0; CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('committed WAL payload');")
            .unwrap();
        let reservation = root.join("reservation");
        fs::create_dir(&reservation).unwrap();
        let roles = [
            reservation.join("database"),
            reservation.join("wal"),
            reservation.join("shm"),
        ];
        fs::copy(&writer_path, &roles[0]).unwrap();
        let writer_wal = writer_path.with_extension("sqlite3-wal");
        assert!(fs::metadata(&writer_wal).unwrap().len() > 0);
        fs::copy(writer_wal, &roles[1]).unwrap();
        fs::write(
            &roles[2],
            b"raw SHM sentinel must not be supplied to SQLite",
        )
        .unwrap();
        drop(writer);
        let originals: Vec<_> = roles
            .iter()
            .map(|path| {
                (
                    fs::read(path).unwrap(),
                    fs::metadata(path).unwrap().modified().unwrap(),
                )
            })
            .collect();
        let mut bundle = ExclusiveBundle::lock_mapped(roles.clone()).unwrap();
        assert!(bundle.missing_sidecars().is_empty());
        for path in &roles {
            sharing_denied(File::open(path).unwrap_err());
            sharing_denied(fs::write(path, b"must never truncate").unwrap_err());
            sharing_denied(fs::remove_file(path).unwrap_err());
        }
        let probe_dir = root.join("probe");
        let probe = bundle.copy_probe_into(&probe_dir).unwrap();
        assert!(!probe.with_extension("sqlite3-shm").exists());
        assert!(
            probe
                .with_extension("sqlite3-wal")
                .metadata()
                .unwrap()
                .len()
                > 0
        );
        let conn = Connection::open_with_flags(&probe, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(
            conn.query_row("SELECT value FROM marker", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "committed WAL payload"
        );
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        drop(conn);
        drop(bundle);
        for (path, (bytes, modified)) in roles.iter().zip(originals) {
            assert_eq!(fs::read(path).unwrap(), bytes);
            assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
        }
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn missing_sidecar_names_remain_unlocked_and_partial_lock_failure_releases_db() {
        let root = fixture("bundle-failure");
        let database = root.join("library.sqlite3");
        let wal = database.with_extension("sqlite3-wal");
        fs::write(&database, b"synthetic database role bytes").unwrap();
        let mut bundle = ExclusiveBundle::lock_database(&database).unwrap();
        assert_eq!(bundle.missing_sidecars().len(), 2);
        // This succeeds: retaining the DB handle does not reserve an absent name.
        fs::write(&wal, b"foreign newly appeared WAL bytes").unwrap();
        let probe = root.join("must-not-be-created");
        assert_eq!(
            bundle.copy_probe_into(&probe).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(!probe.exists());
        drop(bundle);
        assert_eq!(
            fs::read(&database).unwrap(),
            b"synthetic database role bytes"
        );
        assert_eq!(fs::read(&wal).unwrap(), b"foreign newly appeared WAL bytes");
        let occupied_wal = File::open(&wal).unwrap();
        sharing_denied(ExclusiveBundle::lock_database(&database).unwrap_err());
        // Acquiring DB succeeded before WAL failed; RAII must release that DB.
        assert_eq!(
            fs::read(&database).unwrap(),
            b"synthetic database role bytes"
        );
        drop(occupied_wal);
        let bundle = ExclusiveBundle::lock_database(&database).unwrap();
        drop(bundle);
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn reparse_parent_is_rejected_without_touching_its_target() {
        let root = fixture("reparse");
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let source = target.join("source.sqlite3");
        fs::write(&source, b"reparse target sentinel").unwrap();
        let junction = root.join("junction");
        make_junction(&junction, &target);
        assert_ne!(
            fs::symlink_metadata(&junction).unwrap().file_attributes()
                & FILE_ATTRIBUTE_REPARSE_POINT,
            0
        );
        assert_eq!(
            WindowsExclusiveFile::lock_existing(&junction.join("source.sqlite3"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(fs::read(&source).unwrap(), b"reparse target sentinel");
        fs::remove_dir(&junction).unwrap(); // Remove only the junction itself.
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn cleanup_rejects_a_root_redirected_to_another_owned_fixture() {
        let original = fixture("cleanup-original");
        let other = fixture("cleanup-other");
        fs::write(original.join("original.sentinel"), b"original owned bytes").unwrap();
        fs::write(other.join("other.sentinel"), b"other owned bytes").unwrap();
        assert_ne!(original.marker, other.marker);
        let displaced = original.root.with_file_name(format!(
            "{}-displaced",
            original.root.file_name().unwrap().to_string_lossy()
        ));
        // Both rename endpoints are explicit direct children of this fixture's
        // canonical TEMP. Do not move any computed path without these checks.
        check_cleanup_root(&original).unwrap();
        assert_eq!(displaced.parent(), Some(original.temporary.as_path()));
        assert!(!displaced.try_exists().unwrap());
        fs::rename(&original.root, &displaced).unwrap();
        make_junction(&original.root, &other.root);
        assert_eq!(
            remove_fixture(&original).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            fs::read(other.join("other.sentinel")).unwrap(),
            b"other owned bytes"
        );
        assert_eq!(
            fs::read(other.join("fixture.marker")).unwrap(),
            other.marker
        );
        assert_eq!(
            fs::read(displaced.join("fixture.marker")).unwrap(),
            original.marker
        );
        assert_eq!(
            fs::read(displaced.join("original.sentinel")).unwrap(),
            b"original owned bytes"
        );
        // Remove only the known junction, restore this fixture's original name,
        // then let each caller's unique marker authorize its own cleanup.
        assert_ne!(
            fs::symlink_metadata(&original.root)
                .unwrap()
                .file_attributes()
                & FILE_ATTRIBUTE_REPARSE_POINT,
            0
        );
        fs::remove_dir(&original.root).unwrap();
        assert!(!original.root.try_exists().unwrap());
        assert_eq!(fs::canonicalize(&displaced).unwrap(), displaced);
        assert_eq!(displaced.parent(), Some(original.temporary.as_path()));
        fs::rename(&displaced, &original.root).unwrap();
        remove_fixture(&original).unwrap();
        remove_fixture(&other).unwrap();
    }
}
