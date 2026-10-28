//! Real std file bytes, shared cursor, thread access, append, sync and reopen.
use ogc_system_probe::probe_support::ProbeLogger;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::thread;

#[derive(Clone, Copy)]
pub enum Medium {
    Sd,
    Usb,
}
static RUN_TAG: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn initialize_run_tag(logger: &mut ProbeLogger) -> bool {
    if RUN_TAG.get().is_some() {
        return true;
    }
    let tag = match crate::std_probe_config::value("run-id") {
        Ok(Some(tag))
            if tag.len() <= 64
                && tag
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') =>
        {
            tag
        }
        Ok(None) => format!("{:016x}", ogc_lwp_driver::now_ticks()),
        Ok(Some(_)) => {
            logger.record("filesystem run ID has valid path bytes", false);
            return false;
        }
        Err(error) => {
            logger.record_with_detail("filesystem run ID is valid", false, error);
            return false;
        }
    };
    logger.note(&format!("Exclusive filesystem run tag: {tag}"));
    if RUN_TAG.set(tag).is_err() {
        logger.record("filesystem run tag is initialized once", false);
        return false;
    }
    true
}

impl Medium {
    fn name(self) -> &'static std::ffi::CStr {
        match self {
            Self::Sd => c"sd",
            Self::Usb => c"usb",
        }
    }
    fn path(self, tail: &str) -> PathBuf {
        let (head, rest) = tail.split_once('/').unwrap_or((tail, ""));
        let tail = if matches!(
            head,
            "wii-std-fs-probe.tmp"
                | "wii-std-fs-probe-dir"
                | "wii-std-path-probe"
                | "wii-std-fat-create.tmp"
                | "wii-std-fat-ops"
        ) {
            let tag = RUN_TAG.get().expect("filesystem run tag initialized");
            format!(
                "{head}-{tag}{}{}",
                if rest.is_empty() { "" } else { "/" },
                rest
            )
        } else {
            tail.to_owned()
        };
        PathBuf::from(format!(
            "{}:/{tail}",
            self.name().to_str().expect("ASCII mount")
        ))
    }
    fn cpath(self, tail: &str) -> std::ffi::CString {
        std::ffi::CString::new(self.path(tail).as_os_str().as_encoded_bytes())
            .expect("probe path has no NUL")
    }
    fn mount(self) -> Result<(), ogc_fat::FatError> {
        match self {
            Self::Sd => ogc_fat::mount_sd(),
            Self::Usb => ogc_fat::mount_usb(),
        }
    }
    fn other(self) -> Self {
        match self {
            Self::Sd => Self::Usb,
            Self::Usb => Self::Sd,
        }
    }
}

const INITIAL: &[u8] = b"Wii std file probe\n";
const APPENDED: &[u8] = b"Wii std file probe\nappend\n";

fn raw_provider(logger: &mut ProbeLogger) {
    logger.record(
        "filesystem ABI rejects a null request",
        unsafe { ogc_std_runtime::__wii_std_fs(std::ptr::null_mut()) } == -22,
    );
    let mut request = ogc_std_runtime::FsRequest::new(99, std::ptr::null_mut());
    logger.record(
        "filesystem ABI rejects an unknown operation",
        unsafe { ogc_std_runtime::__wii_std_fs(&mut request) } == -134,
    );
    logger.record(
        "FAT errors translate into the Newlib code space",
        ogc_fat::fat_errno(ogc_fat::FatError::Busy) == 16
            && ogc_fat::fat_errno(ogc_fat::FatError::MediaRemoved) == 19
            && ogc_fat::fat_errno(ogc_fat::FatError::FileTooLarge) == 27,
    );
}

fn typed_file_checks(logger: &mut ProbeLogger, medium: Medium) -> Result<(), ogc_fat::FatError> {
    let mut file = ogc_fat::FatFile::open_with_flags(&medium.cpath("wii-std-fs-probe.tmp"), 0)?;
    logger.record("typed open starts at offset zero", file.position() == 0);
    file.seek_checked(4)?;
    let mut bytes = [0; 4];
    let count = file.read(&mut bytes)?;
    logger.record(
        "typed seek reads the persisted offset",
        count == 4 && bytes == *b"std " && file.position() == 8,
    );
    let (size, readonly) = file.metadata()?;
    logger.record(
        "typed metadata reads the same stored file",
        size == 26 && !readonly,
    );
    file.set_times(Some(946684800), Some(946730097))?;
    logger.record(
        "typed timestamp setter writes selected stored fields",
        file.timestamps()?.modified == Some(946730096)
            && ogc_fat::get_times(&medium.cpath("wii-std-fs-probe.tmp"))?.accessed
                == Some(946684800),
    );
    ogc_fat::set_times(&medium.cpath("wii-std-fs-probe.tmp"), None, Some(946730096))?;
    file.flush()?;
    logger.record(
        "read-only flush preserves size and attributes",
        file.metadata()? == (26, false),
    );
    file.close()?;
    let reopened = ogc_fat::FatFile::open(&medium.cpath("wii-std-fs-probe.tmp"))?;
    logger.record(
        "read-only close preserves persisted metadata",
        reopened.metadata()? == (26, false),
    );
    reopened.close()?;
    typed_create_checks(logger, medium)?;
    typed_path_operations_checks(logger, medium)?;
    Ok(())
}

