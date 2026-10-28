#![no_std]
//! Retained-mode widgets and layout for Wii and GameCube applications.
//!
//! `Ui` owns widget state. Applications send input snapshots to `update` and
//! draw the retained tree through a `Canvas`. Custom widgets can use the same
//! canvas for immediate drawing inside their assigned bounds.

extern crate alloc;

mod canvas;
#[cfg(feature = "controller-input")]
pub mod controller_input;
mod repeat;
mod tree;

pub mod font;
pub use font::glyph_rows;

#[cfg(all(feature = "gx-backend", target_arch = "powerpc"))]
pub mod gx;

#[cfg(all(feature = "pad-input", target_arch = "powerpc"))]
pub mod pad;
#[cfg(all(feature = "wpad-input", target_arch = "powerpc"))]
pub mod wpad;

pub use canvas::{Canvas, CanvasError, Color, Rect, TextAlign, TextStyle};
#[cfg(feature = "controller-input")]
pub use controller_input::{
    ControllerInputAdapter, ControllerInputBindings, ControllerInputConfig, ControllerInputFrame,
    ControllerInputLabels, ControllerInputProfile, ControllerInputReleases, Stick,
};
pub use repeat::KeyRepeat;
pub use tree::{
    Axis, CustomWidget, Dimension, Direction, Layout, Size, SliderSpec, Ui, UiError, UiEvent,
    UiInput, WidgetId,
};

#[cfg(all(feature = "gx-backend", target_arch = "powerpc"))]
pub use gx::GxCanvas;
#[cfg(all(feature = "pad-input", target_arch = "powerpc"))]
pub use pad::PadInputAdapter;
#[cfg(all(feature = "wpad-input", target_arch = "powerpc"))]
pub use wpad::WpadInputAdapter;
