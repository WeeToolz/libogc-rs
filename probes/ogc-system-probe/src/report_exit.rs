/// Continue after a report failure only when the caller requested an exit.
pub const fn should_continue_after_report_failure(exit_requested: bool) -> bool {
    exit_requested
}

/// Keep the recovery watchdog armed while a requested reset is in progress.
pub const fn should_cancel_recovery_watchdog(
    report_succeeded: bool,
    system_reset_requested: bool,
) -> bool {
    report_succeeded && !system_reset_requested
}

#[cfg(test)]
mod tests {
    use super::{should_cancel_recovery_watchdog, should_continue_after_report_failure};

    #[test]
    fn report_failure_keeps_a_requested_exit_active() {
        assert!(should_continue_after_report_failure(true));
    }

    #[test]
    fn report_failure_keeps_the_probe_open_without_an_exit_request() {
        assert!(!should_continue_after_report_failure(false));
    }

    #[test]
    fn successful_report_does_not_cancel_watchdog_before_system_reset() {
        assert!(!should_cancel_recovery_watchdog(true, true));
    }

    #[test]
    fn successful_report_cancels_watchdog_without_system_reset() {
        assert!(should_cancel_recovery_watchdog(true, false));
    }

    #[test]
    fn failed_report_keeps_recovery_watchdog_armed() {
        assert!(!should_cancel_recovery_watchdog(false, false));
        assert!(!should_cancel_recovery_watchdog(false, true));
    }
}