fn typed_create_checks(logger: &mut ProbeLogger, medium: Medium) -> Result<(), ogc_fat::FatError> {
    const CREATED: &[u8] = b"typed create_new persisted bytes\n";
    const REPLACEMENT: &[u8] = b"short replacement\n";
    let path = medium.cpath("wii-std-fat-create.tmp");
    let mut owns_path = false;

    let result = (|| {
        let mut created = ogc_fat::FatFile::create_new(&path)?;
        owns_path = true;
        logger.record("typed create_new starts empty", created.is_empty());
        logger.record(
            "typed create_new writes the full payload",
            created.write(CREATED)? == CREATED.len(),
        );
        created.flush()?;
        created.close()?;

        let mut reopened = ogc_fat::FatFile::open(&path)?;
        let mut persisted = [0; CREATED.len()];
        let count = reopened.read(&mut persisted)?;
        logger.record(
            "typed create_new bytes survive close and reopen",
            count == CREATED.len() && persisted == *CREATED,
        );
        reopened.close()?;

        let duplicate = match ogc_fat::FatFile::create_new(&path) {
            Err(ogc_fat::FatError::AlreadyExists) => true,
            Err(_) => false,
            Ok(file) => {
                file.close()?;
                false
            }
        };
        logger.record(
            "typed create_new rejects an existing file without truncation",
            duplicate,
        );
        let mut reopened = ogc_fat::FatFile::open(&path)?;
        let mut persisted = [0; CREATED.len()];
        let count = reopened.read(&mut persisted)?;
        logger.record(
            "typed create_new failure preserves stored bytes",
            count == CREATED.len() && persisted == *CREATED,
        );
        reopened.close()?;

        let mut truncated = ogc_fat::FatFile::create_truncate(&path)?;
        logger.record(
            "typed create_truncate clears the previous size",
            truncated.is_empty(),
        );
        logger.record(
            "typed create_truncate writes the replacement",
            truncated.write(REPLACEMENT)? == REPLACEMENT.len(),
        );
        truncated.flush()?;
        truncated.close()?;

        let mut reopened = ogc_fat::FatFile::open(&path)?;
        let mut persisted = [0; 32];
        let count = reopened.read(&mut persisted)?;
        logger.record(
            "typed create_truncate bytes survive close and reopen",
            count == REPLACEMENT.len()
                && &persisted[..count] == REPLACEMENT
                && reopened.read(&mut [0; 1])? == 0,
        );
        reopened.close()?;
        Ok(())
    })();

    let path_text = path.to_str().map_err(|_| ogc_fat::FatError::PathError)?;
    let cleanup = if owns_path {
        fs::remove_file(Path::new(path_text))
    } else {
        Ok(())
    };
    if let Err(error) = &cleanup {
        logger.note(&format!("typed create probe cleanup failed: {error}"));
    }
    let absent =
        fs::metadata(Path::new(path_text)).is_err_and(|error| error.raw_os_error() == Some(2));
    logger.record(
        "typed create probe cleanup succeeds",
        cleanup.is_ok() && absent,
    );
    if (cleanup.is_err() || !absent) && result.is_ok() {
        return Err(ogc_fat::FatError::Io);
    }
    result
}

