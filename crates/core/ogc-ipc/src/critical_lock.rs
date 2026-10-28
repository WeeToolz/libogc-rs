//! Keep a short single-core critical section from being preempted by its user.

pub(crate) fn acquire<S>(
    mut mask: impl FnMut() -> S,
    mut restore: impl FnMut(S),
    mut try_lock: impl FnMut() -> bool,
    mut yield_thread: impl FnMut(),
) -> S {
    loop {
        let state = mask();
        if try_lock() {
            return state;
        }
        restore(state);
        yield_thread();
    }
}

pub(crate) fn release<S>(state: S, unlock: impl FnOnce(), restore: impl FnOnce(S)) {
    unlock();
    restore(state);
}

#[cfg(test)]
mod tests {
    extern crate std;
    use self::std::cell::{Cell, RefCell};
    use self::std::vec::Vec;
    use super::*;

    #[test]
    fn ownership_is_never_exposed_with_preemption_enabled() {
        let masked = Cell::new(false);
        let attempts = Cell::new(0);
        let trace = RefCell::new(Vec::new());
        let state = acquire(
            || {
                let old = masked.replace(true);
                trace.borrow_mut().push("mask");
                old
            },
            |old| {
                masked.set(old);
                trace.borrow_mut().push("restore");
            },
            || {
                assert!(masked.get(), "an interrupt could preempt the owner");
                attempts.set(attempts.get() + 1);
                trace.borrow_mut().push("try");
                attempts.get() == 2
            },
            || {
                assert!(!masked.get(), "never schedule with interrupts masked");
                trace.borrow_mut().push("yield");
            },
        );
        assert!(masked.get());
        release(
            state,
            || {
                assert!(masked.get());
                trace.borrow_mut().push("unlock");
            },
            |old| {
                masked.set(old);
                trace.borrow_mut().push("restore");
            },
        );
        assert!(!masked.get());
        assert_eq!(
            *trace.borrow(),
            [
                "mask", "try", "restore", "yield", "mask", "try", "unlock", "restore"
            ]
        );
    }

    #[test]
    fn nested_interrupt_mask_is_preserved() {
        let state = acquire(
            || 0u32,
            |_| panic!("uncontended"),
            || true,
            || panic!("uncontended"),
        );
        release(state, || {}, |state| assert_eq!(state, 0));
    }
}
