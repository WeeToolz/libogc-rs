//! Copy an XFB with aligned word accesses, including uncached Wii memory.

/// Both pointers must be aligned to four bytes, valid for `bytes`, and disjoint.
/// `bytes` must be a multiple of four. The caller must own the destination.
pub(crate) unsafe fn copy_words(source: *const u32, destination: *mut u32, bytes: usize) {
    assert_eq!(bytes % 4, 0);
    for index in 0..bytes / 4 {
        // Byte stores to uncached XFB memory do not preserve the other bytes
        // of a pixel pair on hardware. Volatile word accesses also prevent
        // LLVM from replacing this loop with the byte-based memcpy routine.
        unsafe {
            destination
                .add(index)
                .write_volatile(source.add(index).read_volatile())
        };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn snapshot_preserves_pixel_pairs_and_source() {
        let source = [0x1080_1080u32, 0xeb80_eb80, 0x3170_31b1, 0x5160_5157];
        let mut destination = [0u32; 4];
        unsafe { super::copy_words(source.as_ptr(), destination.as_mut_ptr(), 16) };
        assert_eq!(
            destination,
            [0x1080_1080, 0xeb80_eb80, 0x3170_31b1, 0x5160_5157]
        );
        assert_eq!(source, [0x1080_1080, 0xeb80_eb80, 0x3170_31b1, 0x5160_5157]);
    }
}
