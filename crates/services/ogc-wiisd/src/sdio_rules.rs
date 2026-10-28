//! Bounds for the Wii SDIO host controller's broadcast-command DMA path.

const SDIO_PAGE_BYTES: usize = 4096;
pub(super) const SDIOCMD_TYPE_BC: u32 = 1;
pub(crate) const SDIO_BC_BUFFER_ALIGNMENT: usize = 64;

pub(crate) fn transfer_length(block_count: u32, block_size: u32) -> Option<usize> {
    block_count
        .checked_mul(block_size)
        .map(|length| length as usize)
}

/// Return whether one broadcast-command transfer stays within a single 4 KiB page.
///
/// Empty transfers have no DMA range. Non-broadcast commands do not use this
/// IOS path restriction. A non-empty broadcast transfer needs a non-null Wii
/// address, a representable end address, and a length no larger than one page.
pub(crate) fn broadcast_range_valid(cmd_type: u32, address: usize, length: usize) -> bool {
    if cmd_type != SDIOCMD_TYPE_BC || length == 0 {
        return true;
    }
    if address == 0
        || u32::try_from(address).is_err()
        || length > SDIO_BC_BUFFER_ALIGNMENT
        || !address.is_multiple_of(SDIO_BC_BUFFER_ALIGNMENT)
        || length > SDIO_PAGE_BYTES
    {
        return false;
    }

    let Some(last_address) = address.checked_add(length - 1) else {
        return false;
    };
    u32::try_from(last_address).is_ok()
        && address / SDIO_PAGE_BYTES == last_address / SDIO_PAGE_BYTES
}

#[cfg(test)]
mod tests {
    use super::{SDIO_BC_BUFFER_ALIGNMENT, broadcast_range_valid, transfer_length};

    const BROADCAST: u32 = super::SDIOCMD_TYPE_BC;
    const APPLICATION: u32 = 3;

    #[test]
    fn accepts_aligned_switch_function_ranges_at_page_edges() {
        assert!(broadcast_range_valid(BROADCAST, 0x8000_0000, 64));
        assert!(broadcast_range_valid(BROADCAST, 0x8000_0fc0, 64));
        assert!(broadcast_range_valid(
            BROADCAST,
            0x8000_0000,
            SDIO_BC_BUFFER_ALIGNMENT
        ));
    }

    #[test]
    fn rejects_a_broadcast_transfer_that_crosses_a_page() {
        assert!(!broadcast_range_valid(BROADCAST, 0x8000_0fe0, 64));
        assert!(!broadcast_range_valid(BROADCAST, 0x8000_0fff, 2));
        assert!(!broadcast_range_valid(BROADCAST, 0x8000_0001, 64));
        assert!(!broadcast_range_valid(BROADCAST, 0x8000_0000, 65));
    }

    #[test]
    fn rejects_null_oversized_and_wrapping_broadcast_ranges() {
        assert!(!broadcast_range_valid(BROADCAST, 0, 64));
        assert!(!broadcast_range_valid(BROADCAST, 0x8000_0000, 4097));
        assert!(!broadcast_range_valid(BROADCAST, u32::MAX as usize, 2));
        assert!(!broadcast_range_valid(
            BROADCAST,
            u32::MAX as usize - 15,
            32
        ));
    }

    #[test]
    fn rejects_transfer_length_overflow_before_pointer_access() {
        assert_eq!(transfer_length(1, 64), Some(64));
        assert_eq!(transfer_length(u32::MAX, 1), Some(u32::MAX as usize));
        assert_eq!(transfer_length(u32::MAX, 2), None);
    }

    #[test]
    fn empty_and_non_broadcast_commands_keep_their_existing_range_rules() {
        assert!(broadcast_range_valid(BROADCAST, 0, 0));
        assert!(broadcast_range_valid(APPLICATION, 0, 8192));
        assert!(broadcast_range_valid(APPLICATION, 0x8000_0fe0, 64));
    }
}