fn typed_path_operations_checks(
    logger: &mut ProbeLogger,
    medium: Medium,
) -> Result<(), ogc_fat::FatError> {
    const CONTENT: &[u8] = b"typed rename persisted bytes\n";
    let directory = medium.cpath("wii-std-fat-ops");
    let source = medium.cpath("wii-std-fat-ops/source.bin");
    let destination = medium.cpath("wii-std-fat-ops/destination.bin");
    let missing = medium.cpath("wii-std-fat-ops/missing.bin");
    let mut owns_directory = false;
    let mut owns_source = false;
    let mut owns_destination = false;

    let result = (|| {
        ogc_fat::create_directory(&directory)?;
        owns_directory = true;
        logger.record(
            "typed create_directory creates a directory",
            ogc_fat::stat(&directory)?.is_directory,
        );

        let mut file = ogc_fat::FatFile::create_new(&source)?;
        owns_source = true;
        logger.record(
            "typed path operation writes the full payload",
            file.write(CONTENT)? == CONTENT.len(),
        );
        file.flush()?;
        file.close()?;

        ogc_fat::rename(&source, &destination)?;
        owns_source = false;
        owns_destination = true;
        let mut reopened = ogc_fat::FatFile::open(&destination)?;
        let mut bytes = [0; CONTENT.len()];
        let count = reopened.read(&mut bytes)?;
        logger.record(
            "typed rename preserves stored file bytes",
            count == CONTENT.len() && bytes == *CONTENT,
        );
        reopened.close()?;

        logger.record(
            "typed rename failure preserves destination bytes",
            matches!(
                ogc_fat::rename(&missing, &destination),
                Err(ogc_fat::FatError::NotFound)
            ),
        );
        logger.record(
            "typed remove_directory rejects non-empty directory",
            matches!(
                ogc_fat::remove_directory(&directory),
                Err(ogc_fat::FatError::DirectoryNotEmpty)
            ) && ogc_fat::stat(&destination)?.size == CONTENT.len() as u32,
        );

        ogc_fat::remove_file(&destination)?;
        owns_destination = false;
        logger.record(
            "typed remove_file removes the file",
            matches!(
                ogc_fat::stat(&destination),
                Err(ogc_fat::FatError::NotFound)
            ),
        );
        ogc_fat::remove_directory(&directory)?;
        owns_directory = false;
        logger.record(
            "typed remove_directory removes an empty directory",
            matches!(ogc_fat::stat(&directory), Err(ogc_fat::FatError::NotFound)),
        );
        Ok(())
    })();

    let mut cleanup_ok = true;
    if owns_source {
        cleanup_ok &= ogc_fat::remove_file(&source).is_ok();
    }
    if owns_destination {
        cleanup_ok &= ogc_fat::remove_file(&destination).is_ok();
    }
    if owns_directory {
        cleanup_ok &= ogc_fat::remove_directory(&directory).is_ok();
    }
    logger.record("typed path operation cleanup succeeds", cleanup_ok);
    if !cleanup_ok && result.is_ok() {
        return Err(ogc_fat::FatError::Io);
    }
    result
}

/// Change attributes only on the file obtained by create_new in run().
fn readonly_attribute_checks(
    logger: &mut ProbeLogger,
    medium: Medium,
) -> Result<(), ogc_fat::FatError> {
    let owned_path = medium.cpath("wii-std-fs-probe.tmp");
    let path = owned_path.as_c_str();
    let original = ogc_fat::get_attributes(path)?;
    // Clear archive so a mistaken metadata rewrite cannot pass this check.
    let readonly = (original & !ogc_fat::ATTR_ARCHIVE) | ogc_fat::ATTR_READONLY;
    let result = (|| {
        ogc_fat::set_attributes(path, readonly)?;
        let mut file = ogc_fat::FatFile::open(path)?;
        file.set_readonly(false)?;
        ogc_fat::set_readonly(path, true)?;
        logger.record(
            "typed and path permissions preserve other DOS attributes",
            ogc_fat::get_attributes(path)? == readonly,
        );
        logger.record(
            "FAT read-only attribute is reported",
            file.metadata()? == (26, true),
        );
        file.flush()?;
        file.close()?;
        logger.record(
            "read-only sync and close preserve all DOS attributes",
            ogc_fat::get_attributes(path)? == readonly,
        );
        match ogc_fat::FatFile::create(path) {
            Err(ogc_fat::FatError::ReadOnly) => {
                logger.record("read-only attribute rejects a writable open", true)
            }
            Err(error) => return Err(error),
            Ok(file) => {
                logger.record("read-only attribute rejects a writable open", false);
                file.close()?;
            }
        }
        Ok(())
    })();
    // Restore even if set_attributes wrote its sector before a transport error.
    let restored = ogc_fat::set_attributes(path, original);
    if let Err(error) = &restored {
        logger.note(&format!("probe file attribute restore failed: {error:?}"));
        logger.record("probe file attributes restored", false);
    }
    result.and(restored)
}

fn remount_check(logger: &mut ProbeLogger, path: &Path, medium: Medium) -> Result<(), String> {
    logger.note("medium remount check: unmount starting");
    ogc_fat::unmount(medium.name()).map_err(|error| format!("medium unmount failed: {error:?}"))?;
    logger.note("medium remount check: mount starting");
    medium
        .mount()
        .map_err(|error| format!("medium remount failed: {error:?}"))?;
    logger.note("medium remount check: persisted read starting");
    let bytes = fs::read(path).map_err(|error| format!("read after remount failed: {error}"))?;
    logger.record(
        "file bytes survive a medium unmount and remount",
        bytes == APPENDED,
    );
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("timestamp read after remount failed: {error}"))?;
    logger.record(
        "timestamps survive medium unmount and remount",
        modified == std::time::UNIX_EPOCH + std::time::Duration::from_secs(946730096),
    );
    Ok(())
}

