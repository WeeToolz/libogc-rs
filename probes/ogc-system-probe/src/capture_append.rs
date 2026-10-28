use core::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppendError {
    CapacityExceeded,
}

/// Return the all-or-nothing destination range for one capture append.
pub fn append_range(
    current_len: usize,
    incoming_len: usize,
    capacity: usize,
) -> Result<Range<usize>, AppendError> {
    let end = current_len
        .checked_add(incoming_len)
        .filter(|end| *end <= capacity)
        .ok_or(AppendError::CapacityExceeded)?;
    Ok(current_len..end)
}

/// Copy one complete event into an already reserved destination range.
pub fn copy_reserved(destination: &mut [u8], incoming: &[u8]) -> Result<(), AppendError> {
    if destination.len() != incoming.len() {
        return Err(AppendError::CapacityExceeded);
    }
    destination.copy_from_slice(incoming);
    Ok(())
}

/// Append one complete event to a single-owner capture buffer.
pub fn append(
    buffer: &mut [u8],
    current_len: &mut usize,
    incoming: &[u8],
) -> Result<(), AppendError> {
    append_before_publish(buffer, current_len, incoming, |_, _| {})
}

/// Append bytes, then run a bounded hook before publishing the new visible
/// length. Production uses an empty hook; tests inspect reader visibility at
/// this exact boundary.
pub fn append_before_publish<F>(
    buffer: &mut [u8],
    current_len: &mut usize,
    incoming: &[u8],
    before_publish: F,
) -> Result<(), AppendError>
where
    F: FnOnce(&[u8], usize),
{
    let range = append_range(*current_len, incoming.len(), buffer.len())?;
    copy_reserved(&mut buffer[range.clone()], incoming)?;
    before_publish(buffer, *current_len);
    *current_len = range.end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppendError, append, append_before_publish, append_range};

    #[test]
    fn accepts_full_line_at_exact_capacity() {
        assert_eq!(append_range(4, 4, 8), Ok(4..8));
    }

    #[test]
    fn rejects_overflow_without_advancing_the_capture_length() {
        let mut buffer = [0xaa; 8];
        let mut current_len = 7;
        assert_eq!(
            append(&mut buffer, &mut current_len, b"xy"),
            Err(AppendError::CapacityExceeded)
        );
        assert_eq!(current_len, 7);
        assert_eq!(buffer, [0xaa; 8]);
        append(&mut buffer, &mut current_len, b"z").unwrap();
        assert_eq!(current_len, 8);
        assert_eq!(buffer[7], b'z');
    }

    #[test]
    fn rejects_integer_overflow() {
        assert_eq!(
            append_range(usize::MAX, 1, usize::MAX),
            Err(AppendError::CapacityExceeded)
        );
    }

    #[test]
    fn incomplete_append_is_not_visible_until_copy_finishes() {
        let mut buffer = [0; 8];
        let mut visible_len = 2;
        buffer[..2].copy_from_slice(b"ok");
        append_before_publish(&mut buffer, &mut visible_len, b"event", |bytes, len| {
            assert_eq!(&bytes[..len], b"ok");
        })
        .unwrap();
        assert_eq!(&buffer[..visible_len], b"okevent");
    }
}
