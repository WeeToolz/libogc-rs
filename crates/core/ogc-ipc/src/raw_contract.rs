//! Bounds and address rules for synchronous raw IOCTLV requests.
pub(crate) fn mailbox_after_send(count: i32, relaunch: bool, background: bool) -> i32 {
    count - 1 - i32::from(relaunch && !background)
}

pub(crate) fn mailbox_after_ack(count: i32) -> i32 {
    if count < 1 { count + 1 } else { count }
}
pub(crate) fn default_buffer_bounds(
    linker: (usize, usize),
    loader: (usize, usize),
) -> (usize, usize) {
    (
        if loader.0 == 0 { linker.0 } else { loader.0 },
        if loader.1 == 0 { linker.1 } else { loader.1 },
    )
}

pub(crate) fn heap_create_preflight(
    size: usize,
    slot_available: bool,
    minimum_size: usize,
    no_heap_error: i32,
    no_memory_error: i32,
) -> Result<(), i32> {
    if !slot_available {
        Err(no_heap_error)
    } else if size < minimum_size {
        Err(no_memory_error)
    } else {
        Ok(())
    }
}

pub(crate) fn vector_layout(
    input: usize,
    output: usize,
    capacity: usize,
    vector_size: usize,
    pointer: usize,
) -> Option<(usize, usize)> {
    let total = input.checked_add(output)?;
    let length = total.checked_mul(vector_size)?;
    if total == 0
        || total > capacity
        || input > i32::MAX as usize
        || output > i32::MAX as usize
        || length > isize::MAX as usize
        || pointer == 0
        || !pointer.is_multiple_of(32)
        || pointer.checked_add(length).is_none()
    {
        None
    } else {
        Some((total, length))
    }
}
pub(crate) fn buffer_valid(pointer: usize, length: usize) -> bool {
    length == 0
        || (pointer != 0
            && pointer.is_multiple_of(32)
            && length <= isize::MAX as usize
            && pointer
                .checked_add(length)
                .and_then(|end| end.checked_add(31))
                .is_some())
}
/// A zero-length IOS address is protocol data, not a CPU RAM pointer.
pub(crate) fn request_address(pointer: usize, length: usize) -> usize {
    if length == 0 {
        pointer
    } else {
        pointer & !0xc000_0000usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relaunch_acknowledgements_match_c_mailbox_count() {
        assert_eq!(mailbox_after_send(1, true, false), -1);
        assert_eq!(mailbox_after_send(1, true, true), 0);
        assert_eq!(mailbox_after_send(1, false, false), 0);
        assert_eq!(mailbox_after_ack(-1), 0);
        assert_eq!(mailbox_after_ack(0), 1);
        assert_eq!(mailbox_after_ack(1), 1);
        // A reply also returns one mailbox credit while a relaunch is pending.
        // Either event order leaves the same count after two events.
        let foreground = mailbox_after_send(1, true, false);
        assert_eq!(mailbox_after_ack(mailbox_after_ack(foreground)), 1);
        let background = mailbox_after_send(1, true, true);
        assert_eq!(mailbox_after_ack(mailbox_after_ack(background)), 1);
    }

    #[test]
    fn zero_sized_heap_error_matches_c_slot_and_memory_order() {
        assert_eq!(heap_create_preflight(0, true, 40, -5, -22), Err(-22));
        assert_eq!(heap_create_preflight(0, false, 40, -5, -22), Err(-5));
        assert_eq!(heap_create_preflight(40, true, 40, -5, -22), Ok(()));
    }

    #[test]
    fn ipc_bounds_use_each_nonzero_loader_override() {
        let linker = (0x933e_0000, 0x9340_0000);
        assert_eq!(
            default_buffer_bounds(linker, (0x933d_0000, 0)),
            (0x933d_0000, 0x9340_0000)
        );
        assert_eq!(
            default_buffer_bounds(linker, (0, 0x9341_0000)),
            (0x933e_0000, 0x9341_0000)
        );
        assert_eq!(default_buffer_bounds(linker, (0, 0)), linker);
    }

    #[test]
    fn vector_layout_checks_counts_and_aligned_storage() {
        assert_eq!(vector_layout(1, 1, 2, 8, 0x1000), Some((2, 16)));
        assert_eq!(vector_layout(2, 1, 2, 8, 0x1000), None);
        assert_eq!(vector_layout(usize::MAX, 1, 4, 8, 0x1000), None);
        assert_eq!(vector_layout(1, 0, 1, 8, 0x1004), None);
        assert_eq!(vector_layout(0, 0, 4, 8, 0x1000), None);
    }

    #[test]
    fn buffer_validation_keeps_protocol_addresses_for_empty_vectors() {
        assert!(buffer_valid(0xdead_beef, 0));
        assert!(buffer_valid(0x8000_1000, 32));
        assert!(!buffer_valid(0x8000_1001, 32));
        assert!(buffer_valid(0x8000_1000, 31));
        assert!(buffer_valid(0x9000_1000, 1));
        assert!(!buffer_valid(usize::MAX & !31, 32));
    }

    #[test]
    fn request_address_preserves_empty_vector_values_and_maps_dma_buffers() {
        assert_eq!(request_address(0xdead_beef, 0), 0xdead_beef);
        assert_eq!(request_address(0xc000_1000, 32), 0x0000_1000);
        assert_eq!(request_address(0x8000_1000, 32), 0x0000_1000);
    }
}
/// Submission failure is distinct from a completed IOS request with an error.
pub(crate) fn submission_result(queued: i32, reply: i32) -> Result<i32, i32> {
    if queued == 0 { Ok(reply) } else { Err(queued) }
}

#[cfg(test)]
mod submission_tests {
    #[test]
    fn final_reply_and_transport_errors_keep_their_code_spaces() {
        assert_eq!(super::submission_result(-6, 0), Err(-6));
        assert_eq!(super::submission_result(0, -6), Ok(-6));
        assert_eq!(super::submission_result(0, 13), Ok(13));
    }
}
