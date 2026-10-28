//! Atomic status handoff for the C deinitialization entry point.

use core::sync::atomic::{AtomicI32, Ordering};

pub fn record_status(status: &AtomicI32, result: i32) {
    status.store(result, Ordering::Release);
}

pub fn latest_status(status: &AtomicI32) -> i32 {
    status.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deinitialization_result_stays_visible_until_next_attempt() {
        let status = AtomicI32::new(0);
        record_status(&status, -41);
        assert_eq!(latest_status(&status), -41);

        record_status(&status, 0);
        assert_eq!(latest_status(&status), 0);
    }
}