fn progress(logger: &mut ProbeLogger, operation: &str) {
    logger.note(&format!("filesystem operation: {operation}"));
    ogc_system_probe::probe_support::send_agent_progress(6);
}

fn directory_checks(logger: &mut ProbeLogger, medium: Medium) -> io::Result<()> {
    let directory_path = medium.path("wii-std-fs-probe-dir");
    let path = directory_path.as_path();
    progress(logger, "create owned directory");
    fs::create_dir(path)?;
    let child = path.join("entry.bin");
    let copied = path.join("copy.bin");
    let mut owned_copy = false;
    let mut owned_child = false;
    let result = (|| {
        logger.record(
            "directory creation produces a real directory",
            fs::metadata(path)?.is_dir(),
        );
        let mut empty = fs::read_dir(path)?;
        logger.record(
            "empty directory traversal excludes dot entries",
            empty.next().transpose()?.is_none() && empty.next().transpose()?.is_none(),
        );
        logger.record(
            "directory iterator keeps the mount busy",
            ogc_fat::unmount(medium.name()) == Err(ogc_fat::FatError::Busy),
        );
        progress(logger, "close empty directory iterator");
        drop(empty);
        progress(logger, "create directory child");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&child)?;
        owned_child = true;
        progress(logger, "write directory child");
        file.write_all(b"directory bytes\n")?;
        progress(logger, "sync directory child");
        file.sync_all()?;
        drop(file);
        progress(logger, "open typed directory");
        let mut typed = ogc_fat::FatDirectory::open(&medium.cpath("wii-std-fs-probe-dir"))
            .map_err(|error| io::Error::from_raw_os_error(ogc_fat::fat_errno(error)))?;
        let mut names = Vec::new();
        while let Some(entry) = typed
            .next_entry()
            .map_err(|error| io::Error::from_raw_os_error(ogc_fat::fat_errno(error)))?
        {
            if entry.name != b"." && entry.name != b".." {
                logger.record(
                    "typed directory entry has stored size and type",
                    !entry.is_directory && entry.size == 16,
                );
                names.push(entry.name);
            }
        }
        logger.record(
            "typed traversal returns the same stored filename",
            names == [b"entry.bin".to_vec()],
        );
        drop(typed);
        logger.record(
            "read_dir on a file returns ENOTDIR",
            fs::read_dir(&child).is_err_and(|error| error.raw_os_error() == Some(20)),
        );
        let mut entries = fs::read_dir(path)?;
        let entry = entries
            .next()
            .ok_or_else(|| io::Error::other("missing directory entry"))??;
        logger.record(
            "directory traversal returns the stored name and path",
            entry.file_name() == "entry.bin" && entry.path() == child,
        );
        logger.record(
            "directory entry type and metadata use the real file",
            entry.file_type()?.is_file()
                && entry.metadata()?.is_file()
                && entry.metadata()?.len() == 16,
        );
        logger.record(
            "directory traversal reaches a stable end",
            entries.next().transpose()?.is_none() && entries.next().transpose()?.is_none(),
        );
        drop(entries);
        logger.record(
            "entry metadata remains available after the iterator closes",
            entry.metadata()?.len() == 16 && fs::read(entry.path())? == b"directory bytes\n",
        );
        // Reserve the destination before copy is permitted to truncate it.
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copied)?;
        owned_copy = true;
        destination.write_all(b"old destination bytes must be truncated")?;
        destination.sync_all()?;
        drop(destination);
        logger.record(
            "copy returns the byte count and persisted source bytes",
            fs::copy(&child, &copied)? == 16
                && fs::read(&copied)? == b"directory bytes\n"
                && fs::metadata(&copied)?.len() == 16,
        );
        logger.record(
            "copy to the same FAT entry fails before truncation",
            fs::copy(&child, &child).is_err_and(|error| error.raw_os_error() == Some(16))
                && fs::read(&child)? == b"directory bytes\n",
        );
        logger.record(
            "copy of a directory preserves the destination",
            fs::copy(path, &copied).is_err() && fs::read(&copied)? == b"directory bytes\n",
        );
        logger.record(
            "copy of a missing source preserves the destination",
            fs::copy(path.join("missing"), &copied)
                .is_err_and(|error| error.raw_os_error() == Some(2))
                && fs::read(&copied)? == b"directory bytes\n",
        );
        logger.record(
            "missing directory returns ENOENT",
            fs::read_dir(path.join("missing")).is_err_and(|error| error.raw_os_error() == Some(2)),
        );
        Ok(())
    })();
    // Remove only the directory and file obtained by this probe.
    let child_cleanup = if owned_child {
        fs::remove_file(&child)
    } else {
        Ok(())
    };
    if let Err(error) = &child_cleanup {
        logger.note(&format!("owned directory child cleanup failed: {error}"));
        logger.record("owned directory child cleanup succeeds", false);
    }
    let copy_cleanup = if owned_copy {
        fs::remove_file(&copied)
    } else {
        Ok(())
    };
    if let Err(error) = &copy_cleanup {
        logger.note(&format!("owned copy cleanup failed: {error}"));
        logger.record("owned copy cleanup succeeds", false);
    }
    let removed = fs::remove_dir(path);
    if let Err(error) = &removed {
        logger.note(&format!("owned directory cleanup failed: {error}"));
    }
    logger.record(
        "empty directory removal removes its path",
        removed.is_ok() && fs::metadata(path).is_err_and(|error| error.raw_os_error() == Some(2)),
    );
    result.and(child_cleanup).and(copy_cleanup).and(removed)
}

