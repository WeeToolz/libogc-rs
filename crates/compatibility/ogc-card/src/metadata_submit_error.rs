extern crate alloc;
use self::alloc::boxed::Box;

/// Submission failure that returns the metadata value not retained by CARD.
pub struct CardMetadataSubmitError<T> {
    status: i32,
    value: Box<T>,
}

impl<T> CardMetadataSubmitError<T> {
    pub(crate) fn new(status: i32, value: T) -> Self {
        Self {
            status,
            value: Box::new(value),
        }
    }

    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub fn into_value(self) -> T {
        *self.value
    }

    pub fn into_parts(self) -> (i32, T) {
        (self.status, *self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::CardMetadataSubmitError;
    use super::alloc::{rc::Rc, vec};
    use core::cell::Cell;

    #[test]
    fn error_preserves_status_and_original_payload() {
        let payload = vec![0x12, 0x34, 0x56];
        let pointer = payload.as_ptr();
        let error = CardMetadataSubmitError::new(-16, payload);
        assert_eq!(error.status(), -16);
        assert_eq!(error.value().as_slice(), &[0x12, 0x34, 0x56]);
        let (status, payload) = error.into_parts();
        assert_eq!(status, -16);
        assert_eq!(payload.as_ptr(), pointer);
        assert_eq!(payload, [0x12, 0x34, 0x56]);
    }

    struct TrackedPayload(Rc<Cell<usize>>);

    impl Drop for TrackedPayload {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn taking_error_value_transfers_ownership_once() {
        let drops = Rc::new(Cell::new(0));
        let error = CardMetadataSubmitError::new(-5, TrackedPayload(Rc::clone(&drops)));
        let payload = error.into_value();
        assert_eq!(drops.get(), 0);
        drop(payload);
        assert_eq!(drops.get(), 1);
    }
}
