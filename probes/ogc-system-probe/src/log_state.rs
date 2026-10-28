//! Portable state for bounded probe log output.
//!
//! This module is shared by the PowerPC logger and host-side failure tests.

extern crate alloc;

use alloc::vec::Vec;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LogError {
    Capacity,
    BackendUnavailable,
    ScreenWrite(i32),
    FileWrite(i32),
    Sync(i32),
}

/// Write and sync a non-authoritative summary candidate before publishing the
/// final verdict. Every operation runs, even after an earlier failure, so
/// cleanup and diagnostics can complete while the final status stays failed.
pub fn finish_summary<T>(
    context: &mut T,
    mut prepare: impl FnMut(&mut T) -> bool,
    mut write_candidate: impl FnMut(&mut T) -> bool,
    mut final_sync: impl FnMut(&mut T) -> bool,
    mut publish: impl FnMut(&mut T, bool),
) -> bool {
    let prepared = prepare(context);
    let candidate_written = write_candidate(context);
    let synchronized = final_sync(context);
    let passed = prepared && candidate_written && synchronized;
    publish(context, passed);
    passed
}

#[derive(Default)]
pub struct LogState {
    pending: Vec<u8>,
    lost_bytes: usize,
    first_error: Option<LogError>,
    file_write_failed: bool,
}

impl LogState {
    #[cfg(test)]
    pub fn pending(&self) -> &[u8] {
        &self.pending
    }

    pub const fn lost_bytes(&self) -> usize {
        self.lost_bytes
    }

    pub const fn first_error(&self) -> Option<LogError> {
        self.first_error
    }

    pub const fn is_healthy(&self) -> bool {
        self.first_error.is_none() && self.lost_bytes == 0
    }

    #[cfg(test)]
    pub const fn file_write_failed(&self) -> bool {
        self.file_write_failed
    }

    pub fn append(&mut self, bytes: &[u8], limit: Option<usize>, committed: usize) {
        let accepted = match limit {
            Some(limit) => bytes
                .len()
                .min(limit.saturating_sub(committed.saturating_add(self.pending.len()))),
            None => bytes.len(),
        };
        self.pending.extend_from_slice(&bytes[..accepted]);
        if accepted != bytes.len() {
            self.lost_bytes = self.lost_bytes.saturating_add(bytes.len() - accepted);
            self.fail(LogError::Capacity);
        }
    }

    pub fn commit_pending(&mut self, length: usize) {
        let consumed = length.min(self.pending.len());
        self.pending.drain(..consumed);
    }

    /// Writes the current queue through a backend and commits it only when the
    /// backend reports complete success. A failed write remains pending and is
    /// not retried because the backend may have accepted a prefix first.
    pub fn flush_with(
        &mut self,
        write_all: impl FnOnce(&[u8]) -> Result<(), i32>,
    ) -> Result<usize, i32> {
        if self.file_write_failed || self.pending.is_empty() {
            return Ok(0);
        }
        let length = self.pending.len();
        match write_all(&self.pending) {
            Ok(()) => {
                self.commit_pending(length);
                Ok(length)
            }
            Err(code) => {
                self.fail(LogError::FileWrite(code));
                Err(code)
            }
        }
    }

    pub fn fail(&mut self, error: LogError) {
        if matches!(error, LogError::FileWrite(_)) {
            self.file_write_failed = true;
        }
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogError, LogState};

    #[test]
    fn failed_write_retains_all_pending_bytes() {
        let mut state = LogState::default();
        state.append(b"first", None, 0);
        state.append(b" second", None, 0);
        let result = state.flush_with(|bytes| {
            assert_eq!(bytes, b"first second");
            Err(-5)
        });

        assert_eq!(result, Err(-5));
        assert_eq!(state.pending(), b"first second");
        assert_eq!(state.first_error(), Some(LogError::FileWrite(-5)));
        assert!(state.file_write_failed());
        assert_eq!(state.flush_with(|_| Ok(())), Ok(0));
        assert_eq!(state.pending(), b"first second");
    }

    #[test]
    fn successful_partial_commit_retains_unwritten_tail() {
        let mut state = LogState::default();
        state.append(b"abcdef", None, 0);
        state.commit_pending(2);

        assert_eq!(state.pending(), b"cdef");
    }

    #[test]
    fn successful_backend_write_commits_all_pending_bytes() {
        let mut state = LogState::default();
        state.append(b"durable log", None, 0);

        let result = state.flush_with(|bytes| {
            assert_eq!(bytes, b"durable log");
            Ok(())
        });

        assert_eq!(result, Ok(11));
        assert!(state.pending().is_empty());
        assert!(state.is_healthy());
    }

    #[test]
    fn capacity_loss_is_counted_and_sticky() {
        let mut state = LogState::default();
        state.append(b"abc", Some(2), 0);
        state.commit_pending(2);
        state.append(b"de", Some(2), 2);

        assert_eq!(state.pending(), b"");
        assert_eq!(state.lost_bytes(), 3);
        assert_eq!(state.first_error(), Some(LogError::Capacity));
        assert!(!state.is_healthy());
    }

    #[test]
    fn first_failure_is_preserved() {
        let mut state = LogState::default();
        state.fail(LogError::BackendUnavailable);
        state.fail(LogError::Sync(-7));

        assert_eq!(state.first_error(), Some(LogError::BackendUnavailable));
    }

    #[test]
    fn late_sync_failure_keeps_log_unhealthy_for_final_summary() {
        use super::finish_summary;

        #[derive(Default)]
        struct Sink {
            events: Vec<&'static str>,
            final_pass: Option<bool>,
        }

        let mut sink = Sink::default();
        let passed = finish_summary(
            &mut sink,
            |sink| {
                sink.events.push("prepare");
                true
            },
            |sink| {
                sink.events.push("candidate");
                true
            },
            |sink| {
                sink.events.push("sync_failed");
                false
            },
            |sink, passed| {
                assert_eq!(sink.events, ["prepare", "candidate", "sync_failed"]);
                sink.events.push("publish");
                sink.final_pass = Some(passed);
            },
        );

        assert!(!passed);
        assert_eq!(sink.events.last(), Some(&"publish"));
        assert_eq!(sink.final_pass, Some(false));
    }

    #[test]
    fn candidate_write_failure_publishes_fail_after_sync_attempt() {
        use super::finish_summary;

        let mut events = Vec::new();
        let passed = finish_summary(
            &mut events,
            |events| {
                events.push("prepare");
                true
            },
            |events| {
                events.push("candidate_failed");
                false
            },
            |events| {
                events.push("sync_attempted");
                true
            },
            |events, passed| events.push(if passed { "PASS" } else { "FAIL" }),
        );

        assert!(!passed);
        assert_eq!(
            events,
            ["prepare", "candidate_failed", "sync_attempted", "FAIL"]
        );
    }

    #[test]
    fn successful_summary_publishes_only_after_candidate_sync() {
        use super::finish_summary;

        let mut events = Vec::new();
        let passed = finish_summary(
            &mut events,
            |events| {
                events.push("prepare");
                true
            },
            |events| {
                events.push("candidate");
                true
            },
            |events| {
                events.push("sync");
                true
            },
            |events, passed| events.push(if passed { "PASS" } else { "FAIL" }),
        );

        assert!(passed);
        assert_eq!(events, ["prepare", "candidate", "sync", "PASS"]);
    }
}
