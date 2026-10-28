use core::cmp::{max, min};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, x: u16, y: u16) -> bool {
        let right = u32::from(self.x) + u32::from(self.width);
        let bottom = u32::from(self.y) + u32::from(self.height);
        u32::from(x) >= u32::from(self.x)
            && u32::from(y) >= u32::from(self.y)
            && u32::from(x) < right
            && u32::from(y) < bottom
    }

    pub(crate) fn intersection(self, other: Self) -> Option<Self> {
        let left = max(u32::from(self.x), u32::from(other.x));
        let top = max(u32::from(self.y), u32::from(other.y));
        let right = min(
            u32::from(self.x) + u32::from(self.width),
            u32::from(other.x) + u32::from(other.width),
        );
        let bottom = min(
            u32::from(self.y) + u32::from(self.height),
            u32::from(other.y) + u32::from(other.height),
        );
        (right > left && bottom > top).then(|| {
            Self::new(
                left as u16,
                top as u16,
                (right - left) as u16,
                (bottom - top) as u16,
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStyle {
    pub color: Color,
    /// Pixel scale for the built-in 5 by 7 bitmap font. Zero is invalid.
    pub scale: u8,
    pub align: TextAlign,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::rgb(255, 255, 255),
            scale: 2,
            align: TextAlign::Center,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasError {
    InvalidGeometry,
    Backend(i32),
}

/// A drawing target for retained widgets and immediate custom widgets.
///
/// `rect` and `clip` use top-left screen coordinates. Implementations must
/// draw only inside the intersection of these rectangles.
pub trait Canvas {
    fn fill_rect(&mut self, rect: Rect, color: Color, clip: Rect) -> Result<(), CanvasError>;

    fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        style: TextStyle,
        clip: Rect,
    ) -> Result<(), CanvasError>;
}

pub(crate) struct ClippedCanvas<'a> {
    inner: &'a mut dyn Canvas,
    clip: Rect,
}

impl<'a> ClippedCanvas<'a> {
    pub(crate) fn new(inner: &'a mut dyn Canvas, clip: Rect) -> Self {
        Self { inner, clip }
    }
}

impl Canvas for ClippedCanvas<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color, _clip: Rect) -> Result<(), CanvasError> {
        if let Some(rect) = rect.intersection(self.clip) {
            self.inner.fill_rect(rect, color, self.clip)
        } else {
            Ok(())
        }
    }

    fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        style: TextStyle,
        _clip: Rect,
    ) -> Result<(), CanvasError> {
        if let Some(clip) = rect.intersection(self.clip) {
            self.inner.draw_text(text, rect, style, clip)
        } else {
            Ok(())
        }
    }
}