fn file_checks(
    logger: &mut ProbeLogger,
    path: &Path,
    mut file: File,
    medium: Medium,
) -> io::Result<()> {
    let info = file.metadata()?;
    logger.record(
        "new file has zero length",
        info.is_file() && info.len() == 0,
    );
    logger.record(
        "disabling the last access option rejects the open",
        OpenOptions::new()
            .read(true)
            .read(false)
            .open(path)
            .is_err_and(|e| e.kind() == io::ErrorKind::InvalidInput),
    );
    logger.record(
        "append and truncate require create_new",
        OpenOptions::new()
            .append(true)
            .truncate(true)
            .open(path)
            .is_err_and(|e| e.kind() == io::ErrorKind::InvalidInput),
    );
    let conflict = OpenOptions::new()
        .write(true)
        .create_new(true)
        .truncate(true)
        .open(path);
    logger.record(
        "create_new rejects existing path without truncation",
        conflict.as_ref().is_err_and(|e| {
            e.kind() == io::ErrorKind::AlreadyExists && e.raw_os_error() == Some(17)
        }),
    );
    drop(conflict);
    file.write_all(INITIAL)?;
    logger.record(
        "path metadata sees writes before an explicit flush",
        fs::metadata(path)?.len() == 19
            && ogc_fat::stat(&medium.cpath("wii-std-fs-probe.tmp"))
                .is_ok_and(|info| info.size == 19),
    );
    file.flush()?;
    file.sync_data()?;
    file.sync_all()?;
    let mut clone = file.try_clone()?;
    clone.seek(SeekFrom::Start(0))?;
    let mut first = [0; 4];
    file.read_exact(&mut first)?;
    logger.record(
        "clones use one shared cursor",
        first == *b"Wii " && clone.stream_position()? == 4,
    );
    let worker = thread::Builder::new().spawn(move || -> io::Result<(Vec<u8>, u64)> {
        let mut rest = Vec::new();
        clone.read_to_end(&mut rest)?;
        Ok((rest, clone.stream_position()?))
    })?;
    let (rest, position) = worker
        .join()
        .map_err(|_| io::Error::other("file worker panicked"))??;
    logger.record(
        "worker reads the remaining bytes from the shared cursor",
        rest == b"std file probe\n" && position == 19 && file.stream_position()? == 19,
    );
    logger.record(
        "open handle keeps the mount busy",
        ogc_fat::unmount(medium.name()) == Err(ogc_fat::FatError::Busy),
    );
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    logger.record(
        "file content equals independent expected bytes",
        bytes == INITIAL,
    );
    logger.record(
        "metadata reports real size",
        file.metadata()?.len() == 19 && fs::metadata(path)?.len() == 19,
    );
    logger.record(
        "unsupported timestamps return an error",
        file.metadata()?
            .modified()
            .is_err_and(|e| e.kind() == io::ErrorKind::Unsupported),
    );
    let access = std::time::UNIX_EPOCH + std::time::Duration::from_secs(946684800);
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(946730097);
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(access)
            .set_modified(modified),
    )?;
    logger.record(
        "file timestamp writes use FAT day and two-second resolution",
        file.metadata()?.accessed()? == access
            && file.metadata()?.modified()?
                == std::time::UNIX_EPOCH + std::time::Duration::from_secs(946730096),
    );
    logger.record(
        "invalid timestamp leaves both timestamp fields unchanged",
        file.set_times(
            std::fs::FileTimes::new()
                .set_accessed(access)
                .set_modified(std::time::UNIX_EPOCH),
        )
        .is_err_and(|error| error.raw_os_error() == Some(22))
            && file.metadata()?.accessed()? == access
            && file.metadata()?.modified()?
                == std::time::UNIX_EPOCH + std::time::Duration::from_secs(946730096),
    );
    logger.record(
        "out-of-range seek preserves the cursor",
        file.seek(SeekFrom::Start(u64::MAX))
            .is_err_and(|e| e.raw_os_error() == Some(27))
            && file.stream_position()? == 19,
    );
    logger.record(
        "out-of-range resize preserves size and cursor",
        file.set_len(u64::MAX)
            .is_err_and(|e| e.raw_os_error() == Some(27))
            && file.metadata()?.len() == 19
            && file.stream_position()? == 19,
    );
    file.set_len(4)?;
    logger.record(
        "resize preserves the shared cursor",
        file.metadata()?.len() == 4 && file.stream_position()? == 19,
    );
    file.set_len(8)?;
    file.seek(SeekFrom::Start(0))?;
    let mut extended = [0xcc; 8];
    file.read_exact(&mut extended)?;
    logger.record(
        "file extension is zero-filled",
        extended == [b'W', b'i', b'i', b' ', 0, 0, 0, 0],
    );
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(INITIAL)?;
    file.sync_all()?;
    drop(file);

    let mut appended = OpenOptions::new().read(true).append(true).open(path)?;
    appended.seek(SeekFrom::Start(0))?;
    appended.write_all(b"append\n")?;
    appended.sync_all()?;
    drop(appended);
    let mut reopened = File::open(path)?;
    bytes.clear();
    reopened.read_to_end(&mut bytes)?;
    logger.record(
        "append writes at EOF after a seek, and bytes survive reopen",
        bytes == APPENDED,
    );
    let denied = reopened.write(b"must not write");
    logger.record(
        "read-only open preserves its access mode and errno",
        denied.is_err_and(|e| e.raw_os_error() == Some(9)),
    );
    drop(reopened);
    Ok(())
}

