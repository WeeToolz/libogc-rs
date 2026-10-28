//! Host tests for the GX copy-scale engine used by production.
#[cfg(test)]
#[path = "../../../crates/hardware/ogc-gx/src/copy_scale.rs"]
mod copy_scale;
#[cfg(test)]
#[path = "../../../crates/hardware/ogc-gx/src/fifo_rules.rs"]
mod fifo_rules;

#[cfg(test)]
mod tests {
    use super::copy_scale::{copy_y_scale, fits_framebuffer, num_xfb_lines, y_scale_factor};

    #[test]
    fn impossible_scale_returns_without_looping() {
        assert_eq!(copy_y_scale(480, 240), None);
        assert_eq!(y_scale_factor(0, 480), None);
        assert_eq!(num_xfb_lines(480, 256), 480);
        assert_eq!(num_xfb_lines(480, 255), 481);
    }

    #[test]
    fn valid_scale_matches_reference_register_values() {
        assert_eq!(copy_y_scale(480, 480), Some(256));
        assert_eq!(copy_y_scale(240, 480), Some(127));
        assert_eq!(num_xfb_lines(240, 127), 482);
        assert!(!fits_framebuffer(240, 480));
        assert!(fits_framebuffer(480, 480));
        assert_eq!(copy_y_scale(480, 360), Some(341));
        assert_eq!(y_scale_factor(480, 360), Some(0.75));
    }

    #[test]
    fn out_of_range_dimensions_are_rejected() {
        assert_eq!(copy_y_scale(1025, 480), None);
        assert_eq!(copy_y_scale(480, 1025), None);
    }

    #[test]
    fn scale_enable_matches_wrapping_c_arithmetic() {
        assert_eq!(super::copy_scale::scale_enable_bit(128), 0x400);
        assert_eq!(super::copy_scale::scale_enable_bit(256), 0);
        assert_eq!(super::copy_scale::scale_enable_bit(341), 0x400);
    }

    #[test]
    fn fifo_wrapped_distance_keeps_both_addresses_in_the_buffer() {
        use super::fifo_rules::fifo_pointers;
        assert_eq!(fifo_pointers(0x8000_0000, 0x10000, 0x8000_f000, 0x8000_1000), Some((0x8000_f000, 0x8000_1000, 0x2000)));
        assert_eq!(fifo_pointers(0x8000_0000, 0x10000, 0x8000_1000, 0x8000_f000), Some((0x8000_1000, 0x8000_f000, 0xe000)));
        assert_eq!(fifo_pointers(0x8000_0000, 0x10000, 0x8001_0000, 0x8000_1000), None);
    }
}
