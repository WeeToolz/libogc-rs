//! USB transport state changes shared by the driver and host failure tests.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotResetStep {
    ClassReset,
    DelayMs(u64),
    ClearInHalt,
    ClearOutHalt,
}

/// Complete reset recovery and return the first failed operation.
pub fn reset(mut run: impl FnMut(BotResetStep) -> i32) -> i32 {
    let mut result = run(BotResetStep::ClassReset);
    for step in [
        BotResetStep::DelayMs(60),
        BotResetStep::ClearInHalt,
        BotResetStep::DelayMs(10),
        BotResetStep::ClearOutHalt,
        BotResetStep::DelayMs(10),
    ] {
        let next_result = run(step);
        if result >= 0 && next_result < 0 {
            result = next_result;
        }
    }
    result
}

/// Commit the suspended state only after the USB request succeeds.
pub fn suspend(suspended: &mut u8, fd: i32, suspend_device: impl FnOnce(i32) -> i32) -> i32 {
    if *suspended == 1 {
        return 0;
    }
    let result = suspend_device(fd);
    if result < 0 {
        return result;
    }
    *suspended = 1;
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    const STEPS: [BotResetStep; 6] = [
        BotResetStep::ClassReset,
        BotResetStep::DelayMs(60),
        BotResetStep::ClearInHalt,
        BotResetStep::DelayMs(10),
        BotResetStep::ClearOutHalt,
        BotResetStep::DelayMs(10),
    ];

    #[test]
    fn reset_reports_each_failure_and_completes_recovery_in_order() {
        for failure in 0..STEPS.len() {
            let mut calls = Vec::new();
            let result = reset(|step| {
                calls.push(step);
                if calls.len() - 1 == failure { -55 } else { 0 }
            });
            assert_eq!(result, -55);
            assert_eq!(calls, STEPS);
        }
    }

    #[test]
    fn reset_preserves_the_first_error() {
        let mut calls = 0;
        assert_eq!(
            reset(|_| {
                calls += 1;
                -40 - calls
            }),
            -41
        );
        assert_eq!(calls, 6);
    }

    #[test]
    fn reset_success_requires_all_recovery_steps() {
        let mut calls = Vec::new();
        assert_eq!(
            reset(|step| {
                calls.push(step);
                0
            }),
            0
        );
        assert_eq!(calls, STEPS);
    }

    #[test]
    fn suspend_failure_preserves_state_and_allows_retry() {
        let mut suspended = 0;
        assert_eq!(
            suspend(&mut suspended, 23, |fd| {
                assert_eq!(fd, 23);
                -55
            }),
            -55
        );
        assert_eq!(suspended, 0);
        assert_eq!(
            suspend(&mut suspended, 23, |fd| {
                assert_eq!(fd, 23);
                0
            }),
            0
        );
        assert_eq!(suspended, 1);
        assert_eq!(
            suspend(&mut suspended, 23, |_| panic!("already suspended")),
            0
        );
    }
}
