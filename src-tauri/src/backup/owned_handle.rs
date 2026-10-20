//! Process-local Windows primitives for guarded restore operations.
//!
//! Present objects remain exclusive while their handles live. Missing sidecar
//! names are not locked. Directory guards prevent directory renames, not child
//! creation. Fingerprints describe content and identity, never transaction
//! authorization; the caller must validate its journal before destructive use.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    mem::{offset_of, size_of},
    os::windows::{
        ffi::OsStrExt, fs::MetadataExt, fs::OpenOptionsExt, io::AsRawHandle, io::FromRawHandle,
    },
    path::{Path, PathBuf},
};

const GENERIC_READ_WRITE_DELETE: u32 = 0xc001_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_RENAME_INFO_CLASS: i32 = 3;
const FILE_DISPOSITION_INFO_CLASS: i32 = 4;
const FILE_ID_INFO_CLASS: i32 = 18;
const FILE_READ_ATTRIBUTES: u32 = 0x80;
const FILE_LIST_DIRECTORY: u32 = 0x1;
const DELETE_ACCESS: u32 = 0x0001_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const SHARE_READ_WRITE: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectoryIdentity {
    pub(super) volume_serial: u64,
    pub(super) file_id: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileFingerprint {
    pub(super) volume_serial: u64,
    pub(super) file_id: [u8; 16],
    pub(super) size: u64,
    pub(super) modified: u64,
    pub(super) sha256: String,
}

// SDK FILE_ID_INFO: ULONGLONG VolumeSerialNumber; FILE_ID_128 FileId.
#[repr(C)]
struct FileIdInfo {
    volume_serial: u64,
    file_id: [u8; 16],
}

const _: () = assert!(size_of::<FileIdInfo>() == 24);

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

    #[link_name = "GetFileInformationByHandleEx"]
    fn get_file_information_by_handle_ex(
        handle: *mut c_void,
        class: i32,
        information: *mut c_void,
        size: u32,
    ) -> i32;
}

// Layouts checked against winternl.h, including the pointer-sized first union
// of IO_STATUS_BLOCK. NtCreateFile returns NTSTATUS, not a Win32 last-error.
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: *mut c_void,
    object_name: *mut UnicodeString,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_quality_of_service: *mut c_void,
}

#[repr(C)]
struct IoStatusBlock {
    status_or_pointer: usize,
    information: usize,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    #[link_name = "NtCreateFile"]
    fn nt_create_file(
        handle: *mut *mut c_void,
        desired_access: u32,
        attributes: *mut ObjectAttributes,
        status: *mut IoStatusBlock,
        allocation_size: *mut i64,
        file_attributes: u32,
        share_access: u32,
        disposition: u32,
        options: u32,
        ea_buffer: *mut c_void,
        ea_length: u32,
    ) -> i32;

    #[link_name = "RtlNtStatusToDosError"]
    fn rtl_nt_status_to_dos_error(status: i32) -> u32;
}

fn file_identity(file: &File) -> io::Result<DirectoryIdentity> {
    let mut information = FileIdInfo {
        volume_serial: 0,
        file_id: [0; 16],
    };
    // SAFETY: the initialized SDK-sized buffer is writable for this synchronous
    // query, and the owned File keeps the handle alive throughout the call.
    let success = unsafe {
        get_file_information_by_handle_ex(
            file.as_raw_handle(),
            FILE_ID_INFO_CLASS,
            (&mut information as *mut FileIdInfo).cast(),
            size_of::<FileIdInfo>() as u32,
        )
    };
    if success == 0 {
        let error = io::Error::last_os_error();
        return Err(error);
    }
    Ok(DirectoryIdentity {
        volume_serial: information.volume_serial,
        file_id: information.file_id,
    })
}

