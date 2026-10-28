/// Keep the count check and waiter registration in one interrupt critical
/// section. The sleep backend restores interrupts while the thread is blocked.
/// Return true if the retrace already occurred, or false after a queue wake.
pub fn sleep_if_unchanged<G, E>(
    previous: u32,
    enter_critical: impl FnOnce() -> G,
    mut read_count: impl FnMut() -> u32,
    mut sleep: impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let _guard = enter_critical();
    if read_count() != previous {
        return Ok(true);
    }
    sleep()?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct Guard<'a>(&'a Cell<bool>);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }

    #[test]
    fn retrace_between_check_and_sleep_cannot_be_delivered_before_registration() {
        // Explicit interrupt/queue boundary fixture: an IRQ raised after the
        // count read must remain pending until the sleep registers its waiter.
        let masked = Cell::new(false);
        let pending = Cell::new(false);
        let count = Cell::new(7);
        let wakes = Cell::new(0);
        let result = sleep_if_unchanged(
            7,
            || {
                masked.set(true);
                Guard(&masked)
            },
            || {
                assert!(masked.get());
                pending.set(true);
                count.get()
            },
            || {
                assert!(masked.get());
                assert!(pending.replace(false));
                count.set(8);
                wakes.set(wakes.get() + 1);
                Ok::<(), i32>(())
            },
        );
        assert_eq!(result, Ok(false));
        assert_eq!(count.get(), 8);
        assert_eq!(wakes.get(), 1);
        assert!(!masked.get());
        assert_eq!(
            sleep_if_unchanged(7, || (), || count.get(), || panic!("already advanced")),
            Ok::<bool, i32>(true)
        );
    }

    #[test]
    fn sleep_failure_is_returned_and_interrupt_state_is_restored() {
        let masked = Cell::new(false);
        assert_eq!(
            sleep_if_unchanged(
                0,
                || {
                    masked.set(true);
                    Guard(&masked)
                },
                || 0,
                || Err(-1)
            ),
            Err(-1)
        );
        assert!(!masked.get());
    }
}
