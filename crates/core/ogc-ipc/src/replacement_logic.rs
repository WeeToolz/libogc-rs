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
    fn failed_handshake_keeps_requests_unretired() {
        let retired = core::cell::Cell::new(false);
        retire_after_fresh_handshake(|| false, || retired.set(true));
        assert!(!retired.get());
    }

    #[test]
    fn successful_handshake_retires_requests() {
        let retired = core::cell::Cell::new(false);
        retire_after_fresh_handshake(|| true, || retired.set(true));
        assert!(retired.get());
    }
}
