//! Compile the actual std adapters with the host byte-string representation.
pub use std::{error, ffi, fmt, fs, io, path, slice, sync, vec};
#[path = "../../rust-std/overlay/std/src/sys/env/wii.rs"]
pub mod environment;
#[path = "../../crates/services/ogc-std-runtime/src/fs_abi.rs"]
pub mod fs_abi;
#[path = "../../rust-std/overlay/std/src/sys/paths/wii.rs"]
#[allow(dead_code)]
mod paths;
mod sys {
    pub use crate::environment as env;
    pub mod args {
        pub fn args() -> std::vec::IntoIter<std::ffi::OsString> {
            Vec::new().into_iter()
        }
    }
    pub mod wii {
        pub use crate::fs_abi;
    }
}
use ffi::{OsStr, OsString};
#[test]
fn environment_updates_values_and_snapshots_without_duplicate_keys() {
    let key = OsStr::new("STD_TEST_SNAPSHOT");
    unsafe {
        environment::setenv(key, OsStr::new("old")).unwrap();
    }
    let snapshot = environment::env();
    unsafe {
        environment::setenv(key, OsStr::new("new")).unwrap();
    }
    assert_eq!(environment::getenv(key), Some(OsString::from("new")));
    assert_eq!(environment::env().filter(|(k, _)| k == key).count(), 1);
    assert_eq!(
        snapshot.filter(|(k, _)| k == key).collect::<Vec<_>>(),
        [(key.to_owned(), OsString::from("old"))]
    );
    unsafe {
        environment::unsetenv(key).unwrap();
    }
    assert_eq!(environment::getenv(key), None);
}
#[test]
fn invalid_environment_inputs_fail_without_mutation() {
    for key in ["", "bad=name", "bad\0name"] {
        unsafe {
            assert_eq!(
                environment::setenv(OsStr::new(key), OsStr::new("value"))
                    .unwrap_err()
                    .raw_os_error(),
                Some(22)
            );
            assert_eq!(
                environment::unsetenv(OsStr::new(key))
                    .unwrap_err()
                    .raw_os_error(),
                Some(22)
            );
        }
        assert_eq!(environment::getenv(OsStr::new(key)), None);
    }
    unsafe {
        environment::setenv(OsStr::new("STD_TEST_INVALID"), OsStr::new("valid")).unwrap();
        assert!(
            environment::setenv(OsStr::new("STD_TEST_INVALID"), OsStr::new("bad\0value")).is_err()
        );
    }
    assert_eq!(
        environment::getenv(OsStr::new("STD_TEST_INVALID")),
        Some(OsString::from("valid"))
    );
}
#[test]
fn environment_supports_non_utf8_empty_values_and_thread_visibility() {
    let key = OsStr::new("STD_TEST_BYTES");
    let bytes = unsafe { OsStr::from_encoded_bytes_unchecked(b"\xff=value") };
    unsafe {
        environment::setenv(key, bytes).unwrap();
    }
    std::thread::spawn(|| {
        assert_eq!(
            environment::getenv(OsStr::new("STD_TEST_BYTES"))
                .unwrap()
                .as_encoded_bytes(),
            b"\xff=value"
        )
    })
    .join()
    .unwrap();
    unsafe {
        environment::setenv(key, OsStr::new("")).unwrap();
    }
    assert_eq!(environment::getenv(key), Some(OsString::new()));
}
#[test]
fn path_lists_preserve_mount_colons_empty_parts_and_raw_bytes() {
    let raw = unsafe { OsStr::from_encoded_bytes_unchecked(b"sd:/apps;;usb:/\xff;") };
    let items = paths::split_paths(raw).collect::<Vec<_>>();
    assert_eq!(
        items
            .iter()
            .map(|p| p.as_os_str().as_encoded_bytes())
            .collect::<Vec<_>>(),
        [b"sd:/apps".as_slice(), b"", b"usb:/\xff", b""]
    );
    assert_eq!(paths::join_paths(items.iter()).unwrap(), raw);
    assert_eq!(
        paths::split_paths_ref(raw).unwrap().collect::<Vec<_>>(),
        items.iter().map(|p| p.as_path()).collect::<Vec<_>>()
    );
    assert!(paths::join_paths(["sd:/bad;name"].into_iter()).is_err());
}
#[test]
fn no_loader_path_has_an_explicit_error() {
    assert_eq!(paths::current_exe().unwrap_err().raw_os_error(), Some(2));
}
