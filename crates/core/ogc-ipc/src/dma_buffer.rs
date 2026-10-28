//! IOS transfers must not invalidate cache lines shared with caller data.

pub const CACHE_LINE_BYTES: usize = 32;

pub fn needs_staging(address: usize, length: usize) -> bool {
    length != 0
        && (!address.is_multiple_of(CACHE_LINE_BYTES) || !length.is_multiple_of(CACHE_LINE_BYTES))
}

/// Reserve complete cache lines, including the final partial line. Aligning
/// only the pointer lets allocator metadata share that final DMA cache line.
pub fn staging_length(length: usize) -> Option<usize> {
    if length == 0 {
        return None;
    }
    length
        .checked_add(CACHE_LINE_BYTES - 1)
        .map(|bytes| bytes & !(CACHE_LINE_BYTES - 1))
        .filter(|&bytes| bytes <= isize::MAX as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_reserves_the_entire_last_cache_line() {
        assert_eq!(staging_length(1), Some(32));
        assert_eq!(staging_length(5), Some(32));
        assert_eq!(staging_length(32), Some(32));
        assert_eq!(staging_length(36), Some(64));
        assert_eq!(staging_length(0), None);
        assert_eq!(staging_length(usize::MAX), None);
        assert_eq!(staging_length(isize::MAX as usize), None);
    }

    #[test]
    fn small_ios_output_preserves_a_dirty_neighbor_in_the_same_cache_line() {
        // Model RAM and a dirty CPU cache line. Invalidating a direct 8-byte
        // output would reload the entire line and lose the updated neighbor.
        let mut ram = [0u8; CACHE_LINE_BYTES];
        let mut cache = ram;
        cache[16] = 0xa5;
        let reply = 0x0001_0001_4842_4345u64.to_be_bytes();
        if needs_staging(0x8000_1000, reply.len()) {
            let mut staging_ram = [0u8; CACHE_LINE_BYTES];
            staging_ram[..reply.len()].copy_from_slice(&reply);
            cache[..reply.len()].copy_from_slice(&staging_ram[..reply.len()]);
        } else {
            ram[..reply.len()].copy_from_slice(&reply);
            cache = ram;
        }
        assert_eq!(&cache[..8], &reply);
        assert_eq!(cache[16], 0xa5);
    }

    #[test]
    fn only_complete_exclusive_cache_lines_can_be_transferred_directly() {
        assert!(!needs_staging(0x8000_1000, 0));
        assert!(!needs_staging(0x8000_1000, 64));
        assert!(needs_staging(0x8000_1004, 64));
        assert!(needs_staging(0x8000_1000, 30));
    }
}
