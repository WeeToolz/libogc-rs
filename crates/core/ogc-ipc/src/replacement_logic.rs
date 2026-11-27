pub(crate) fn retire_after_fresh_handshake(
    handshake: impl FnOnce() -> bool,
    retire: impl FnOnce(),
) {
    if !handshake() {
        return;
    }
    retire();
}

#[cfg(test)]
mod tests {
    use super::retire_after_fresh_handshake;

    #[test]
    fn failed_handshake_preserves_requests_and_delivers_no_callbacks() {
        let in_flight = core::cell::Cell::new(3usize);
        let callbacks = core::cell::Cell::new(0usize);
        retire_after_fresh_handshake(|| false, || {
            let completed = in_flight.replace(0);
            callbacks.set(completed);
        });
        assert_eq!(in_flight.get(), 3);
        assert_eq!(callbacks.get(), 0);
    }

    #[test]
    fn successful_handshake_retires_each_request_once() {
        let in_flight = core::cell::Cell::new(3usize);
        let callbacks = core::cell::Cell::new(0usize);
        retire_after_fresh_handshake(|| true, || {
            let completed = in_flight.replace(0);
            callbacks.set(completed);
        });
        assert_eq!(in_flight.get(), 0);
        assert_eq!(callbacks.get(), 3);
    }
}
