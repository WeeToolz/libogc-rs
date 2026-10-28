//! Select the owned XFB that VI and the console both use.

/// Return which owned XFB VI scans when it also matches the console target.
pub(crate) fn displayed_buffer_index(
    current: usize,
    console_target: usize,
    framebuffer_a: usize,
    framebuffer_b: usize,
) -> Option<usize> {
    if current == 0 || current != console_target {
        return None;
    }
    if current == framebuffer_a {
        Some(0)
    } else if current == framebuffer_b {
        Some(1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn displayed_xfb_must_match_vi_and_console_targets() {
        assert_eq!(
            super::displayed_buffer_index(0xc000_1000, 0xc000_1000, 0xc000_1000, 0xc000_2000),
            Some(0)
        );
        assert_eq!(
            super::displayed_buffer_index(0xc000_2000, 0xc000_2000, 0xc000_1000, 0xc000_2000),
            Some(1)
        );
        assert_eq!(
            super::displayed_buffer_index(0xc000_1000, 0xc000_2000, 0xc000_1000, 0xc000_2000),
            None
        );
        assert_eq!(
            super::displayed_buffer_index(0xc000_3000, 0xc000_3000, 0xc000_1000, 0xc000_2000),
            None
        );
    }
}
