//! Close only an existing transport; retain ownership when close fails.

pub(crate) fn close_transport<T>(
    transport: &mut Option<T>,
    close: impl FnOnce(&T) -> Result<(), i32>,
) -> Result<(), i32> {
    if let Some(handle) = transport.as_ref() {
        close(handle)?;
        *transport = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_shutdown_does_not_open_or_stop_a_drive() {
        let mut transport: Option<u32> = None;
        assert_eq!(
            close_transport(&mut transport, |_| panic!("no active transport")),
            Ok(())
        );
        assert_eq!(transport, None);
    }

    #[test]
    fn close_failure_keeps_the_handle_for_a_real_retry() {
        let mut transport = Some(7);
        assert_eq!(
            close_transport(&mut transport, |handle| {
                assert_eq!(*handle, 7);
                Err(-8)
            }),
            Err(-8)
        );
        assert_eq!(transport, Some(7));
        assert_eq!(
            close_transport(&mut transport, |handle| {
                assert_eq!(*handle, 7);
                Ok(())
            }),
            Ok(())
        );
        assert_eq!(transport, None);
    }
}