pub fn run(logger: &mut ProbeLogger) {
    run_medium(logger, Medium::Sd);
}

pub fn run_medium(logger: &mut ProbeLogger, medium: Medium) {
    if !initialize_run_tag(logger) {
        return;
    }
    logger.section(&format!(
        "STD FAT FILES AND PATHS: {}",
        medium.name().to_str().expect("ASCII mount")
    ));
    raw_provider(logger);
    logger.record(
        "std runtime is running",
        ogc_std_runtime::runtime_snapshot().phase == ogc_std_runtime::RuntimePhase::Running,
    );
    let root = medium.path("");
    let path = medium.path("wii-std-fs-probe.tmp");
    logger.record(
        "mounted root and joined path are absolute",
        root.is_absolute() && path.is_absolute(),
    );
    logger.record(
        "mounted paths have a prefix and a root component",
        matches!(path.components().next(), Some(Component::Prefix(_)))
            && matches!(path.components().nth(1), Some(Component::RootDir)),
    );
    logger.record(
        "rooted join keeps the mount prefix",
        path.join("/other") == medium.path("other"),
    );
    environment_checks(logger);
    logger.record(
        "NUL path is rejected",
        File::open("sd:/bad\0path").is_err_and(|e| e.kind() == io::ErrorKind::InvalidInput),
    );
    let owned_mount = !ogc_fat::is_mounted(medium.name());
    if owned_mount {
        progress(logger, "mount medium");
        if let Err(error) = medium.mount() {
            logger.note(&format!("medium mount failed: {error:?}"));
            let detail = if matches!(medium, Medium::Usb) {
                format!("usb: {error:?}; {:?}", ogc_usbstorage_sys::probe_status())
            } else {
                format!("sd: {error:?}")
            };
            logger.record_with_detail("medium mount is available", false, &detail);
            return;
        }
    }
    progress(logger, "directory acceptance");
    if let Err(error) = directory_checks(logger, medium) {
        logger.note(&format!(
            "directory checks failed: {error}; USB status={:?}",
            ogc_usbstorage_sys::probe_status()
        ));
        logger.record_with_detail("directory checks complete", false, &error.to_string());
    }
    progress(logger, "path service acceptance");
    if let Err(error) = path_service_checks(logger, medium) {
        logger.note(&format!(
            "path service checks failed: {error}; USB status={:?}",
            ogc_usbstorage_sys::probe_status()
        ));
        logger.record_with_detail("path service checks complete", false, &error.to_string());
    }
    let before = ogc_fat::file_close_failures();
    // The probe never opens an existing file for destructive writes. Cleanup
    // only removes this path after create_new grants ownership.
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => {
            if let Err(error) = file_checks(logger, &path, file, medium) {
                logger.note(&format!("file checks failed: {error}"));
                logger.record_with_detail("file checks complete", false, &error.to_string());
            }
            if let Err(error) = typed_file_checks(logger, medium) {
                logger.note(&format!("typed file checks failed: {error:?}"));
                logger.record_with_detail(
                    "typed file checks complete",
                    false,
                    &format!("{error:?}"),
                );
            }
            if let Err(error) = readonly_attribute_checks(logger, medium) {
                logger.note(&format!("read-only attribute checks failed: {error:?}"));
                logger.record_with_detail(
                    "read-only attribute checks complete",
                    false,
                    &format!("{error:?}"),
                );
            }
            if owned_mount {
                if let Err(error) = remount_check(logger, &path, medium) {
                    logger.note(&error);
                    logger.record_with_detail(
                        "medium remount and file read complete",
                        false,
                        &error,
                    );
                }
            } else {
                logger.note("medium remount was not run: the mount existed before this probe");
                logger.record("medium remount is required for acceptance", false);
            }
            let removed = fs::remove_file(&path);
            if let Err(error) = &removed {
                logger.note(&format!("owned file cleanup failed: {error}"));
            }
            logger.record(
                "owned probe file removal succeeds",
                removed.is_ok()
                    && fs::metadata(&path).is_err_and(|error| error.raw_os_error() == Some(2)),
            );
        }
        Err(error) => {
            logger.note(&format!(
                "create_new failed; existing path is preserved: {error}"
            ));
            logger.record("probe obtains a new file", false);
        }
    }
    logger.record(
        "file cleanup has no new failures",
        ogc_fat::file_close_failures() == before,
    );
    logger.note_i32("last file close errno", ogc_fat::last_file_close_errno());
    if owned_mount {
        logger.record(
            "owned medium unmount succeeds",
            ogc_fat::unmount(medium.name()).is_ok() && !ogc_fat::is_mounted(medium.name()),
        );
    }
}

