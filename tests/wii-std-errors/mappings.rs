//! Tests the production mappings and devoptab engines with explicit failures.
#![allow(dead_code)]
extern crate self as ogc_lwp_driver;
extern crate self as ogc_sysbase;
#[path = "../../crates/core/ogc-sysbase/src/error.rs"]
mod error;
pub use error::*;
#[path = "../../crates/core/ogc-sysbase/src/device_io.rs"]
mod device_io;
#[path = "../../crates/core/ogc-lwp-driver/src/thread.rs"]
mod thread;
#[path = "../../crates/core/ogc-lwp-driver/src/tls.rs"]
mod tls;
pub use thread::ThreadError;
pub use tls::TlsError;
#[path = "../../crates/services/ogc-fat/src/fat_error.rs"]
mod fat_error;
#[path = "../../crates/services/ogc-std-runtime/src/errors.rs"]
mod lwp_error;
#[path = "../../crates/services/ogc-network-sys/src/socket_error.rs"]
mod socket_error;
use std::cell::Cell;

#[test]
fn fat_errors_have_independent_expected_newlib_codes() {
    use fat_error::FatError::*;
    let cases = [
        (AlreadyMounted, 17),
        (AlreadyExists, 17),
        (Busy, 16),
        (DeviceUnavailable, 19),
        (MediaRemoved, 19),
        (InvalidBootSector, 77),
        (UnsupportedVariant, 134),
        (PathError, 22),
        (Io, 5),
        (NotFound, 2),
        (NoSpace, 28),
        (ReadOnly, 30),
        (IsDirectory, 21),
        (NotDirectory, 20),
        (DirectoryNotEmpty, 90),
        (NotSupported, 134),
        (PermissionDenied, 13),
        (CrossDevice, 18),
        (BadDescriptor, 9),
        (FileTooLarge, 27),
    ];
    for (error, errno) in cases {
        assert_eq!(fat_error::fat_errno(error), errno);
    }
    assert_eq!(describe_errno(77).kind, ErrnoKind::InvalidData);
}

#[test]
fn lwp_failures_keep_cancellation_distinct_from_cleanup_io() {
    use ThreadError::*;
    for (error, errno) in [
        (ExceptionContext, 13),
        (InterruptsDisabled, 16),
        (InvalidThread, 22),
        (InvalidStack, 22),
        (InvalidTimeout, 22),
        (MissingEntry, 22),
        (ResourceExhausted, 12),
        (InvalidName, 22),
        (BufferTooSmall, 22),
        (UnknownStack, 134),
        (CleanupComplete, 16),
        (CleanupCallbackConflict, 16),
        (TlsCleanup, 5),
        (Cancelled, 140),
    ] {
        assert_eq!(lwp_error::thread_error(error), errno);
    }
    for (error, errno) in [
        (TlsError::InvalidKey, 22),
        (TlsError::KeyCapacity, 12),
        (TlsError::DestructorLimit, 5),
        (TlsError::ReentrantCleanup, 16),
        (TlsError::ExceptionContext, 1),
    ] {
        assert_eq!(lwp_error::tls_error(error), errno);
    }
    assert_eq!(describe_errno(140).message, "operation canceled");
}

#[test]
fn ipc_and_ios_codes_remain_distinct_and_keep_original_details() {
    use socket_error::NetError;
    for (code, errno) in [(-4, 22), (-5, 12), (-6, 2), (-8, 11), (-22, 12), (-999, 5)] {
        let error = NetError::reply(Err(code)).unwrap_err();
        assert_eq!(error.errno(), errno);
        assert_eq!(NetError::decode(error.encoded()), error);
        assert!(!error.retryable());
    }
    for (code, errno) in [
        (-6, 11),
        (-14, 111),
        (-15, 104),
        (-27, 4),
        (-49, 12),
        (-76, 116),
    ] {
        let error = NetError::reply(Ok(code)).unwrap_err();
        assert_eq!(error.errno(), errno);
        assert_eq!(NetError::decode(error.encoded()), error);
    }
}

#[test]
fn callback_errno_is_read_immediately_and_not_used_on_success() {
    let errno = Cell::new(0);
    assert_eq!(
        device_io::count(
            4,
            || {
                errno.set(28);
                -1
            },
            || errno.get()
        ),
        Err(-28)
    );
    assert_eq!(
        device_io::status(
            || {
                errno.set(30);
                -1
            },
            || errno.get()
        ),
        Err(-30)
    );
    assert_eq!(
        device_io::seek(
            || {
                errno.set(29);
                -1
            },
            || errno.get()
        ),
        Err(-29)
    );
    assert_eq!(
        device_io::count(4, || 2, || panic!("stale errno read")),
        Ok(2)
    );
    assert_eq!(
        device_io::seek(|| 4096, || panic!("stale errno read")),
        Ok(4096)
    );
    assert_eq!(
        device_io::status(|| 0, || panic!("stale errno read")),
        Ok(())
    );
    assert_eq!(device_io::status(|| -1, || 0), Err(-5));
    assert_eq!(device_io::status(|| 1, || 0), Err(-5));
    assert_eq!(device_io::count(4, || 5, || 0), Err(-5));
    assert_eq!(
        device_io::count(usize::MAX, || panic!("invalid extent"), || 0),
        Err(-22)
    );
}

#[test]
fn partial_writes_preserve_bytes_and_stop_at_the_real_failure() {
    let mut persisted = Vec::new();
    assert_eq!(
        device_io::write_all(b"abcdef", |bytes| {
            if persisted.len() == 4 {
                return Err(-28);
            }
            persisted.extend_from_slice(&bytes[..2]);
            Ok(2)
        }),
        Err(-28)
    );
    assert_eq!(persisted, b"abcd");
    assert_eq!(device_io::write_all(b"x", |_| Ok(0)), Err(-5));
    assert_eq!(device_io::write_all(b"x", |_| Ok(2)), Err(-5));
    assert_eq!(device_io::write_all(b"", |_| panic!("empty write")), Ok(()));
}

#[test]
fn close_releases_once_and_returns_and_records_the_flush_failure() {
    let events = std::cell::RefCell::new(Vec::new());
    let before = device_io::device_close_failures();
    let result = device_io::close(
        || {
            events.borrow_mut().push("close failed");
            Err(-19)
        },
        || events.borrow_mut().push("state released"),
    );
    assert_eq!(result, Err(-19));
    result.unwrap_or_else(device_io::record_close_failure);
    assert_eq!(*events.borrow(), ["close failed", "state released"]);
    assert_eq!(device_io::device_close_failures(), before + 1);
    assert_eq!(device_io::last_device_close_errno(), 19);
}

#[path = "../../crates/services/ogc-network-sys/src/ipc_cleanup.rs"]
mod ipc_cleanup;
#[test]
fn initialization_reports_close_failure_and_retains_secondary_errors() {
    let before = ipc_cleanup::initialization_close_failures();
    assert_eq!(ipc_cleanup::finish(0, Err(-6)), -6);
    assert_eq!(ipc_cleanup::finish(-100, Err(-22)), -100);
    ipc_cleanup::after_failure(Err(-4));
    assert_eq!(ipc_cleanup::initialization_close_failures(), before + 3);
    assert_eq!(ipc_cleanup::last_initialization_close_error(), -4);
    assert_eq!(ipc_cleanup::finish(0, Ok(())), 0);
    assert_eq!(ipc_cleanup::finish(-100, Ok(())), -100);
    ipc_cleanup::after_failure(Ok(()));
    assert_eq!(ipc_cleanup::initialization_close_failures(), before + 3);
}
