pub(crate) const CAPACITY: usize = 128 * 1024;

pub(crate) fn silence_unfilled(output: &mut [u8], filled: usize) {
    let filled = filled.min(output.len());
    output[filled..].fill(0);
}

#[cfg(test)]
mod tests {
    use super::{CAPACITY, silence_unfilled};

    #[test]
    fn output_ring_holds_more_than_half_a_second_of_audio() {
        const STEREO_16BIT_48KHZ_BYTES_PER_SECOND: usize = 48_000 * 2 * 2;

        assert!(CAPACITY * 1_000 / STEREO_16BIT_48KHZ_BYTES_PER_SECOND >= 600);
        assert_eq!(CAPACITY * 1_000 / STEREO_16BIT_48KHZ_BYTES_PER_SECOND, 682);
    }

    #[test]
    fn dma_buffer_tail_is_silence_after_a_short_ring_read() {
        let mut output = [0xA5; 8];

        silence_unfilled(&mut output, 3);

        assert_eq!(output, [0xA5, 0xA5, 0xA5, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn empty_ring_clears_the_full_dma_buffer() {
        let mut output = [0xA5; 8];

        silence_unfilled(&mut output, 0);

        assert_eq!(output, [0; 8]);
    }
}
