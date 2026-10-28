/// Completion accounting shared by the hardware cleanup path and host tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControllerCleanupState {
    usb_complete: bool,
    bluetooth_complete: bool,
    failed: bool,
}

impl ControllerCleanupState {
    pub const fn new() -> Self {
        Self {
            usb_complete: false,
            bluetooth_complete: false,
            failed: false,
        }
    }

    pub fn observe_usb_complete(&mut self, complete: bool) {
        self.usb_complete = complete;
    }

    pub fn observe_bluetooth_complete(&mut self, complete: bool) {
        self.bluetooth_complete = complete;
    }

    pub fn record_failure(&mut self) {
        self.failed = true;
    }

    pub const fn is_complete(self) -> bool {
        self.transports_complete() && !self.failed
    }

    pub const fn transports_complete(self) -> bool {
        self.usb_complete && self.bluetooth_complete
    }

    pub const fn usb_complete(self) -> bool {
        self.usb_complete
    }
}

#[cfg(test)]
mod tests {
    use super::ControllerCleanupState;

    #[test]
    fn shutdown_submission_does_not_stand_in_for_completion() {
        let mut state = ControllerCleanupState::new();
        state.observe_usb_complete(false);
        state.observe_bluetooth_complete(false);
        assert!(!state.is_complete());

        state.observe_usb_complete(true);
        assert!(!state.is_complete());
        state.observe_bluetooth_complete(true);
        assert!(state.is_complete());
    }

    #[test]
    fn transport_or_cleanup_errors_keep_verdict_failed_after_completion() {
        let mut state = ControllerCleanupState::new();
        state.observe_usb_complete(true);
        state.observe_bluetooth_complete(true);
        state.record_failure();
        assert!(!state.is_complete());
    }
}
