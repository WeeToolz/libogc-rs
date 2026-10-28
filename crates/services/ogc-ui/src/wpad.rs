//! Wii Remote, Nunchuk, and Classic Controller input adapter.

use crate::{Direction, UiError, UiInput};

/// Converts one WPAD sample to toolkit input.
///
/// Create one adapter for the selected controller channel and pass it the
/// current `WpadData` sample on each UI update. The caller selects which
/// controller owns the UI when multiple controllers are connected.
#[derive(Clone, Copy, Debug)]
pub struct WpadInputAdapter {
    width: u16,
    height: u16,
}

impl WpadInputAdapter {
    pub fn new(width: u16, height: u16) -> Result<Self, UiError> {
        if width == 0 || height == 0 {
            return Err(UiError::InvalidViewport);
        }
        Ok(Self { width, height })
    }

    /// Map the current press-edge masks and valid IR sample.
    ///
    /// `btns_d` is supplied by libogc as a button-down edge mask. When the
    /// controller reports an error, this returns an empty snapshot so held
    /// state from a lost connection cannot trigger a delayed action.
    pub fn update(&self, sample: &wpad_sys::WpadData) -> UiInput {
        if sample.err != wpad_sys::WPAD_ERR_NONE {
            return UiInput::default();
        }

        let buttons = sample.btns_d;
        let direction = if contains_any(
            buttons,
            wpad_sys::WPAD_BUTTON_UP | wpad_sys::WPAD_CLASSIC_BUTTON_UP,
        ) {
            Some(Direction::Up)
        } else if contains_any(
            buttons,
            wpad_sys::WPAD_BUTTON_DOWN | wpad_sys::WPAD_CLASSIC_BUTTON_DOWN,
        ) {
            Some(Direction::Down)
        } else if contains_any(
            buttons,
            wpad_sys::WPAD_BUTTON_LEFT | wpad_sys::WPAD_CLASSIC_BUTTON_LEFT,
        ) {
            Some(Direction::Left)
        } else if contains_any(
            buttons,
            wpad_sys::WPAD_BUTTON_RIGHT | wpad_sys::WPAD_CLASSIC_BUTTON_RIGHT,
        ) {
            Some(Direction::Right)
        } else {
            None
        };

        let activate = contains_any(
            buttons,
            wpad_sys::WPAD_BUTTON_A
                | wpad_sys::WPAD_BUTTON_2
                | wpad_sys::WPAD_NUNCHUK_BUTTON_C
                | wpad_sys::WPAD_CLASSIC_BUTTON_A,
        );
        let cancel = contains_any(
            buttons,
            wpad_sys::WPAD_BUTTON_B
                | wpad_sys::WPAD_NUNCHUK_BUTTON_Z
                | wpad_sys::WPAD_CLASSIC_BUTTON_B
                | wpad_sys::WPAD_CLASSIC_BUTTON_ZL,
        );
        let pointer = (sample.data_present & wpad_sys::WPAD_DATA_IR != 0 && sample.ir.valid != 0)
            .then(|| self.pointer_position(sample.ir.x, sample.ir.y, sample.ir.vres))
            .flatten();

        UiInput {
            direction,
            activate,
            cancel,
            pointer,
            pointer_activate: activate && pointer.is_some(),
        }
    }

    fn pointer_position(&self, x: f32, y: f32, resolution: [u32; 2]) -> Option<(u16, u16)> {
        Some((
            scale_coordinate(x, resolution[0], self.width)?,
            scale_coordinate(y, resolution[1], self.height)?,
        ))
    }
}

fn contains_any(buttons: u32, mask: u32) -> bool {
    buttons & mask != 0
}

fn scale_coordinate(position: f32, resolution: u32, extent: u16) -> Option<u16> {
    if !position.is_finite() || position < 0.0 || resolution == 0 || extent == 0 {
        return None;
    }
    let scaled = position * f32::from(extent) / resolution as f32;
    // The value is nonnegative, so the float-to-integer conversion truncates
    // toward zero, which gives the required pixel floor without libm.
    Some(scaled.min(f32::from(extent - 1)) as u16)
}
