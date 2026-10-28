//! State transitions shared by production runtime entry and its host tests.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePhase {
    Cold,
    Prepared,
    Running,
    Cleaning,
    Cleaned,
    Returned,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryError {
    InvalidState,
    WrongThread,
    InvalidArguments,
    CleanupFailed(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub phase: RuntimePhase,
    pub initial_thread: u32,
    pub argc: i32,
    pub argv: usize,
    pub cleanup_result: Option<i32>,
    pub return_status: Option<i32>,
}

pub(crate) struct EntryState(RuntimeSnapshot);

impl EntryState {
    pub(crate) const fn new() -> Self {
        Self(RuntimeSnapshot {
            phase: RuntimePhase::Cold,
            initial_thread: 0,
            argc: 0,
            argv: 0,
            cleanup_result: None,
            return_status: None,
        })
    }

    pub(crate) fn snapshot(&self) -> RuntimeSnapshot {
        self.0
    }

    pub(crate) fn prepare(
        &mut self,
        thread: u32,
        argc: i32,
        argv: usize,
    ) -> Result<(), EntryError> {
        if self.0.phase != RuntimePhase::Cold {
            return Err(EntryError::InvalidState);
        }
        if thread == 0
            || thread == u32::MAX
            || argc < 0
            || (argc != 0 && argv == 0)
            || !argv.is_multiple_of(core::mem::align_of::<usize>())
        {
            return Err(EntryError::InvalidArguments);
        }
        self.0.initial_thread = thread;
        self.0.argc = argc;
        self.0.argv = argv;
        self.0.phase = RuntimePhase::Prepared;
        Ok(())
    }

    fn check_thread(&self, thread: u32) -> Result<(), EntryError> {
        if thread != self.0.initial_thread {
            Err(EntryError::WrongThread)
        } else {
            Ok(())
        }
    }

    pub(crate) fn enter(
        &mut self,
        thread: u32,
        argc: isize,
        argv: usize,
    ) -> Result<(), EntryError> {
        self.check_thread(thread)?;
        if self.0.phase != RuntimePhase::Prepared {
            return Err(EntryError::InvalidState);
        }
        if argc != self.0.argc as isize || argv != self.0.argv {
            return Err(EntryError::InvalidArguments);
        }
        self.0.phase = RuntimePhase::Running;
        Ok(())
    }

    /// False means cleanup already completed successfully. Work in progress or
    /// failed work never takes this idempotent success path.
    pub(crate) fn begin_cleanup(&mut self, thread: u32) -> Result<bool, EntryError> {
        self.check_thread(thread)?;
        match self.0.phase {
            RuntimePhase::Prepared | RuntimePhase::Running => {
                self.0.phase = RuntimePhase::Cleaning;
                Ok(true)
            }
            RuntimePhase::Cleaned => Ok(false),
            RuntimePhase::Failed => match self.0.cleanup_result {
                Some(result) => Err(EntryError::CleanupFailed(result)),
                None => Err(EntryError::InvalidState),
            },
            _ => Err(EntryError::InvalidState),
        }
    }

    pub(crate) fn complete_cleanup(&mut self, result: i32) -> Result<(), EntryError> {
        if self.0.phase != RuntimePhase::Cleaning {
            return Err(EntryError::InvalidState);
        }
        self.0.cleanup_result = Some(result);
        self.0.phase = if result == 0 {
            RuntimePhase::Cleaned
        } else {
            RuntimePhase::Failed
        };
        if result == 0 {
            Ok(())
        } else {
            Err(EntryError::CleanupFailed(result))
        }
    }

    pub(crate) fn returned(&mut self, thread: u32, status: i32) -> Result<(), EntryError> {
        self.check_thread(thread)?;
        if self.0.phase != RuntimePhase::Cleaned {
            return Err(EntryError::InvalidState);
        }
        self.0.return_status = Some(status);
        self.0.phase = RuntimePhase::Returned;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_checks_arguments_and_initial_thread_without_changing_state() {
        let mut state = EntryState::new();
        assert_eq!(state.prepare(7, 2, 0x1000), Ok(()));
        let before = state.snapshot();
        assert_eq!(state.enter(8, 2, 0x1000), Err(EntryError::WrongThread));
        assert_eq!(state.enter(7, 3, 0x1000), Err(EntryError::InvalidArguments));
        assert_eq!(state.enter(7, 2, 0x2000), Err(EntryError::InvalidArguments));
        assert_eq!(state.snapshot(), before);
        assert_eq!(state.enter(7, 2, 0x1000), Ok(()));
        assert_eq!(state.enter(7, 2, 0x1000), Err(EntryError::InvalidState));
    }

    #[test]
    fn cleanup_success_must_precede_return_and_can_only_do_work_once() {
        let mut state = EntryState::new();
        state.prepare(7, 0, 0).unwrap();
        state.enter(7, 0, 0).unwrap();
        assert_eq!(state.returned(7, 42), Err(EntryError::InvalidState));
        assert_eq!(state.begin_cleanup(8), Err(EntryError::WrongThread));
        assert_eq!(state.begin_cleanup(7), Ok(true));
        assert_eq!(state.begin_cleanup(7), Err(EntryError::InvalidState));
        assert_eq!(state.complete_cleanup(0), Ok(()));
        assert_eq!(state.begin_cleanup(7), Ok(false));
        assert_eq!(state.returned(7, 42), Ok(()));
        assert_eq!(state.snapshot().return_status, Some(42));
        assert_eq!(state.snapshot().cleanup_result, Some(0));
        assert_eq!(state.begin_cleanup(7), Err(EntryError::InvalidState));
    }

    #[test]
    fn cleanup_failure_stays_failed_and_preserves_the_result() {
        let mut state = EntryState::new();
        state.prepare(7, 0, 0).unwrap();
        // A C-entry application has no compiler-generated enter call.
        assert_eq!(state.begin_cleanup(7), Ok(true));
        assert_eq!(
            state.complete_cleanup(-5),
            Err(EntryError::CleanupFailed(-5))
        );
        assert_eq!(state.begin_cleanup(7), Err(EntryError::CleanupFailed(-5)));
        assert_eq!(state.returned(7, 0), Err(EntryError::InvalidState));
        assert_eq!(state.snapshot().phase, RuntimePhase::Failed);
        assert_eq!(state.snapshot().cleanup_result, Some(-5));
        assert_eq!(state.snapshot().return_status, None);
    }

    #[test]
    fn malformed_entry_metadata_is_rejected_before_preparation() {
        for (thread, argc, argv) in [
            (0, 0, 0),
            (u32::MAX, 0, 0),
            (7, -1, 0),
            (7, 1, 0),
            (7, 1, 0x1001),
        ] {
            let mut state = EntryState::new();
            assert_eq!(
                state.prepare(thread, argc, argv),
                Err(EntryError::InvalidArguments)
            );
            assert_eq!(state.snapshot().phase, RuntimePhase::Cold);
        }
    }
}
