//! Shared startup, presence and failure cleanup used by FAT mounts.
pub(crate) fn check_device_presence<E>(
    mounted: bool,
    startup: impl FnOnce() -> bool,
    is_inserted: impl FnOnce() -> bool,
    shutdown: impl FnOnce() -> Result<(), E>,
) -> Result<bool, E> {
    if !mounted && !startup() {
        return Ok(false);
    }
    let inserted = is_inserted();
    if !mounted && !inserted {
        shutdown()?;
    }
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_media_releases_a_started_device() {
        let mut stopped = false;
        assert_eq!(
            check_device_presence(
                false,
                || true,
                || false,
                || {
                    stopped = true;
                    Ok::<_, i32>(())
                }
            ),
            Ok(false)
        );
        assert!(stopped);
    }
    #[test]
    fn cleanup_failure_reaches_the_mount_caller() {
        assert_eq!(
            check_device_presence(false, || true, || false, || Err(-7)),
            Err(-7)
        );
    }
    #[test]
    fn mounted_devices_are_not_restarted_or_closed() {
        assert_eq!(
            check_device_presence(true, || panic!("restart"), || false, || panic!("close")),
            Ok::<_, i32>(false)
        );
    }
    #[test]
    fn startup_failure_does_not_probe_or_close_an_unstarted_device() {
        assert_eq!(
            check_device_presence(false, || false, || panic!("probe"), || panic!("close")),
            Ok::<_, i32>(false)
        );
    }
}
