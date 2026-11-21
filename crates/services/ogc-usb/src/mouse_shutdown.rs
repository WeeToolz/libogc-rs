//! Stop the worker before its owner releases any resources.

pub(crate) fn stop_worker(
    wake: impl FnOnce() -> i32,
    join: impl FnOnce() -> i32,
) -> Result<(), i32> {
    let wake_status = wake();
    if wake_status != 0 {
        return Err(wake_status);
    }
    let join_status = join();
    if join_status != 0 {
        return Err(join_status);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::stop_worker;
    use core::cell::Cell;

    #[test]
    fn failed_wake_leaves_join_for_a_retry() {
        assert_eq!(stop_worker(|| -5, || panic!("join could block")), Err(-5));
    }

    #[test]
    fn join_error_is_returned_after_wake() {
        let woke = Cell::new(false);
        let status = stop_worker(
            || {
                woke.set(true);
                0
            },
            || {
                assert!(woke.get());
                -6
            },
        );
        assert_eq!(status, Err(-6));
    }

    #[test]
    fn successful_wake_and_join_allow_cleanup() {
        assert_eq!(stop_worker(|| 0, || 0), Ok(()));
    }
}
