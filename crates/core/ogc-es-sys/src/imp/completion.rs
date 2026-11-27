// Preserve the IOS result across the post-request side effects of the C ES API.
pub(super) fn finish_set_uid(
    result: i32,
    deinitialize: impl FnOnce() -> i32,
    initialize: impl FnOnce() -> i32,
) -> i32 {
    if result >= 0 && deinitialize() != 0 {
        let _ = initialize();
    }
    result
}

pub(super) fn finish_verify(result: i32, release: impl FnOnce(), reopen: impl FnOnce()) -> i32 {
    release();
    if result >= 0 {
        reopen();
    }
    result
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use self::alloc::vec::Vec;
    use super::*;
    use core::cell::RefCell;

    #[test]
    fn set_uid_only_refreshes_isfs_on_the_c_conditions() {
        for (result, deinit_result, expected) in [
            (-1017, 0, &[][..]),
            (0, 0, &["deinitialize"][..]),
            (7, -1, &["deinitialize", "initialize"][..]),
        ] {
            let events = RefCell::new(Vec::new());
            let rc = finish_set_uid(
                result,
                || {
                    events.borrow_mut().push("deinitialize");
                    deinit_result
                },
                || {
                    events.borrow_mut().push("initialize");
                    -9
                },
            );
            assert_eq!(rc, result);
            assert_eq!(events.borrow().as_slice(), expected);
        }
    }

    #[test]
    fn verify_releases_before_reopen_and_preserves_result() {
        for (result, expected) in [
            (-1017, &["release"][..]),
            (0, &["release", "reopen"][..]),
            (9, &["release", "reopen"][..]),
        ] {
            let events = RefCell::new(Vec::new());
            let rc = finish_verify(
                result,
                || events.borrow_mut().push("release"),
                || {
                    events.borrow_mut().push("reopen");
                },
            );
            assert_eq!(rc, result);
            assert_eq!(events.borrow().as_slice(), expected);
        }
    }
}