fn environment_checks(logger: &mut ProbeLogger) {
    let key = "WII_STD_ENV_PROBE";
    let original = std::env::var_os(key);
    // Only this probe mutates these variables, before its worker starts.
    unsafe {
        std::env::set_var(key, "first");
    }
    let snapshot = std::env::vars_os();
    unsafe {
        std::env::set_var(key, "second");
    }
    logger.record(
        "environment update retains one key and snapshots own values",
        std::env::var_os(key).as_deref() == Some(std::ffi::OsStr::new("second"))
            && snapshot.filter(|(name, _)| name == key).collect::<Vec<_>>()
                == [(key.into(), "first".into())]
            && std::env::vars_os().filter(|(name, _)| name == key).count() == 1,
    );
    unsafe {
        std::env::remove_var(key);
    }
    logger.record(
        "environment removal is visible",
        std::env::var_os(key).is_none(),
    );
    if let Some(value) = original {
        unsafe {
            std::env::set_var(key, value);
        }
    }
    let paths = [Path::new("sd:/apps"), Path::new(""), Path::new("usb:/data")];
    logger.record(
        "path lists preserve mount prefixes and empty entries",
        std::env::join_paths(paths)
            .is_ok_and(|joined| std::env::split_paths(&joined).collect::<Vec<_>>() == paths),
    );
    logger.record(
        "path lists reject embedded separators",
        std::env::join_paths(["sd:/bad;name"]).is_err(),
    );
    match std::env::current_exe() {
        Ok(path) => logger.record("loader executable path is absolute", path.is_absolute()),
        Err(error) => logger.note(&format!("loader executable path unavailable: {error}")),
    }
}

