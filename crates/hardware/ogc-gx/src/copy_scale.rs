pub(crate) fn num_xfb_lines(efb_height: u16, mut yscale: u32) -> u32 {
    if yscale == 0 {
        return 1024;
    }
    let mut lines = (((u32::from(efb_height.wrapping_sub(1)) << 8) / yscale) + 1).min(1024);
    if yscale > 128 && yscale < 256 {
        while yscale & 1 != 0 {
            yscale >>= 1;
        }
        if yscale != 0 && u32::from(efb_height) % yscale == 0 {
            lines = (lines + 1).min(1024);
        }
    }
    lines
}

pub(crate) fn y_scale_factor(efb_height: u16, xfb_height: u16) -> Option<f32> {
    if efb_height == 0 || xfb_height == 0 || efb_height > 1024 || xfb_height > 1024 {
        return None;
    }
    let mut ratio = f32::from(efb_height) / f32::from(xfb_height);
    let mut yscale = ((256.0 / ratio) as u32) & 0x1ff;
    let mut count = u32::from(xfb_height);
    let mut lines = num_xfb_lines(efb_height, yscale);
    while lines >= u32::from(xfb_height) {
        if count == 0 {
            return None;
        }
        ratio = count as f32 / f32::from(efb_height);
        count -= 1;
        yscale = ((256.0 / ratio) as u32) & 0x1ff;
        lines = num_xfb_lines(efb_height, yscale);
    }
    while lines < u32::from(xfb_height) {
        if count > u32::from(u16::MAX) {
            return None;
        }
        ratio = count as f32 / f32::from(efb_height);
        count += 1;
        yscale = ((256.0 / ratio) as u32) & 0x1ff;
        lines = num_xfb_lines(efb_height, yscale);
    }
    Some(ratio)
}

pub(crate) fn copy_y_scale(efb_height: u16, xfb_height: u16) -> Option<u16> {
    let ratio = y_scale_factor(efb_height, xfb_height)?;
    Some((((256.0 / ratio) as u32) & 0x1ff) as u16)
}

pub(crate) fn fits_framebuffer(efb_height: u16, xfb_height: u16) -> bool {
    copy_y_scale(efb_height, xfb_height)
        .is_some_and(|scale| num_xfb_lines(efb_height, u32::from(scale)) <= u32::from(xfb_height))
}

pub(crate) fn scale_enable_bit(scale: u32) -> u32 {
    u32::from(scale != 256) << 10
}
