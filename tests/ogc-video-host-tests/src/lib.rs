#[cfg(test)]
#[path = "../../../crates/hardware/ogc-video-sys/src/framebuffer_state.rs"]
mod framebuffer_state;

#[cfg(test)]
mod tests {
    use super::framebuffer_state::{
        adjusted_display_y_origin, can_apply_flushed_mode_change, framebuffer_to_latch,
        framebuffer_to_publish, line_from_half_line, next_field_after_current,
        right_framebuffer_to_latch, television_standard,
    };

    #[test]
    fn framebuffer_getters_advance_at_flush_and_retrace() {
        let current = 0xc000_1000;
        let configured = 0xc000_2000;

        assert_eq!(framebuffer_to_publish(false, configured, current), None);
        assert_eq!(
            framebuffer_to_publish(true, configured, current),
            Some(configured)
        );
        assert_eq!(framebuffer_to_latch(false, configured, current), None);
        assert_eq!(
            framebuffer_to_latch(true, configured, current),
            Some(configured)
        );
        assert_eq!(framebuffer_to_latch(true, 0, current), Some(0));
    }

    #[test]
    fn right_framebuffer_waits_for_flushed_progressive_3d_mode() {
        assert_eq!(
            right_framebuffer_to_latch(true, 3, 0xc000_2000, 0xc000_1000),
            Some(0xc000_2000)
        );
        assert_eq!(right_framebuffer_to_latch(false, 3, 0xc000_2000, 0), None);
        assert_eq!(right_framebuffer_to_latch(true, 1, 0xc000_2000, 0), None);
        assert_eq!(right_framebuffer_to_latch(true, 3, 0, 0xc000_2000), Some(0));
    }

    #[test]
    fn scan_mode_change_waits_for_the_c_field_boundary() {
        assert!(!can_apply_flushed_mode_change(true, false));
        assert!(can_apply_flushed_mode_change(true, true));
        assert!(can_apply_flushed_mode_change(false, false));
    }

    #[test]
    fn next_field_accounts_for_current_field_and_y_origin_parity() {
        assert_eq!(next_field_after_current(true, 0), 0);
        assert_eq!(next_field_after_current(false, 0), 1);
        assert_eq!(next_field_after_current(true, 1), 1);
        assert_eq!(next_field_after_current(false, 1), 0);
        assert_eq!(adjusted_display_y_origin(0, 3), 3);
        assert_eq!(adjusted_display_y_origin(1, 3), 6);
    }

    #[test]
    fn current_line_wraps_at_the_half_line_field_boundary() {
        assert_eq!(line_from_half_line(100, 525), 50);
        assert_eq!(line_from_half_line(575, 525), 25);
    }

    #[test]
    fn current_tv_mode_returns_the_video_standard() {
        assert_eq!(television_standard(0), 0);
        assert_eq!(television_standard(4), 1);
        assert_eq!(television_standard(8), 2);
        assert_eq!(television_standard(12), 0);
        assert_eq!(television_standard(20), 5);
        assert_eq!(television_standard(24), 1);
    }
}
