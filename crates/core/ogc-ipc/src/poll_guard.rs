//! Serialize manual mailbox draining with the interrupt handler.

pub(crate) fn protected_poll<S, R: FnOnce(S), T>(
    enter: impl FnOnce() -> S,
    restore: R,
    poll: impl FnOnce() -> T,
) -> T {
    struct Guard<S, R: FnOnce(S)> {
        state: Option<S>,
        restore: Option<R>,
    }
    impl<S, R: FnOnce(S)> Drop for Guard<S, R> {
        fn drop(&mut self) {
            if let (Some(state), Some(restore)) = (self.state.take(), self.restore.take()) {
                restore(state);
            }
        }
    }
    let _guard = Guard {
        state: Some(enter()),
        restore: Some(restore),
    };
    poll()
}

#[cfg(all(test, not(target_arch = "powerpc")))]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn drain_masks_interrupts_and_restores_before_caller_continues() {
        let enabled = Cell::new(true);
        let drained = protected_poll(
            || enabled.replace(false),
            |saved| enabled.set(saved),
            || {
                // An interrupt cannot consume this same mailbox reply here.
                assert!(!enabled.get());
                true
            },
        );
        assert!(drained);
        // The caller may now yield to the completion worker.
        assert!(enabled.get());
    }

    #[test]
    fn nested_mask_does_not_enable_interrupts() {
        let enabled = Cell::new(false);
        protected_poll(|| enabled.replace(false), |saved| enabled.set(saved), || ());
        assert!(!enabled.get());
    }

    #[test]
    fn unwind_restores_the_saved_state() {
        let enabled = Cell::new(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            protected_poll(
                || enabled.replace(false),
                |saved| enabled.set(saved),
                || panic!("injected drain failure"),
            );
        }));
        assert!(result.is_err());
        assert!(enabled.get());
    }
}
