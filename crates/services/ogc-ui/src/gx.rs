use crate::{Canvas, CanvasError, Color, Rect, TextAlign, TextStyle, UiError};

/// Solid-color and bitmap-text canvas backed by the real GX FIFO.
pub struct GxCanvas {
    width: u16,
    height: u16,
}

impl GxCanvas {
    /// Create a canvas after GX FIFO and frame setup are complete.
    ///
    /// # Safety
    /// The caller must initialize GX and retain exclusive control of GX state
    /// while this canvas is used. Call the application's GX present operation
    /// after drawing the UI.
    pub unsafe fn new(width: u16, height: u16) -> Result<Self, UiError> {
        if width == 0 || height == 0 || width > 1024 || height > 1024 {
            return Err(UiError::InvalidViewport);
        }
        let w = f32::from(width);
        let h = f32::from(height);
        let projection = [
            [2.0 / w, 0.0, 0.0, -1.0],
            [0.0, -2.0 / h, 0.0, 1.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        unsafe { ogc_gx::configure_solid_2d(&projection) };
        Ok(Self { width, height })
    }

    /// Create a canvas from the active libogc render mode.
    ///
    /// # Safety
    /// The caller must satisfy the safety requirements of [`Self::new`]. The
    /// mode must be the one used by the active EFB and GX frame setup.
    pub unsafe fn from_video_mode(mode: &ogc_video::GxrModeObj) -> Result<Self, UiError> {
        unsafe { Self::new(mode.fb_width, mode.efb_height) }
    }

    pub const fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

impl Canvas for GxCanvas {
    fn fill_rect(&mut self, rect: Rect, color: Color, clip: Rect) -> Result<(), CanvasError> {
        let Some(rect) = rect.intersection(clip) else {
            return Ok(());
        };
        if rect.width == 0 || rect.height == 0 {
            return Ok(());
        }
        unsafe {
            ogc_gx::draw_solid_rect(
                f32::from(rect.x),
                f32::from(rect.y),
                f32::from(rect.width),
                f32::from(rect.height),
                0.0,
                ogc_gx::GxColor {
                    r: color.red,
                    g: color.green,
                    b: color.blue,
                    a: color.alpha,
                },
            )
        }
        .map_err(|_| CanvasError::InvalidGeometry)
    }

    fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        style: TextStyle,
        clip: Rect,
    ) -> Result<(), CanvasError> {
        if style.scale == 0 {
            return Err(CanvasError::InvalidGeometry);
        }
        let Some(clip) = rect.intersection(clip) else {
            return Ok(());
        };
        let scale = u32::from(style.scale);
        let advance = 6 * scale;
        let line_height = 8 * scale;
        let line_count = text.lines().count() as u32;
        let block_height = line_count.saturating_mul(line_height);
        let first_line_y = u32::from(rect.y)
            .saturating_add(u32::from(rect.height).saturating_sub(block_height) / 2);
        for (line_index, line) in text.lines().enumerate() {
            let line_width = (line.chars().count() as u32).saturating_mul(advance);
            let start_x = match style.align {
                TextAlign::Start => u32::from(rect.x),
                TextAlign::Center => u32::from(rect.x)
                    .saturating_add(u32::from(rect.width).saturating_sub(line_width) / 2),
                TextAlign::End => u32::from(rect.x)
                    .saturating_add(u32::from(rect.width).saturating_sub(line_width)),
            };
            let line_y = first_line_y.saturating_add(line_index as u32 * line_height);
            for (character_index, character) in line.chars().enumerate() {
                let glyph_x = start_x.saturating_add(character_index as u32 * advance);
                for (row_index, bits) in crate::font::glyph_rows(character).into_iter().enumerate()
                {
                    let mut column = 0u32;
                    while column < 5 {
                        if bits & (1 << (4 - column)) == 0 {
                            column += 1;
                            continue;
                        }
                        let run_start = column;
                        while column < 5 && bits & (1 << (4 - column)) != 0 {
                            column += 1;
                        }
                        let px = glyph_x.saturating_add(run_start * scale);
                        let py = line_y.saturating_add(row_index as u32 * scale);
                        let run = Rect::new(
                            px.min(u32::from(u16::MAX)) as u16,
                            py.min(u32::from(u16::MAX)) as u16,
                            ((column - run_start) * scale).min(u32::from(u16::MAX)) as u16,
                            u16::from(style.scale),
                        );
                        self.fill_rect(run, style.color, clip)?;
                    }
                }
            }
        }
        Ok(())
    }
}