fn delete_held(file: &File) -> io::Result<()> {
    let information = DispositionInfo { delete_file: 1 };
    // SAFETY: one SDK BOOLEAN is passed while this DELETE-capable handle lives.
    // The OS atomically rejects nonempty directories; this never recurses.
    let success = unsafe {
        set_file_information_by_handle(
            file.as_raw_handle(),
            FILE_DISPOSITION_INFO_CLASS,
            (&information as *const DispositionInfo).cast(),
            size_of::<DispositionInfo>() as u32,
        )
    };
    if success == 0 {
        let error = io::Error::last_os_error();
        return Err(error);
    }
    Ok(())
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
pub(super) struct WindowsExclusiveFile {
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

    pub(super) fn lock_existing(path: &Path) -> io::Result<Self> {
        Self::open(path, false)
    }

    pub(super) fn create_new(path: &Path) -> io::Result<Self> {
        Self::open(path, true)
    }

    pub(super) fn write_created(&mut self, bytes: &[u8]) -> io::Result<()> {
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

    #[cfg(test)]
    fn read_bytes(&mut self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.clone_at_start()?.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn read_limited(&mut self, max: usize) -> io::Result<Vec<u8>> {
        let maximum = u64::try_from(max)
            .map_err(|_| invalid_input("The metadata limit exceeds the file API length"))?;
        if self.file.metadata()?.len() > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Metadata exceeds its limit",
            ));
        }
        let limit = maximum
            .checked_add(1)
            .ok_or_else(|| invalid_input("The metadata limit overflows"))?;
        let mut bytes = Vec::new();
        self.clone_at_start()?.take(limit).read_to_end(&mut bytes)?;
        if bytes.len() > max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Metadata stream exceeds its limit",
            ));
        }
        Ok(bytes)
    }

    /// A diagnostic path only; it must never authorize cleanup or replacement.
    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn fingerprint(&mut self) -> io::Result<FileFingerprint> {
        let before_identity = file_identity(&self.file)?;
        let before = self.file.metadata()?;
        let mut input = self.clone_at_start()?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut count = 0_u64;
        loop {
            let size = input.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            count = count
                .checked_add(size as u64)
                .ok_or_else(|| io::Error::other("File length overflow while fingerprinting"))?;
            digest.update(&buffer[..size]);
        }
        let after_identity = file_identity(&self.file)?;
        let after = self.file.metadata()?;
        if before_identity != after_identity
            || count != before.file_size()
            || before.file_size() != after.file_size()
            || before.last_write_time() != after.last_write_time()
        {
            return Err(io::Error::other(
                "The held file changed while fingerprinting",
            ));
        }
        Ok(FileFingerprint {
            volume_serial: after_identity.volume_serial,
            file_id: after_identity.file_id,
            size: after.file_size(),
            modified: after.last_write_time(),
            sha256: format!("{:x}", digest.finalize()),
        })
    }

    pub(super) fn copy_to(&mut self, target: &Path) -> io::Result<Self> {
        let mut output = Self::create_new(target)?;
        io::copy(&mut self.clone_at_start()?, &mut output.file)?;
        output.file.sync_all()?;
        // A failed copy leaves its create-new output for inspection, never
        // removes a path that might later refer to an unrelated object.
        Ok(output)
    }

    pub(super) fn rename_no_replace(&mut self, target: &Path) -> io::Result<()> {
        self.path = Self::rename_held_no_replace(&self.file, target)?;
        Ok(())
    }

    fn rename_held_no_replace(file: &File, target: &Path) -> io::Result<PathBuf> {
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
            // The public Win32 contract also describes FileName as NUL
            // terminated. Reserve the terminator explicitly for every buffer
            // alignment; allocator padding is not a string terminator.
            .and_then(|bytes| bytes.checked_add(size_of::<u16>()))
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
                file.as_raw_handle(),
                FILE_RENAME_INFO_CLASS,
                info.cast(),
                buffer_size,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(target)
    }

    pub(super) fn delete_owned(self) -> io::Result<()> {
        if !self.created_here {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "A pre-existing object has no primitive cleanup authorization",
            ));
        }
        delete_held(&self.file)
    }

    /// The caller must first prove transaction authorization and raw preservation.
    /// Matching content/identity alone is not a journal or commit authorization.
    pub(super) fn delete_matching(mut self, expected: &FileFingerprint) -> io::Result<()> {
        if self.fingerprint()? != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "File fingerprint mismatch; original preserved",
            ));
        }
        delete_held(&self.file)
    }
}

#[derive(Debug)]
pub(super) struct ExclusiveBundle {
    role_paths: [PathBuf; 3],
    files: [Option<WindowsExclusiveFile>; 3],
}

impl ExclusiveBundle {
    pub(super) fn lock_database(database: &Path) -> io::Result<Self> {
        Self::lock_mapped(super::companions(database))
    }

