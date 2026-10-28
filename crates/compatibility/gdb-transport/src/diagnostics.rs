//! Bounded listener diagnostics. The network worker owns this record; queries
//! read it on that same worker. It survives GDB listener failure until restart.

pub const DIAGNOSTIC_QUERY: [u8; 16] = *b"HBCD\x01\0\0\0\0\0\0\0\0\0\0\0";
pub const DIAGNOSTIC_REPLY_LEN: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DiagnosticOperation {
    HostIp = 1,
    Socket = 2,
    GetFlags = 3,
    SetFlags = 4,
    Bind = 5,
    Listen = 6,
    Accept = 7,
    Read = 8,
    Write = 9,
    Close = 10,
    Transport = 11,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticFailure {
    pub operation: DiagnosticOperation,
    /// None means no IOS error is available, for example an unavailable host IP
    /// or a transport invariant failure.
    pub code: Option<i32>,
    pub cleanup: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ListenerDiagnosticState {
    Starting = 1,
    Listening = 2,
    StartupFailed = 3,
    PollFailed = 4,
    Stopped = 5,
    CleanupFailed = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListenerDiagnostics {
    state: ListenerDiagnosticState,
    ever_opened: bool,
    listener_stopped: bool,
    failure: Option<DiagnosticFailure>,
}

impl Default for ListenerDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl ListenerDiagnostics {
    pub const fn new() -> Self {
        Self {
            state: ListenerDiagnosticState::Starting,
            ever_opened: false,
            listener_stopped: false,
            failure: None,
        }
    }

    pub fn listening(&mut self) {
        self.state = ListenerDiagnosticState::Listening;
        self.ever_opened = true;
    }

    pub fn startup_failed(&mut self, failure: DiagnosticFailure) {
        self.state = ListenerDiagnosticState::StartupFailed;
        self.failure = Some(failure);
    }

    pub fn poll_failed(&mut self, failure: DiagnosticFailure) {
        self.state = ListenerDiagnosticState::PollFailed;
        self.failure = Some(failure);
    }

    pub fn listener_closed(&mut self) {
        self.listener_stopped = self.ever_opened;
        if self.failure.is_none() {
            self.state = ListenerDiagnosticState::Stopped;
        }
    }

    pub fn cleanup_failed(&mut self, failure: DiagnosticFailure) {
        match self.failure.as_mut() {
            Some(original) => {
                if original.cleanup.is_none() {
                    original.cleanup = failure.code.or(failure.cleanup);
                }
            }
            None => {
                self.failure = Some(failure);
                self.state = ListenerDiagnosticState::CleanupFailed;
            }
        }
    }

    /// Fixed binary response. Malformed queries return status 1 and no snapshot.
    /// Flags: ever opened, listener closed, IOS code present, cleanup code present.
    pub fn reply(&self, query: &[u8; 16]) -> [u8; DIAGNOSTIC_REPLY_LEN] {
        let mut reply = [0; DIAGNOSTIC_REPLY_LEN];
        reply[..4].copy_from_slice(b"HBCD");
        reply[4] = 1;
        if query != &DIAGNOSTIC_QUERY {
            reply[5] = 1;
            return reply;
        }
        reply[6] = self.state as u8;
        reply[7] = u8::from(self.ever_opened) | (u8::from(self.listener_stopped) << 1);
        if let Some(failure) = self.failure {
            reply[8] = failure.operation as u8;
            if let Some(code) = failure.code {
                reply[7] |= 4;
                reply[12..16].copy_from_slice(&code.to_be_bytes());
            }
            if let Some(code) = failure.cleanup {
                reply[7] |= 8;
                reply[16..20].copy_from_slice(&code.to_be_bytes());
            }
        }
        reply[20..22].copy_from_slice(&super::DEFAULT_PORT.to_be_bytes());
        reply
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listening_reply_uses_fixed_wire_values() {
        let mut diagnostics = ListenerDiagnostics::new();
        diagnostics.listening();
        assert_eq!(
            diagnostics.reply(&DIAGNOSTIC_QUERY),
            [
                72, 66, 67, 68, 1, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11, 12, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0
            ]
        );
    }

    #[test]
    fn startup_failure_remains_queryable_without_claiming_listener_opened() {
        let mut diagnostics = ListenerDiagnostics::new();
        diagnostics.startup_failed(DiagnosticFailure {
            operation: DiagnosticOperation::Bind,
            code: Some(-3),
            cleanup: Some(-29),
        });
        let expected = [
            72, 66, 67, 68, 1, 0, 3, 12, 5, 0, 0, 0, 255, 255, 255, 253, 255, 255, 255, 227, 11,
            12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(diagnostics.reply(&DIAGNOSTIC_QUERY), expected);
        assert_eq!(diagnostics.reply(&DIAGNOSTIC_QUERY), expected);
    }

    #[test]
    fn poll_failure_and_cleanup_preserve_original_ios_error() {
        let mut diagnostics = ListenerDiagnostics::new();
        diagnostics.listening();
        diagnostics.poll_failed(DiagnosticFailure {
            operation: DiagnosticOperation::Accept,
            code: Some(-28),
            cleanup: None,
        });
        diagnostics.cleanup_failed(DiagnosticFailure {
            operation: DiagnosticOperation::Close,
            code: Some(-29),
            cleanup: None,
        });
        let before = diagnostics.reply(&DIAGNOSTIC_QUERY);
        assert_eq!(&before[6..9], &[4, 13, 7]);
        assert_eq!(&before[12..20], &[255, 255, 255, 228, 255, 255, 255, 227]);
        diagnostics.listener_closed();
        assert_eq!(&diagnostics.reply(&DIAGNOSTIC_QUERY)[6..9], &[4, 15, 7]);
    }

    #[test]
    fn malformed_query_is_rejected_without_changing_record() {
        let diagnostics = ListenerDiagnostics::new();
        let mut bad = DIAGNOSTIC_QUERY;
        bad[15] = 1;
        assert_eq!(
            diagnostics.reply(&bad),
            [
                72, 66, 67, 68, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0
            ]
        );
        assert_eq!(diagnostics.reply(&DIAGNOSTIC_QUERY)[6], 1);
    }

    #[test]
    fn each_backend_stage_preserves_its_wire_operation_and_raw_error() {
        let stages = [
            (DiagnosticOperation::HostIp, 1),
            (DiagnosticOperation::Socket, 2),
            (DiagnosticOperation::GetFlags, 3),
            (DiagnosticOperation::SetFlags, 4),
            (DiagnosticOperation::Bind, 5),
            (DiagnosticOperation::Listen, 6),
            (DiagnosticOperation::Accept, 7),
            (DiagnosticOperation::Read, 8),
            (DiagnosticOperation::Write, 9),
            (DiagnosticOperation::Close, 10),
        ];
        for (operation, expected_wire) in stages {
            let mut diagnostics = ListenerDiagnostics::new();
            diagnostics.startup_failed(DiagnosticFailure {
                operation,
                code: Some(-28),
                cleanup: None,
            });
            let reply = diagnostics.reply(&DIAGNOSTIC_QUERY);
            assert_eq!(reply[8], expected_wire);
            assert_eq!(&reply[12..16], &[255, 255, 255, 228]);
        }
    }

    #[test]
    fn clean_close_and_failed_close_have_distinct_observable_results() {
        let mut diagnostics = ListenerDiagnostics::new();
        diagnostics.listening();
        diagnostics.cleanup_failed(DiagnosticFailure {
            operation: DiagnosticOperation::Close,
            code: Some(-29),
            cleanup: None,
        });
        assert_eq!(&diagnostics.reply(&DIAGNOSTIC_QUERY)[6..9], &[6, 5, 10]);
        let mut clean = ListenerDiagnostics::new();
        clean.listening();
        clean.listener_closed();
        assert_eq!(&clean.reply(&DIAGNOSTIC_QUERY)[6..9], &[5, 3, 0]);
    }

    #[test]
    fn transport_failure_does_not_invent_ios_error() {
        let mut diagnostics = ListenerDiagnostics::new();
        diagnostics.listening();
        diagnostics.poll_failed(DiagnosticFailure {
            operation: DiagnosticOperation::Transport,
            code: None,
            cleanup: None,
        });
        assert_eq!(&diagnostics.reply(&DIAGNOSTIC_QUERY)[6..9], &[4, 1, 11]);
    }
}
