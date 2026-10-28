//! Ownership of join and detach for production LWP threads.

use core::sync::atomic::{AtomicBool, Ordering};

/// Publish all worker writes, including TLS cleanup, before a join can finish.
pub(crate) struct ThreadCompletion(AtomicBool);

impl ThreadCompletion {
    pub(crate) const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub(crate) fn publish(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ThreadLifecycle {
    detached: bool,
    join_claimed: bool,
    std_owned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleError {
    Detached,
    JoinClaimed,
    WrongOwner,
}

impl ThreadLifecycle {
    pub(crate) const fn new() -> Self {
        Self::with_owner(false)
    }

    pub(crate) const fn with_owner(std_owned: bool) -> Self {
        Self {
            detached: false,
            join_claimed: false,
            std_owned,
        }
    }

    pub(crate) fn begin_join(&mut self, std_owned: bool) -> Result<(), LifecycleError> {
        if self.std_owned != std_owned {
            return Err(LifecycleError::WrongOwner);
        }
        if self.detached {
            return Err(LifecycleError::Detached);
        }
        if self.join_claimed {
            return Err(LifecycleError::JoinClaimed);
        }
        self.join_claimed = true;
        Ok(())
    }

    pub(crate) fn detach(&mut self, std_owned: bool) -> Result<(), LifecycleError> {
        if self.std_owned != std_owned {
            return Err(LifecycleError::WrongOwner);
        }
        if self.detached {
            return Err(LifecycleError::Detached);
        }
        if self.join_claimed {
            return Err(LifecycleError::JoinClaimed);
        }
        self.detached = true;
        Ok(())
    }

    pub(crate) fn is_detached(self) -> bool {
        self.detached
    }

    pub(crate) fn cancel_join(&mut self) {
        self.join_claimed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_handles_cannot_consume_std_thread_ownership() {
        let mut state = ThreadLifecycle::with_owner(true);
        assert_eq!(state.begin_join(false), Err(LifecycleError::WrongOwner));
        assert_eq!(state.detach(false), Err(LifecycleError::WrongOwner));
        assert!(!state.is_detached());
        assert_eq!(state.begin_join(true), Ok(()));
        state.cancel_join();
        assert_eq!(state.detach(true), Ok(()));
        let mut compatibility = ThreadLifecycle::new();
        assert_eq!(
            compatibility.begin_join(true),
            Err(LifecycleError::WrongOwner)
        );
        assert_eq!(compatibility.detach(true), Err(LifecycleError::WrongOwner));
        assert_eq!(compatibility.begin_join(false), Ok(()));
    }

    #[test]
    fn completion_starts_pending() {
        let completion = ThreadCompletion::new();
        assert!(!completion.is_complete());
        completion.publish();
        assert!(completion.is_complete());
    }

    #[test]
    fn completion_publishes_worker_writes_before_join() {
        use core::cell::UnsafeCell;
        use std::sync::Arc;
        struct Shared {
            data: UnsafeCell<[u32; 2]>,
            completion: ThreadCompletion,
        }
        // SAFETY: The worker writes once. The parent reads only after the
        // production completion engine's release/acquire publication.
        unsafe impl Sync for Shared {}
        let shared = Arc::new(Shared {
            data: UnsafeCell::new([0; 2]),
            completion: ThreadCompletion::new(),
        });
        let worker_data = shared.clone();
        let worker = std::thread::spawn(move || {
            unsafe { *worker_data.data.get() = [0x1357_2468, 0x9abc_def0] };
            worker_data.completion.publish();
        });
        while !shared.completion.is_complete() {
            std::thread::yield_now();
        }
        // Read before host join, so that host join is not the publication path.
        assert_eq!(unsafe { *shared.data.get() }, [0x1357_2468, 0x9abc_def0]);
        worker.join().unwrap();
    }

    #[test]
    fn join_claim_excludes_a_second_joiner_and_detach() {
        let mut state = ThreadLifecycle::new();
        assert_eq!(state.begin_join(false), Ok(()));
        assert_eq!(state.begin_join(false), Err(LifecycleError::JoinClaimed));
        assert_eq!(state.detach(false), Err(LifecycleError::JoinClaimed));
        assert!(!state.is_detached());
        state.cancel_join();
        assert_eq!(state.detach(false), Ok(()));
    }

    #[test]
    fn detached_thread_cannot_be_joined_or_detached_twice() {
        let mut state = ThreadLifecycle::new();
        assert_eq!(state.detach(false), Ok(()));
        assert!(state.is_detached());
        assert_eq!(state.begin_join(false), Err(LifecycleError::Detached));
        assert_eq!(state.detach(false), Err(LifecycleError::Detached));
    }
}
