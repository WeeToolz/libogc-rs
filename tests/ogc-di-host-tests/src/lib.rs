#[cfg(test)]
#[path = "../../../crates/services/ogc-di/src/retry.rs"]
mod retry;

#[cfg(test)]
mod tests {
    use super::retry::{retry_di_read, LIBDI_MAX_RETRIES};

    const EIO: i32 = -5;

    #[test]
    fn retries_after_transient_failure_and_stops_on_success() {
        let mut attempts = 0;
        let result = retry_di_read(
            || {
                attempts += 1;
                if attempts == 3 {
                    0
                } else {
                    EIO
                }
            },
            LIBDI_MAX_RETRIES,
        );

        assert_eq!(result, 0);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn stops_after_maximum_attempts() {
        let mut attempts = 0;
        let result = retry_di_read(
            || {
                attempts += 1;
                EIO
            },
            LIBDI_MAX_RETRIES,
        );

        assert_eq!(result, EIO);
        assert_eq!(attempts, LIBDI_MAX_RETRIES);
    }
}
