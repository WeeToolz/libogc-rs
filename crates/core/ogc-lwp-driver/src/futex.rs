//! Shared deadline and scheduler waiter selection rules.

/// Keep relative deadlines strictly within the scheduler's signed half range.
/// Reserve one tick so skipping the no-deadline sentinel cannot cross it.
pub const FUTEX_MAX_TIMEOUT_TICKS: u64 = (1u64 << 63) - 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FutexError {
    ExceptionContext,
    InterruptsDisabled,
    InvalidTimeout,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FutexWait {
    ValueChanged,
    Woken,
    TimedOut,
}

/// Round up to hardware ticks. u128 covers every Duration at the Wii frequency.
pub(crate) fn duration_ticks(duration: core::time::Duration, frequency: u64) -> u128 {
    (duration.as_nanos() * u128::from(frequency)).div_ceil(1_000_000_000)
}

/// A common duration budget for sleep and atomic waits. Hardware deadlines use
/// bounded chunks; only real elapsed ticks reduce the remaining duration.
pub(crate) struct TickBudget(u128);

impl TickBudget {
    pub(crate) fn new(duration: core::time::Duration, frequency: u64) -> Self {
        Self(duration_ticks(duration, frequency))
    }

    pub(crate) fn chunk(&self) -> u64 {
        self.0.min(u128::from(FUTEX_MAX_TIMEOUT_TICKS)) as u64
    }

    pub(crate) fn elapse(&mut self, ticks: u64) -> bool {
        self.0 = self.0.saturating_sub(u128::from(ticks));
        self.0 == 0
    }
}

pub(crate) fn deadline(now: u64, ticks: u64) -> Result<u64, FutexError> {
    if ticks > FUTEX_MAX_TIMEOUT_TICKS {
        return Err(FutexError::InvalidTimeout);
    }
    let deadline = now.wrapping_add(ticks);
    // u64::MAX means no deadline in the scheduler. Wait one extra tick when
    // the real deadline would have that value; never expire early.
    Ok(if deadline == u64::MAX { 0 } else { deadline })
}

#[derive(Clone, Copy)]
pub(crate) struct Waiter {
    pub index: usize,
    pub priority: u8,
    pub order: u64,
}

/// Highest effective priority first; FIFO within one priority.
pub(crate) fn select(best: Option<Waiter>, candidate: Waiter) -> Option<Waiter> {
    Some(match best {
        Some(best)
            if best.priority > candidate.priority
                || (best.priority == candidate.priority && best.order <= candidate.order) =>
        {
            best
        }
        _ => candidate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_budget_keeps_long_waits_finite_and_counts_actual_elapsed_time() {
        let mut budget = TickBudget::new(core::time::Duration::from_millis(25), 60_750_000);
        assert_eq!(budget.chunk(), 1_518_750);
        assert!(!budget.elapse(1_000_000));
        assert_eq!(budget.chunk(), 518_750);
        assert!(budget.elapse(600_000));
        assert_eq!(budget.chunk(), 0);
        let mut long = TickBudget::new(core::time::Duration::MAX, 60_750_000);
        assert_eq!(long.chunk(), 0x7fff_ffff_ffff_fffe);
        assert!(!long.elapse(0x7fff_ffff_ffff_fffe));
        assert_eq!(long.chunk(), 0x7fff_ffff_ffff_fffe);
    }

    #[test]
    fn deadlines_wrap_and_do_not_use_the_infinite_sentinel() {
        assert_eq!(deadline(100, 25), Ok(125));
        assert_eq!(deadline(u64::MAX - 5, 10), Ok(4));
        assert_eq!(deadline(u64::MAX - 5, 5), Ok(0));
        assert_eq!(
            deadline(0, FUTEX_MAX_TIMEOUT_TICKS),
            Ok(0x7fff_ffff_ffff_fffe)
        );
        assert_eq!(
            deadline(0, FUTEX_MAX_TIMEOUT_TICKS + 1),
            Err(FutexError::InvalidTimeout)
        );
        assert_eq!(deadline(0, u64::MAX), Err(FutexError::InvalidTimeout));
    }

    #[test]
    fn wake_selection_respects_priority_then_fifo() {
        let mut best = None;
        for (index, priority, order) in [(0, 64, 1), (1, 90, 9), (2, 90, 4), (3, 60, 0)] {
            best = select(
                best,
                Waiter {
                    index,
                    priority,
                    order,
                },
            );
        }
        assert_eq!(best.unwrap().index, 2);
    }

    #[test]
    fn timeout_conversion_rounds_up_without_duration_overflow() {
        use core::time::Duration;
        assert_eq!(duration_ticks(Duration::ZERO, 60_750_000), 0);
        assert_eq!(duration_ticks(Duration::from_nanos(1), 60_750_000), 1);
        assert_eq!(
            duration_ticks(Duration::from_millis(25), 60_750_000),
            1_518_750
        );
        assert_eq!(duration_ticks(Duration::new(1, 1), 60_750_000), 60_750_001);
        assert_eq!(
            duration_ticks(Duration::MAX, 60_750_000),
            1_120_639_702_477_855_260_672_000_000
        );
    }
}
