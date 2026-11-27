//! Shared descriptor for the IOS filesystem service.

use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::{FileDescriptor, IPC_OK, IPC_OPEN_NONE, open_file};

/// Invalid argument or uninitialized filesystem service.
pub const ISFS_EINVAL: i32 = -101;

static ISFS_INITIALIZED: AtomicBool = AtomicBool::new(false);
static FS_SERVICE_FD: AtomicI32 = AtomicI32::new(-1);

/// Return whether the filesystem service has been initialized.
pub fn is_initialized() -> bool {
    ISFS_INITIALIZED.load(Ordering::Acquire)
}

/// Return the descriptor for filesystem service ioctl requests.
pub fn descriptor() -> Result<FileDescriptor, i32> {
    let fd = FS_SERVICE_FD.load(Ordering::Acquire);
    if !is_initialized() || fd < 0 {
        return Err(ISFS_EINVAL);
    }
    Ok(FileDescriptor::new(fd))
}

/// Open the shared IOS filesystem service descriptor.
pub fn initialize() -> i32 {
    if is_initialized() && FS_SERVICE_FD.load(Ordering::Acquire) >= 0 {
        return IPC_OK;
    }

    match open_file(c"/dev/fs", IPC_OPEN_NONE) {
        Ok(fd) => {
            FS_SERVICE_FD.store(fd.raw(), Ordering::Release);
            ISFS_INITIALIZED.store(true, Ordering::Release);
            IPC_OK
        }
        Err(rc) => {
            FS_SERVICE_FD.store(-1, Ordering::Release);
            rc
        }
    }
}

/// Close the shared IOS filesystem service descriptor.
pub fn deinitialize() -> i32 {
    if ISFS_INITIALIZED.swap(false, Ordering::AcqRel) {
        let fd = FS_SERVICE_FD.swap(-1, Ordering::AcqRel);
        if fd >= 0 {
            FileDescriptor::new(fd)
                .close()
                .map_or_else(|rc| rc, |_| IPC_OK)
        } else {
            IPC_OK
        }
    } else {
        ISFS_EINVAL
    }
}
