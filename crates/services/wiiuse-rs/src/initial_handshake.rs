//! Initial connection handshake used by Wii Remote setup.
//!
//! This state machine ports the status, expansion ID, and calibration stages
//! from `C-sources/wiiuse/src/io.c` and `C-sources/libogc/wiiuse/io.c`.

/// Expansion identifier for the Wii U Pro controller.
pub const WII_U_PRO_EXPANSION_ID: u32 = 0xa420_0120;

/// The next remote operation after a valid report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandshakeAction {
    /// Read the expansion identifier before reading calibration.
    ReadExpansionId,
    /// Read the Wii Remote accelerometer calibration.
    ReadCalibration,
    /// Refresh the remote status after initialization.
    RefreshStatus,
}

/// Invalid report order for the initial handshake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitialHandshakeError {
    /// The status report arrived outside the initial status stage.
    UnexpectedStatus,
    /// The expansion ID arrived before the status report requested it.
    UnexpectedExpansionId,
    /// Calibration completed before the calibration read was requested.
    UnexpectedCalibration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    AwaitingStatus,
    AwaitingExpansionId,
    AwaitingCalibration,
    Complete,
}

/// Tracks the ordered initialization of one Wii Remote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitialHandshake {
    phase: Phase,
}

impl InitialHandshake {
    /// Create a handshake that waits for the first status report.
    pub const fn new() -> Self {
        Self {
            phase: Phase::AwaitingStatus,
        }
    }

    /// Start the expansion ID or calibration stage from the first status.
    pub fn status_report(
        &mut self,
        has_expansion: bool,
    ) -> Result<HandshakeAction, InitialHandshakeError> {
        if self.phase != Phase::AwaitingStatus {
            return Err(InitialHandshakeError::UnexpectedStatus);
        }
        if has_expansion {
            self.phase = Phase::AwaitingExpansionId;
            Ok(HandshakeAction::ReadExpansionId)
        } else {
            self.phase = Phase::AwaitingCalibration;
            Ok(HandshakeAction::ReadCalibration)
        }
    }

    /// Continue from the initial expansion ID read.
    pub fn expansion_id(&mut self, id: u32) -> Result<HandshakeAction, InitialHandshakeError> {
        if self.phase != Phase::AwaitingExpansionId {
            return Err(InitialHandshakeError::UnexpectedExpansionId);
        }
        if id == WII_U_PRO_EXPANSION_ID {
            self.phase = Phase::Complete;
            Ok(HandshakeAction::RefreshStatus)
        } else {
            self.phase = Phase::AwaitingCalibration;
            Ok(HandshakeAction::ReadCalibration)
        }
    }

    /// Mark the calibration read complete and request the final status.
    pub fn calibration_complete(&mut self) -> Result<HandshakeAction, InitialHandshakeError> {
        if self.phase != Phase::AwaitingCalibration {
            return Err(InitialHandshakeError::UnexpectedCalibration);
        }
        self.phase = Phase::Complete;
        Ok(HandshakeAction::RefreshStatus)
    }

    /// Return the handshake to the status stage after a failed read request.
    pub fn retry(&mut self) {
        if self.phase != Phase::Complete {
            self.phase = Phase::AwaitingStatus;
        }
    }

    /// Return whether the initial handshake completed.
    pub const fn is_complete(&self) -> bool {
        matches!(self.phase, Phase::Complete)
    }
}

impl Default for InitialHandshake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_without_expansion_reads_calibration_before_ready() {
        let mut handshake = InitialHandshake::new();
        assert_eq!(
            handshake.status_report(false),
            Ok(HandshakeAction::ReadCalibration)
        );
        assert!(!handshake.is_complete());
        assert_eq!(
            handshake.calibration_complete(),
            Ok(HandshakeAction::RefreshStatus)
        );
        assert!(handshake.is_complete());
    }

    #[test]
    fn standard_remote_reads_expansion_id_then_calibration() {
        let mut handshake = InitialHandshake::new();
        assert_eq!(
            handshake.status_report(true),
            Ok(HandshakeAction::ReadExpansionId)
        );
        assert_eq!(
            handshake.expansion_id(0),
            Ok(HandshakeAction::ReadCalibration)
        );
        assert!(!handshake.is_complete());
        assert_eq!(
            handshake.calibration_complete(),
            Ok(HandshakeAction::RefreshStatus)
        );
        assert!(handshake.is_complete());
    }

    #[test]
    fn wii_u_pro_completes_without_calibration_read() {
        let mut handshake = InitialHandshake::new();
        assert_eq!(
            handshake.status_report(true),
            Ok(HandshakeAction::ReadExpansionId)
        );
        assert_eq!(
            handshake.expansion_id(WII_U_PRO_EXPANSION_ID),
            Ok(HandshakeAction::RefreshStatus)
        );
        assert!(handshake.is_complete());
        assert_eq!(
            handshake.calibration_complete(),
            Err(InitialHandshakeError::UnexpectedCalibration)
        );
    }

    #[test]
    fn invalid_order_is_returned_and_failed_request_can_retry() {
        let mut handshake = InitialHandshake::new();
        assert_eq!(
            handshake.expansion_id(0),
            Err(InitialHandshakeError::UnexpectedExpansionId)
        );
        assert_eq!(
            handshake.status_report(false),
            Ok(HandshakeAction::ReadCalibration)
        );
        handshake.retry();
        assert_eq!(
            handshake.status_report(true),
            Ok(HandshakeAction::ReadExpansionId)
        );
    }
}