fn path_service_checks(logger: &mut ProbeLogger, medium: Medium) -> io::Result<()> {
    let root_path = medium.path("wii-std-path-probe");
    let root = root_path.as_path();
    // This grants ownership of the complete new tree. Never reuse an old tree.
    fs::create_dir(root)?;
    let original_cwd = std::env::current_dir();
    let result = (|| {
        fs::create_dir(root.join("Nested"))?;
        std::env::set_current_dir(root)?;
        logger.record(
            "current directory uses the real mounted path",
            std::env::current_dir()? == root,
        );
        let mut source = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open("source.bin")?;
        source.write_all(b"renamed bytes")?;
        source.sync_all()?;
        let mut permissions = source.metadata()?.permissions();
        permissions.set_readonly(true);
        source.set_permissions(permissions.clone())?;
        logger.record(
            "handle permission changes persist in path metadata",
            fs::metadata("source.bin")?.permissions().readonly(),
        );
        permissions.set_readonly(false);
        source.set_permissions(permissions)?;
        drop(source);
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open("target.bin")?;
        target.write_all(b"old target must disappear")?;
        target.sync_all()?;
        drop(target);
        logger.record(
            "typed rename rejects cross-volume paths before a write",
            ogc_fat::rename_replace(
                &medium.cpath("wii-std-path-probe/source.bin"),
                &medium.other().cpath("wii-std-cross-volume-target"),
            ) == Err(ogc_fat::FatError::CrossDevice),
        );
        fs::rename("source.bin", "target.bin")?;
        logger.record(
            "rename replaces target with source bytes and removes old name",
            fs::read("target.bin")? == b"renamed bytes" && !fs::exists("source.bin")?,
        );
        fs::rename("target.bin", "target.bin")?;
        logger.record(
            "same-file rename preserves bytes",
            fs::read("target.bin")? == b"renamed bytes",
        );
        logger.record(
            "missing rename preserves destination",
            fs::rename("missing", "target.bin").is_err_and(|error| error.raw_os_error() == Some(2))
                && fs::read("target.bin")? == b"renamed bytes",
        );
        logger.record(
            "canonicalize reads stored case and resolves dot components",
            fs::canonicalize("nested/../TARGET.BIN")? == root.join("target.bin"),
        );
        logger.record(
            "canonicalize requires each component to exist",
            fs::canonicalize("missing/../target.bin")
                .is_err_and(|error| error.raw_os_error() == Some(2)),
        );
        let before = std::env::current_dir()?;
        logger.record(
            "failed chdir preserves current directory",
            std::env::set_current_dir("target.bin")
                .is_err_and(|error| error.raw_os_error() == Some(20))
                && std::env::current_dir()? == before,
        );
        std::env::set_current_dir("Nested")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open("child.bin")?;
        file.write_all(b"nested bytes")?;
        file.sync_all()?;
        drop(file);
        logger.record(
            "relative paths follow a directory change",
            fs::read("child.bin")? == b"nested bytes",
        );
        std::env::set_current_dir("..")?;
        let long = "Long filename for alias.bin";
        let mut alias_file = OpenOptions::new().write(true).create_new(true).open(long)?;
        alias_file.write_all(b"alias bytes")?;
        alias_file.sync_all()?;
        drop(alias_file);
        logger.record(
            "typed canonicalize resolves short aliases to stored names",
            ogc_fat::canonicalize(&medium.cpath("wii-std-path-probe/LONGFI~1.BIN"))
                .map_err(|error| io::Error::from_raw_os_error(ogc_fat::fat_errno(error)))?
                == medium
                    .cpath("wii-std-path-probe/Long filename for alias.bin")
                    .as_bytes(),
        );
        logger.record(
            "canonicalize resolves generated short aliases to stored long names",
            fs::canonicalize("LONGFI~1.BIN")? == root.join(long),
        );
        fs::create_dir("empty-target")?;
        fs::rename("Nested", "empty-target")?;
        logger.record(
            "directory rename replaces an empty target and preserves children",
            fs::read("empty-target/child.bin")? == b"nested bytes" && !fs::exists("Nested")?,
        );
        fs::create_dir("other-source")?;
        logger.record(
            "directory rename rejects nonempty target without removal",
            fs::rename("other-source", "empty-target")
                .is_err_and(|error| error.raw_os_error() == Some(90))
                && fs::read("empty-target/child.bin")? == b"nested bytes"
                && fs::metadata("other-source")?.is_dir(),
        );
        let mut permissions = fs::metadata("target.bin")?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions("target.bin", permissions.clone())?;
        logger.record(
            "path permission changes reject a writable open",
            OpenOptions::new()
                .write(true)
                .open("target.bin")
                .is_err_and(|error| error.raw_os_error() == Some(30)),
        );
        permissions.set_readonly(false);
        fs::set_permissions("target.bin", permissions)?;
        Ok(())
    })();
    // Leave the owned tree before removal. Restore an available original cwd.
    let fallback = medium.path("");
    let restored = std::env::set_current_dir(original_cwd.as_deref().unwrap_or(&fallback));
    if let Err(error) = &restored {
        logger.note(&format!("current directory restore failed: {error}"));
        logger.record("current directory restore succeeds", false);
    }
    let cleanup = fs::remove_dir_all(root);
    if let Err(error) = &cleanup {
        logger.note(&format!("owned path tree cleanup failed: {error}"));
        logger.record("owned path tree cleanup succeeds", false);
    }
    logger.record(
        "recursive removal removes files and nested directories",
        cleanup.is_ok() && fs::metadata(root).is_err_and(|error| error.raw_os_error() == Some(2)),
    );
    result.and(restored).and(cleanup)
}
