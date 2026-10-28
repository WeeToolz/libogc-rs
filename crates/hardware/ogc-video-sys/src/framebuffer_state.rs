pub(super) fn framebuffer_to_latch(flushed: bool, next: usize, current: usize) -> Option<usize> {
    (flushed && next != current).then_some(next)
}

pub(super) fn framebuffer_to_publish(
    flushed: bool,
    configured: usize,
    current_next: usize,
) -> Option<usize> {
    (flushed && configured != current_next).then_some(configured)
}

pub(super) const fn can_apply_flushed_mode_change(
    mode_change: bool,
    current_field_even_odd: bool,
) -> bool {
    !mode_change || current_field_even_odd
}

pub(super) const fn next_field_after_current(
    current_field_even_odd: bool,
    adjusted_y_origin: u16,
) -> u32 {
    (current_field_even_odd as u32) ^ 1 ^ ((adjusted_y_origin & 1 != 0) as u32)
}

pub(super) const fn adjusted_display_y_origin(vi_tv_mode: u32, y_origin: u16) -> u16 {
    if vi_tv_mode & 3 == 1 {
        y_origin.wrapping_mul(2)
    } else {
        y_origin
    }
}

pub(super) const fn line_from_half_line(half_line: u32, half_lines_per_field: u32) -> u32 {
    let field_line = if half_line >= half_lines_per_field {
        half_line - half_lines_per_field
    } else {
        half_line
    };
    field_line >> 1
}

pub(super) const fn television_standard(vi_tv_mode: u32) -> u32 {
    match (vi_tv_mode >> 2) & 0x07 {
        0 | 3 => 0,
        2 => 2,
        5 => 5,
        _ => 1,
    }
}

pub(super) fn right_framebuffer_to_latch(
    flushed: bool,
    scan_mode: u32,
    next: usize,
    current: usize,
) -> Option<usize> {
    (flushed && scan_mode == 3 && next != current).then_some(next)
}