    /// Only a pending first-install caller may authorize a missing DB role.
    pub(super) fn lock_optional_database(database: &Path) -> io::Result<Self> {
        Self::lock_roles(super::companions(database), true)
    }

    pub(super) fn lock_mapped(role_paths: [PathBuf; 3]) -> io::Result<Self> {
        Self::lock_roles(role_paths, false)
    }

    fn lock_roles(role_paths: [PathBuf; 3], allow_missing_database: bool) -> io::Result<Self> {
        let mut files = [None, None, None];
        for ordinal in 0..3 {
            files[ordinal] = match WindowsExclusiveFile::lock_existing(&role_paths[ordinal]) {
                Ok(file) => Some(file),
                // Only FILE_NOT_FOUND represents an optional missing object;
                // PATH_NOT_FOUND, sharing, permission and other errors abort.
                Err(error)
                    if error.raw_os_error() == Some(2)
                        && (ordinal != 0 || allow_missing_database) =>
                {
                    None
                }
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

    pub(super) fn check_missing_sidecars(&self) -> io::Result<()> {
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

    pub(super) fn files_mut(&mut self) -> &mut [Option<WindowsExclusiveFile>; 3] {
        &mut self.files
    }

    #[cfg(test)]
    pub(super) fn into_files(self) -> [Option<WindowsExclusiveFile>; 3] {
        self.files
    }

    #[cfg(test)]
    pub(super) fn copy_probe_into(&mut self, directory: &Path) -> io::Result<PathBuf> {
        if self.files[0].is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No held DB role is available to probe",
            ));
        }
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

#[derive(Debug)]
pub(super) struct DirectoryGuard {
    file: File,
    path: PathBuf,
    delete_capable: bool,
    created_identity: Option<DirectoryIdentity>,
}

impl DirectoryGuard {
    fn open_existing(path: &Path, delete_capable: bool) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(invalid_input("An absolute directory path is required"));
        }
        if path.file_name().is_some() {
            check_parent_paths(path)?;
        }
        // Attribute-only opens are exempt from Windows sharing checks and do
        // not prevent a concurrent directory rename. LIST_DIRECTORY participates
        // as read access, so omitting SHARE_DELETE protects this directory.
        let access = FILE_READ_ATTRIBUTES
            | FILE_LIST_DIRECTORY
            | if delete_capable { DELETE_ACCESS } else { 0 };
        let file = OpenOptions::new()
            .access_mode(access)
            .share_mode(SHARE_READ_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_input("A non-reparse directory is required"));
        }
        file_identity(&file)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            delete_capable,
            created_identity: None,
        })
    }

    pub(super) fn lock_existing(path: &Path) -> io::Result<Self> {
        Self::open_existing(path, false)
    }

    /// A validated journal must authorize every destructive use of this guard.
    pub(super) fn lock_existing_mutable(path: &Path) -> io::Result<Self> {
        Self::open_existing(path, true)
    }

    pub(super) fn lock_ancestors(path: &Path) -> io::Result<Vec<Self>> {
        if !path.is_absolute() {
            return Err(invalid_input("An absolute guarded path is required"));
        }
        let mut ancestors: Vec<_> = path.ancestors().skip(1).collect();
        ancestors.reverse();
        ancestors.into_iter().map(Self::lock_existing).collect()
    }

    /// Atomically create a directory and receive its handle. A path-based
    /// CreateDirectory followed by open would not prove creation ownership.
    #[cfg(test)]
    pub(super) fn create_new(path: &Path) -> io::Result<Self> {
        check_parent_paths(path)?;
        let parent = Self::lock_existing(
            path.parent()
                .ok_or_else(|| invalid_input("Missing directory parent"))?,
        )?;
        Self::create_new_in(
            &parent,
            path.file_name()
                .ok_or_else(|| invalid_input("A single directory basename is required"))?,
        )
    }

    /// Reuse a held parent, including a DELETE-capable transaction directory.
    /// Reopening that parent with share3 would conflict with its own guard.
    pub(super) fn create_new_in(parent: &Self, basename: &std::ffi::OsStr) -> io::Result<Self> {
        let mut name: Vec<u16> = basename.encode_wide().collect();
        if basename == std::ffi::OsStr::new(".")
            || basename == std::ffi::OsStr::new("..")
            || name.is_empty()
            || name.iter().any(|value| matches!(*value, 0 | 47 | 58 | 92))
        {
            return Err(invalid_input(
                "The directory basename is not a single NT name",
            ));
        }
        let length = name
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|bytes| u16::try_from(bytes).ok())
            .ok_or_else(|| invalid_input("The directory basename exceeds UNICODE_STRING"))?;
        let mut unicode = UnicodeString {
            length,
            maximum_length: length,
            buffer: name.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: size_of::<ObjectAttributes>() as u32,
            root_directory: parent.file.as_raw_handle(),
            object_name: &mut unicode,
            attributes: 0x40, // OBJ_CASE_INSENSITIVE
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut status_block = IoStatusBlock {
            status_or_pointer: 0,
            information: 0,
        };
        let mut handle = std::ptr::null_mut();
        // SAFETY: all SDK-layout inputs and their UTF-16 backing storage live
        // for the synchronous call. RootDirectory is held, ObjectName is exactly
        // one basename, FILE_CREATE cannot adopt or replace an existing object.
        // No OPEN_REPARSE_POINT option is needed for an atomically NEW object;
        // the held non-reparse parent and post-open metadata are still checked.
        let status = unsafe {
            nt_create_file(
                &mut handle,
                FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY | DELETE_ACCESS | 0x0010_0000, // SYNCHRONIZE
                &mut attributes,
                &mut status_block,
                std::ptr::null_mut(),
                0x80, // FILE_ATTRIBUTE_NORMAL
                SHARE_READ_WRITE,
                2,        // FILE_CREATE
                1 | 0x20, // FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT
                std::ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            // NtCreateFile returns NTSTATUS; GetLastError here would be stale.
            let error = unsafe { rtl_nt_status_to_dos_error(status) };
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        if handle.is_null() || handle as isize == -1 {
            return Err(io::Error::other(
                "NT creation returned no owned directory handle",
            ));
        }
        // SAFETY: a successful NT call transferred this newly opened handle to
        // the caller. File is its unique Rust owner and closes every error path.
        let file = unsafe { File::from_raw_handle(handle) };
        if status_block.information != 2 {
            // FILE_CREATED
            return Err(io::Error::other(
                "NT creation did not prove FILE_CREATED ownership",
            ));
        }
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid_input(
                "The new directory handle is not an ordinary directory",
            ));
        }
        let created_identity = file_identity(&file)?;
        Ok(Self {
            file,
            path: parent.path.join(basename),
            delete_capable: true,
            created_identity: Some(created_identity),
        })
    }

    pub(super) fn identity(&self) -> io::Result<DirectoryIdentity> {
        file_identity(&self.file)
    }

    /// Diagnostic only: directory identity and journal authorization are separate.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(test)]
    fn rename_no_replace(&mut self, target: &Path) -> io::Result<()> {
        if !self.delete_capable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Directory guard has no rename authorization",
            ));
        }
        self.path = WindowsExclusiveFile::rename_held_no_replace(&self.file, target)?;
        Ok(())
    }

    pub(super) fn delete_owned_empty(self, expected: &DirectoryIdentity) -> io::Result<()> {
        if self.created_identity.as_ref() != Some(expected) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Directory was not atomically created by this guard",
            ));
        }
        self.delete_matching_empty(expected)
    }

    /// Matching identity is necessary, never sufficient transaction authority.
    /// The caller must validate the persistent journal before calling this.
    pub(super) fn delete_matching_empty(self, expected: &DirectoryIdentity) -> io::Result<()> {
        if !self.delete_capable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Read-only directory guard cannot delete",
            ));
        }
        if self.identity()? != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Directory identity mismatch; directory preserved",
            ));
        }
        delete_held(&self.file)
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

    #[test]
    fn fingerprint_detects_a_real_commit_on_the_same_file_id_and_guards_deletion() {
        let root = fixture("fingerprint");
        let database = root.join("versioned.sqlite3");
        let conn = Connection::open(&database).unwrap();
        conn.execute_batch("CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('before'); CREATE TABLE payload(value BLOB); INSERT INTO payload VALUES(zeroblob(200000));").unwrap();
        drop(conn);
        let mut first = WindowsExclusiveFile::lock_existing(&database).unwrap();
        let before = first.fingerprint().unwrap();
        assert!(before.size > 64 * 1024);
        assert_eq!(before, first.fingerprint().unwrap());
        assert_eq!(first.path(), database);
        drop(first);

        let conn = Connection::open(&database).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE; UPDATE marker SET value='after-real-commit'; COMMIT;")
            .unwrap();
        drop(conn);
        let mut updated = WindowsExclusiveFile::lock_existing(&database).unwrap();
        let after = updated.fingerprint().unwrap();
        assert_eq!(before.volume_serial, after.volume_serial);
        assert_eq!(before.file_id, after.file_id);
        assert_ne!(before.sha256, after.sha256);
        assert_eq!(
            updated.delete_matching(&before).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let conn =
            Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(
            conn.query_row("SELECT value FROM marker", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "after-real-commit"
        );
        drop(conn);
        let preserved = root.join("preserved.raw");
        fs::copy(&database, &preserved).unwrap();
        let preserved_bytes = fs::read(&preserved).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&preserved_bytes)),
            after.sha256
        );
        let current = WindowsExclusiveFile::lock_existing(&database).unwrap();
        current.delete_matching(&after).unwrap();
        assert!(!database.exists());
        assert_eq!(fs::read(&preserved).unwrap(), preserved_bytes);
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn held_metadata_reads_enforce_the_explicit_limit() {
        let root = fixture("limited-read");
        let path = root.join("manifest.json");
        let mut held = WindowsExclusiveFile::create_new(&path).unwrap();
        held.write_created(b"12345").unwrap();
        assert_eq!(
            held.read_limited(4).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(held.read_limited(5).unwrap(), b"12345");
        held.write_created(b"").unwrap();
        assert!(held.read_limited(0).unwrap().is_empty());
        held.delete_owned().unwrap();
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn optional_database_still_locks_existing_sidecars_and_transfers_only_held_roles() {
        let root = fixture("optional-db");
        let database = root.join("missing.sqlite3");
        let wal = database.with_extension("sqlite3-wal");
        let moved = root.join("held-wal.raw");
        fs::write(&wal, b"existing sidecar without a main file").unwrap();
        let mut bundle = ExclusiveBundle::lock_optional_database(&database).unwrap();
        assert!(bundle.files_mut()[0].is_none());
        sharing_denied(File::open(&wal).unwrap_err());
        assert_eq!(
            bundle
                .copy_probe_into(&root.join("no-db-probe"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!root.join("no-db-probe").exists());
        bundle.check_missing_sidecars().unwrap();
        bundle.files_mut()[1]
            .as_mut()
            .unwrap()
            .rename_no_replace(&moved)
            .unwrap();
        let mut held_roles = bundle.into_files();
        assert_eq!(
            held_roles[1].as_mut().unwrap().read_bytes().unwrap(),
            b"existing sidecar without a main file"
        );
        sharing_denied(File::open(&moved).unwrap_err());
        fs::write(&wal, b"foreign new sidecar at the old name").unwrap();
        drop(held_roles);
        assert_eq!(
            fs::read(&moved).unwrap(),
            b"existing sidecar without a main file"
        );
        assert_eq!(
            fs::read(&wal).unwrap(),
            b"foreign new sidecar at the old name"
        );
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn directory_and_ancestor_guards_block_rename_but_allow_child_creation() {
        let root = fixture("directory-guard");
        let directory = root.join("existing");
        let moved = root.join("renamed-existing");
        fs::create_dir(&directory).unwrap();
        let guard = DirectoryGuard::lock_existing(&directory).unwrap();
        let identity = guard.identity().unwrap();
        assert_eq!(guard.path(), directory);
        sharing_denied(fs::rename(&directory, &moved).unwrap_err());
        fs::write(
            directory.join("new-child"),
            b"a directory guard is not a namespace lock",
        )
        .unwrap();
        sharing_denied(DirectoryGuard::lock_existing_mutable(&directory).unwrap_err());
        assert_eq!(
            guard.delete_matching_empty(&identity).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        let ancestors = DirectoryGuard::lock_ancestors(&directory.join("planned.sqlite3")).unwrap();
        assert_eq!(ancestors.last().unwrap().path(), directory);
        sharing_denied(fs::rename(&directory, &moved).unwrap_err());
        fs::write(
            directory.join("another-child"),
            b"new child remains possible",
        )
        .unwrap();
        drop(ancestors);
        fs::rename(&directory, &moved).unwrap();
        assert_eq!(
            fs::read(moved.join("new-child")).unwrap(),
            b"a directory guard is not a namespace lock"
        );
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn rename_targets_of_every_utf16_buffer_alignment_are_exact() {
        let root = fixture("rename-alignment");
        for padding in 0..8 {
            let source = root.join(format!("source-{padding}"));
            let target_name = format!("精确目标-{}", "x".repeat(padding));
            let target = root.join(&target_name);
            let mut file = WindowsExclusiveFile::create_new(&source).unwrap();
            file.write_created(b"alignment sentinel").unwrap();
            let before = file.fingerprint().unwrap();
            file.rename_no_replace(&target).unwrap();
            assert_eq!(file.fingerprint().unwrap(), before);
            drop(file);
            assert_eq!(fs::read(&target).unwrap(), b"alignment sentinel");
            assert!(fs::read_dir(&root.root)
                .unwrap()
                .any(|entry| entry.unwrap().file_name() == std::ffi::OsStr::new(&target_name)));
            assert!(!source.exists());
        }
        assert_eq!(fs::read_dir(&root.root).unwrap().count(), 9);
        remove_fixture(&root).unwrap();
    }

    #[test]
    fn atomic_directory_creation_refuses_existing_names_and_empty_cleanup_is_guarded() {
        let root = fixture("atomic-directory");
        let foreign = root.join("foreign");
        fs::create_dir(&foreign).unwrap();
        fs::write(foreign.join("sentinel"), b"foreign directory contents").unwrap();
        existing_target(DirectoryGuard::create_new(&foreign).unwrap_err());
        let borrowed = DirectoryGuard::lock_existing_mutable(&foreign).unwrap();
        let foreign_identity = borrowed.identity().unwrap();
        assert_eq!(
            borrowed
                .delete_owned_empty(&foreign_identity)
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            fs::read(foreign.join("sentinel")).unwrap(),
            b"foreign directory contents"
        );
        let blocked_parent = root.join("ordinary-file-parent");
        fs::write(&blocked_parent, b"must not adopt or change this file").unwrap();
        assert!(DirectoryGuard::create_new(&blocked_parent.join("child")).is_err());
        assert_eq!(
            fs::read(&blocked_parent).unwrap(),
            b"must not adopt or change this file"
        );

        let owned = root.join("owned");
        let renamed = root.join("owned-renamed");
        let mut guard = DirectoryGuard::create_new(&owned).unwrap();
        let identity = guard.identity().unwrap();
        existing_target(guard.rename_no_replace(&foreign).unwrap_err());
        guard.rename_no_replace(&renamed).unwrap();
        assert_eq!(guard.identity().unwrap(), identity);
        fs::write(
            renamed.join("child"),
            b"nonempty must survive deletion attempt",
        )
        .unwrap();
        assert!(guard.delete_owned_empty(&identity).is_err());
        assert_eq!(
            fs::read(renamed.join("child")).unwrap(),
            b"nonempty must survive deletion attempt"
        );
        fs::remove_file(renamed.join("child")).unwrap();
        let journal_authorized = DirectoryGuard::lock_existing_mutable(&renamed).unwrap();
        journal_authorized.delete_matching_empty(&identity).unwrap();
        assert!(!renamed.exists());
        let empty = root.join("owned-empty");
        let owned_empty = DirectoryGuard::create_new(&empty).unwrap();
        let empty_identity = owned_empty.identity().unwrap();
        owned_empty.delete_owned_empty(&empty_identity).unwrap();
        assert!(!empty.exists());
        let owned_parent = DirectoryGuard::create_new(&root.join("owned-parent")).unwrap();
        let parent_identity = owned_parent.identity().unwrap();
        let owned_child =
            DirectoryGuard::create_new_in(&owned_parent, std::ffi::OsStr::new("owned-child"))
                .unwrap();
        let child_identity = owned_child.identity().unwrap();
        owned_child.delete_owned_empty(&child_identity).unwrap();
        owned_parent.delete_owned_empty(&parent_identity).unwrap();
        assert_eq!(
            fs::read(foreign.join("sentinel")).unwrap(),
            b"foreign directory contents"
        );
        remove_fixture(&root).unwrap();
    }
}
