//! State transitions for asynchronous USB bulk operations.

use core::sync::atomic::{AtomicI32, Ordering};

pub const PROCESSING: i32 = -10010;
const ALARM_ARMED: i32 = i32::MAX;
const NO_ALARM: i32 = i32::MAX - 1;
const TIMED_OUT_PENDING: i32 = i32::MAX - 2;
const TIMED_OUT_DRAINED: i32 = i32::MAX - 3;
const ALARM_CANCEL_PENDING: i32 = i32::MAX - 4;
const ALARM_CANCEL_DRAINED: i32 = i32::MAX - 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Waiting,
    Complete(i32),
    TimedOut { request_pending: bool },
    AlarmCancelFailed { alarm_pending: bool },
}

pub fn begin(result: &AtomicI32) -> Result<(), ()> {
    loop {
        let previous = result.load(Ordering::Acquire);
        if matches!(previous, TIMED_OUT_PENDING | ALARM_CANCEL_PENDING) {
            return Err(());
        }
        if result
            .compare_exchange(previous, PROCESSING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Ok(());
        }
    }
}

pub fn arm_alarm(result: &AtomicI32, cancel_alarm: impl FnOnce() -> i32) -> State {
    if result
        .compare_exchange(PROCESSING, ALARM_ARMED, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        return State::Waiting;
    }

    let observed = result.load(Ordering::Acquire);
    match classify(observed) {
        State::Complete(value) => {
            if result
                .compare_exchange(
                    value,
                    ALARM_CANCEL_PENDING,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_err()
            {
                return classify(result.load(Ordering::Acquire));
            }
            if cancel_alarm() < 0 {
                State::AlarmCancelFailed {
                    alarm_pending: true,
                }
            } else {
                if result
                    .compare_exchange(
                        ALARM_CANCEL_PENDING,
                        value,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    State::Complete(value)
                } else {
                    classify(result.load(Ordering::Acquire))
                }
            }
        }
        State::TimedOut { .. } => {
            // The timeout callback already ran. It owns the transition that
            // records whether the request callback is still pending.
            classify(result.load(Ordering::Acquire))
        }
        state => state,
    }
}

pub fn mark_no_alarm(result: &AtomicI32) -> bool {
    result
        .compare_exchange(PROCESSING, NO_ALARM, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

pub fn complete(result: &AtomicI32, value: i32, mut cancel_alarm: impl FnMut() -> i32) {
    loop {
        let current = result.load(Ordering::Acquire);
        match current {
            PROCESSING | NO_ALARM => {
                if result
                    .compare_exchange(current, value, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return;
                }
            }
            ALARM_ARMED => {
                if cancel_alarm() < 0 {
                    if result
                        .compare_exchange(
                            ALARM_ARMED,
                            ALARM_CANCEL_PENDING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return;
                    }
                } else {
                    if result
                        .compare_exchange(ALARM_ARMED, value, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return;
                    }
                }
            }
            TIMED_OUT_PENDING => {
                if result
                    .compare_exchange(
                        TIMED_OUT_PENDING,
                        TIMED_OUT_DRAINED,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    return;
                }
            }
            _ => return,
        }
    }
}

pub fn timeout(result: &AtomicI32) {
    loop {
        let current = result.load(Ordering::Acquire);
        let next = match current {
            PROCESSING | ALARM_ARMED => TIMED_OUT_PENDING,
            ALARM_CANCEL_PENDING => ALARM_CANCEL_DRAINED,
            _ => return,
        };
        if result
            .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return;
        }
    }
}

pub fn classify(value: i32) -> State {
    match value {
        PROCESSING | ALARM_ARMED | NO_ALARM => State::Waiting,
        TIMED_OUT_PENDING => State::TimedOut {
            request_pending: true,
        },
        TIMED_OUT_DRAINED => State::TimedOut {
            request_pending: false,
        },
        ALARM_CANCEL_PENDING => State::AlarmCancelFailed {
            alarm_pending: true,
        },
        ALARM_CANCEL_DRAINED => State::AlarmCancelFailed {
            alarm_pending: false,
        },
        result => State::Complete(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_keeps_handle_busy_until_late_request_callback_finishes() {
        let result = AtomicI32::new(PROCESSING);
        timeout(&result);
        assert_eq!(
            classify(result.load(Ordering::Acquire)),
            State::TimedOut {
                request_pending: true
            }
        );
        assert!(begin(&result).is_err());

        complete(&result, 512, || {
            panic!("a fired alarm is not canceled again")
        });
        assert_eq!(
            classify(result.load(Ordering::Acquire)),
            State::TimedOut {
                request_pending: false
            }
        );
        assert!(begin(&result).is_ok());
    }

    #[test]
    fn completion_before_alarm_arm_is_preserved_and_alarm_is_canceled() {
        let result = AtomicI32::new(PROCESSING);
        complete(&result, 31, || panic!("alarm is not marked armed yet"));
        let mut canceled = false;

        assert_eq!(
            arm_alarm(&result, || {
                canceled = true;
                0
            }),
            State::Complete(31)
        );
        assert!(canceled);
    }

    #[test]
    fn armed_completion_cancels_alarm_before_publishing_result() {
        let result = AtomicI32::new(PROCESSING);
        assert_eq!(arm_alarm(&result, || 0), State::Waiting);
        let mut canceled = false;

        complete(&result, 31, || {
            canceled = true;
            0
        });

        assert!(canceled);
        assert_eq!(
            classify(result.load(Ordering::Acquire)),
            State::Complete(31)
        );
    }

    #[test]
    fn failed_alarm_cancel_blocks_reuse_until_alarm_callback_finishes() {
        let result = AtomicI32::new(PROCESSING);
        assert_eq!(arm_alarm(&result, || 0), State::Waiting);
        complete(&result, 31, || -77);
        assert_eq!(
            classify(result.load(Ordering::Acquire)),
            State::AlarmCancelFailed {
                alarm_pending: true
            }
        );
        assert!(begin(&result).is_err());

        timeout(&result);
        assert_eq!(
            classify(result.load(Ordering::Acquire)),
            State::AlarmCancelFailed {
                alarm_pending: false
            }
        );
        assert!(begin(&result).is_ok());
    }

    #[test]
    fn failed_alarm_setup_waits_for_request_callback_without_arming_timer() {
        let result = AtomicI32::new(PROCESSING);
        assert!(mark_no_alarm(&result));
        let mut canceled = false;
        complete(&result, 512, || {
            canceled = true;
            -77
        });

        assert!(!canceled);
        assert_eq!(
            classify(result.load(Ordering::Acquire)),
            State::Complete(512)
        );
    }
}
