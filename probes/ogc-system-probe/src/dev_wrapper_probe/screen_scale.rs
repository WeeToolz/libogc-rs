//! Double a YUYV text image in place, without allocation or display callbacks.
pub fn double_text(
    pixels: &mut [u32],
    stride: usize,
    width: usize,
    height: usize,
) -> Result<(), i32> {
    if width == 0 || width % 2 != 0 || height == 0 || height % 2 != 0 || stride < width {
        return Err(-22);
    }
    let required = (height - 1)
        .checked_mul(stride)
        .and_then(|n| n.checked_add(width))
        .ok_or(-22)?;
    if pixels.len() < required {
        return Err(-22);
    }
    // Work backwards so writes cannot replace unread source pixels.
    for y in (0..height / 2).rev() {
        for x in (0..width / 2).rev() {
            let pair = pixels[y * stride + x];
            let chroma = pair & 0x00ff00ff;
            let left = chroma | (pair & 0xff000000) | ((pair >> 16) & 0x0000ff00);
            let right = chroma | (pair & 0x0000ff00) | ((pair << 16) & 0xff000000);
            for row in [2 * y, 2 * y + 1] {
                pixels[row * stride + 2 * x] = left;
                pixels[row * stride + 2 * x + 1] = right;
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doubles_pixels_and_rows_with_shared_chroma() {
        let mut pixels = [0x10802080, 0, 99, 0x30804080, 0, 99, 0, 0, 99, 0, 0, 99];
        double_text(&mut pixels, 3, 2, 4).unwrap();
        assert_eq!(
            pixels,
            [
                0x10801080, 0x20802080, 99, 0x10801080, 0x20802080, 99, 0x30803080, 0x40804080, 99,
                0x30803080, 0x40804080, 99
            ]
        );
    }
    #[test]
    fn rejects_invalid_bounds_without_writes() {
        for (stride, width, height) in [
            (1, 2, 2),
            (2, 3, 2),
            (2, 2, 3),
            (2, 2, 0),
            (usize::MAX, 2, 4),
            (2, 2, 4),
        ] {
            let mut pixels = [0x12345678; 4];
            assert_eq!(double_text(&mut pixels, stride, width, height), Err(-22));
            assert_eq!(pixels, [0x12345678; 4]);
        }
    }
}
