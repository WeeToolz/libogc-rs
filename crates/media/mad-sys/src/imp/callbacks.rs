//! Callback flow conversion and default decoder error handling from `decoder.c`.

use super::{
    abi::{
        MAD_ERROR_BADCRC, MAD_FLOW_BREAK, MAD_FLOW_CONTINUE, MAD_FLOW_IGNORE, MAD_FLOW_STOP,
        MadFrame,
    },
    frame::frame_mute,
};
use libmad_rs::decoder::Flow;

#[inline]
pub(crate) fn flow_from_abi(flow: i32) -> Flow {
    match flow {
        MAD_FLOW_STOP => Flow::Stop,
        MAD_FLOW_BREAK => Flow::Break,
        MAD_FLOW_IGNORE => Flow::Ignore,
        MAD_FLOW_CONTINUE => Flow::Continue,
        _ => Flow::Continue,
    }
}

/// Apply decoder.c's built-in error callback when a decoder has no custom one.
pub(crate) fn default_error(error: i32, frame: &mut MadFrame, bad_last_frame: &mut bool) -> Flow {
    let (flow, mute) =
        libmad_rs::decoder::driver::default_error_action(error == MAD_ERROR_BADCRC, bad_last_frame);
    if mute {
        unsafe { frame_mute(frame) };
    }
    flow
}
