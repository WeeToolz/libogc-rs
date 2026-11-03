//! Bounds and command type values for the Wii SDIO host controller.

const SDIO_PAGE_BYTES: usize = 4096;
pub(crate) const SDIOCMD_TYPE_NODATA: u32 = 0;
pub(super) const SDIOCMD_TYPE_READ: u32 = 1;
pub(super) const SDIOCMD_TYPE_TRANSFER: u32 = 3;
pub(crate) const SDIO_READ_BUFFER_ALIGNMENT: usize = 64;

pub(crate) fn transfer_length(block_count: u32, block_size: u32) -> Option<usize> {
    block_count
        .checked_mul(block_size)
        .map(|length| length as usize)
}

/// Return whether one read transfer stays within a single 4 KiB page.
///
/// Empty transfers have no DMA range. Other commands do not use this
/// IOS path restriction. A non-empty read transfer needs a non-null Wii
/// address, a representable end address, and a length no larger than one page.
pub(crate) fn read_range_valid(cmd_type: u32, address: usize, length: usize) -> bool {
    if cmd_type != SDIOCMD_TYPE_READ || length == 0 {
        return true;
    }
    if address == 0
        || u32::try_from(address).is_err()
        || length > SDIO_READ_BUFFER_ALIGNMENT
        || !address.is_multiple_of(SDIO_READ_BUFFER_ALIGNMENT)
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
    use super::{SDIO_READ_BUFFER_ALIGNMENT, read_range_valid, transfer_length};

    const READ: u32 = super::SDIOCMD_TYPE_READ;
    const WRITE: u32 = 2;
    const TRANSFER: u32 = super::SDIOCMD_TYPE_TRANSFER;

    #[test]
    fn command_types_match_the_libogc_sdio_contract() {
        assert_eq!(super::SDIOCMD_TYPE_NODATA, 0);
        assert_eq!(READ, 1);
        assert_eq!(WRITE, 2);
        assert_eq!(TRANSFER, 3);
    }

    #[test]
    fn accepts_aligned_read_ranges_at_page_edges() {
        assert!(read_range_valid(READ, 0x8000_0000, 64));
        assert!(read_range_valid(READ, 0x8000_0fc0, 64));
        assert!(read_range_valid(
            READ,
            0x8000_0000,
            SDIO_READ_BUFFER_ALIGNMENT
        ));
    }

    #[test]
    fn rejects_a_read_transfer_that_crosses_a_page() {
        assert!(!read_range_valid(READ, 0x8000_0fe0, 64));
        assert!(!read_range_valid(READ, 0x8000_0fff, 2));
        assert!(!read_range_valid(READ, 0x8000_0001, 64));
        assert!(!read_range_valid(READ, 0x8000_0000, 65));
    }

    #[test]
    fn rejects_null_oversized_and_wrapping_read_ranges() {
        assert!(!read_range_valid(READ, 0, 64));
        assert!(!read_range_valid(READ, 0x8000_0000, 4097));
        assert!(!read_range_valid(READ, u32::MAX as usize, 2));
        assert!(!read_range_valid(READ, u32::MAX as usize - 15, 32));
    }

    #[test]
    fn rejects_transfer_length_overflow_before_pointer_access() {
        assert_eq!(transfer_length(1, 64), Some(64));
        assert_eq!(transfer_length(u32::MAX, 1), Some(u32::MAX as usize));
        assert_eq!(transfer_length(u32::MAX, 2), None);
    }

    #[test]
    fn empty_and_other_commands_keep_their_existing_range_rules() {
        assert!(read_range_valid(READ, 0, 0));
        for cmd_type in [super::SDIOCMD_TYPE_NODATA, WRITE, TRANSFER] {
            assert!(read_range_valid(cmd_type, 0, 8192));
            assert!(read_range_valid(cmd_type, 0x8000_0fe0, 64));
        }
    }
}
