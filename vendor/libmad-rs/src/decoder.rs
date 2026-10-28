//! Synchronous decoder callback flow from `decoder.c`.

#[doc(hidden)]
pub mod driver;

use crate::{
    frame::{Frame, Header, decode_header},
    stream::{Error, Stream},
};

pub type InputCallback<'a> = dyn FnMut(&mut Stream) -> Flow + 'a;
pub type HeaderCallback<'a> = dyn FnMut(&Header) -> Flow + 'a;
pub type FrameDecodeCallback<'a> = dyn FnMut(&mut Stream, &mut Frame) -> Result<(), Error> + 'a;
pub type FilterCallback<'a> = dyn FnMut(&Stream, &mut Frame) -> Flow + 'a;
pub type OutputCallback<'a, P> = dyn FnMut(&Header, &P) -> Flow + 'a;
pub type ErrorCallback<'a> = Box<dyn FnMut(&mut Stream, &mut Frame) -> Flow + 'a>;
use alloc::boxed::Box;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flow {
    Continue,
    Stop,
    Break,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeStatus {
    Complete,
    Incomplete,
    Failed(Error),
}

/// Callbacks used by [`run_sync`]. The decode and synthesis functions are
/// supplied by the layer and synthesis stages, so the driver can preserve
/// libmad's callback order without coupling it to a particular PCM layout.
pub struct SyncCallbacks<'a, P> {
    pub input: &'a mut InputCallback<'a>,
    pub header: Option<&'a mut HeaderCallback<'a>>,
    pub decode: &'a mut FrameDecodeCallback<'a>,
    pub filter: Option<&'a mut FilterCallback<'a>>,
    pub synth: &'a mut dyn FnMut(&Frame) -> P,
    pub output: Option<&'a mut OutputCallback<'a, P>>,
    pub error: Option<ErrorCallback<'a>>,
}

impl<P> core::fmt::Debug for SyncCallbacks<'_, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SyncCallbacks")
            .field("header", &self.header.is_some())
            .field("filter", &self.filter.is_some())
            .field("output", &self.output.is_some())
            .field("error", &self.error.is_some())
            .finish_non_exhaustive()
    }
}

/// Run the synchronous callback loop from `decoder.c`.
///
/// Header parsing always occurs because frame decoding needs it. The optional
/// header callback runs immediately after a successful parse. Frame decoding
/// and synthesis are supplied by the caller. Terminal errors other than
/// buffer exhaustion return `DecodeStatus::Failed`. Buffer exhaustion requests
/// more input.
pub fn run_sync<P>(stream: &mut Stream, callbacks: &mut SyncCallbacks<'_, P>) -> DecodeStatus {
    let mut adapter = RustDriver {
        stream,
        frame: Frame::default(),
        callbacks,
        bad_last_frame: false,
    };
    match driver::run(&mut adapter) {
        Ok(()) => DecodeStatus::Complete,
        Err(error) => DecodeStatus::Failed(error),
    }
}

struct RustDriver<'s, 'c, 'a, P> {
    stream: &'s mut Stream,
    frame: Frame,
    callbacks: &'c mut SyncCallbacks<'a, P>,
    bad_last_frame: bool,
}

impl<P> driver::SyncDriver for RustDriver<'_, '_, '_, P> {
    type Error = Error;

    fn input(&mut self) -> Flow {
        (self.callbacks.input)(self.stream)
    }

    fn header(&mut self) -> Result<Flow, Error> {
        self.frame.header = decode_header(self.stream)?;
        Ok(self
            .callbacks
            .header
            .as_deref_mut()
            .map_or(Flow::Continue, |callback| callback(&self.frame.header)))
    }

    fn decode(&mut self) -> Result<(), Error> {
        let result = (self.callbacks.decode)(self.stream, &mut self.frame);
        match result {
            Ok(()) => self.bad_last_frame = false,
            Err(error) => self.stream.error = error,
        }
        result
    }

    fn filter(&mut self) -> Flow {
        self.callbacks
            .filter
            .as_deref_mut()
            .map_or(Flow::Continue, |callback| {
                callback(self.stream, &mut self.frame)
            })
    }

    fn output(&mut self) -> Flow {
        let pcm = (self.callbacks.synth)(&self.frame);
        self.callbacks
            .output
            .as_deref_mut()
            .map_or(Flow::Continue, |callback| {
                callback(&self.frame.header, &pcm)
            })
    }

    fn dispatch_error(&mut self, _: Error) -> Flow {
        if let Some(callback) = self.callbacks.error.as_mut() {
            callback(self.stream, &mut self.frame)
        } else {
            default_error(self.stream, &mut self.frame, &mut self.bad_last_frame)
        }
    }

    fn error(&self) -> Error {
        self.stream.error
    }
    fn recoverable(error: Error) -> bool {
        error.is_recoverable()
    }
    fn needs_input(&self) -> bool {
        self.stream.error == Error::BufferLength
    }
}

fn default_error(stream: &mut Stream, frame: &mut Frame, bad_last_frame: &mut bool) -> Flow {
    let (flow, mute) = driver::default_error_action(stream.error == Error::BadCrc, bad_last_frame);
    if mute {
        frame.mute();
    }
    flow
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Layer;
    use alloc::vec;
    use core::cell::RefCell;

    fn mpeg1_frames(count: usize) -> Stream {
        let mut bytes = vec![0; 417 * count + crate::MAD_BUFFER_GUARD];
        for frame in bytes[..417 * count].chunks_exact_mut(417) {
            frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        }
        let mut stream = Stream::new();
        stream.buffer(&bytes);
        stream
    }

    #[test]
    fn recoverable_decode_flows_preserve_callback_frame_changes_and_original_error() {
        for flow in [Flow::Stop, Flow::Break, Flow::Ignore, Flow::Continue] {
            let mut stream = mpeg1_frames(2);
            let mut input = |_: &mut Stream| Flow::Continue;
            let mut decoded = 0;
            let mut decode = |_: &mut Stream, frame: &mut Frame| {
                decoded += 1;
                frame.sbsample[0][0][0] = 456;
                if decoded == 1 {
                    Err(Error::BadCrc)
                } else {
                    Ok(())
                }
            };
            let mut filtered = 0;
            let mut filter = |_: &Stream, _: &mut Frame| {
                filtered += 1;
                Flow::Continue
            };
            let mut synth = |frame: &Frame| frame.sbsample[0][0][0];
            let mut samples = vec![];
            let mut output = |_: &Header, sample: &i32| {
                samples.push(*sample);
                Flow::Stop
            };
            let mut errors = 0;
            let mut callbacks = SyncCallbacks {
                input: &mut input,
                header: None,
                decode: &mut decode,
                filter: Some(&mut filter),
                synth: &mut synth,
                output: Some(&mut output),
                error: Some(Box::new(|stream, frame| {
                    errors += 1;
                    assert_eq!(stream.error, Error::BadCrc);
                    frame.sbsample[0][0][0] = 777;
                    stream.error = Error::BadStereo;
                    flow
                })),
            };
            let result = run_sync(&mut stream, &mut callbacks);
            drop(callbacks);
            assert_eq!(
                result,
                if flow == Flow::Break {
                    DecodeStatus::Failed(Error::BadCrc)
                } else {
                    DecodeStatus::Complete
                }
            );
            assert_eq!(errors, 1);
            assert_eq!(decoded, if flow == Flow::Continue { 2 } else { 1 });
            let expected: &[i32] = match flow {
                Flow::Stop | Flow::Break => &[],
                Flow::Ignore => &[777],
                Flow::Continue => &[456],
            };
            assert_eq!(samples, expected, "{flow:?}");
            assert_eq!(filtered, expected.len());
        }
    }

    #[test]
    fn buffer_exhaustion_refills_input_and_resumes_frame_output() {
        let mut stream = mpeg1_frames(1);
        let bytes = stream.buffer.clone();
        let mut input_calls = 0;
        let mut input = |stream: &mut Stream| {
            input_calls += 1;
            match input_calls {
                1 => Flow::Continue,
                2 => {
                    assert_eq!(stream.error, Error::BufferLength);
                    stream.buffer(&bytes);
                    Flow::Continue
                }
                3 => {
                    assert_eq!(stream.error, Error::BufferLength);
                    Flow::Stop
                }
                _ => panic!("unexpected input retry"),
            }
        };
        let mut decoded = 0;
        let mut decode = |_: &mut Stream, frame: &mut Frame| {
            decoded += 1;
            frame.sbsample[0][0][0] = decoded;
            Ok(())
        };
        let mut synth = |frame: &Frame| frame.sbsample[0][0][0];
        let mut samples = vec![];
        let mut output = |_: &Header, sample: &i32| {
            samples.push(*sample);
            Flow::Continue
        };
        let mut callbacks = SyncCallbacks {
            input: &mut input,
            header: None,
            decode: &mut decode,
            filter: None,
            synth: &mut synth,
            output: Some(&mut output),
            error: Some(Box::new(|stream, _| {
                // The zero guard bytes produce a recoverable sync error before EOF.
                assert_eq!(stream.error, Error::LostSync);
                Flow::Continue
            })),
        };
        assert_eq!(
            run_sync(&mut stream, &mut callbacks),
            DecodeStatus::Complete
        );
        drop(callbacks);
        assert_eq!(input_calls, 3);
        assert_eq!(decoded, 2);
        assert_eq!(samples, [1, 2]);
    }

    #[test]
    fn successful_frame_resets_default_crc_mute_history() {
        let mut stream = mpeg1_frames(4);
        let mut input = |_: &mut Stream| Flow::Continue;
        let mut decoded = 0;
        let mut decode = |_: &mut Stream, frame: &mut Frame| {
            decoded += 1;
            frame.sbsample[0][0][0] = if decoded == 3 { 789 } else { 123 };
            if decoded == 3 {
                Ok(())
            } else {
                Err(Error::BadCrc)
            }
        };
        let mut synth = |frame: &Frame| frame.sbsample[0][0][0];
        let mut samples = vec![];
        let mut output = |_: &Header, sample: &i32| {
            samples.push(*sample);
            if samples.len() == 4 {
                Flow::Stop
            } else {
                Flow::Continue
            }
        };
        let mut callbacks = SyncCallbacks {
            input: &mut input,
            header: None,
            decode: &mut decode,
            filter: None,
            synth: &mut synth,
            output: Some(&mut output),
            error: None,
        };
        assert_eq!(
            run_sync(&mut stream, &mut callbacks),
            DecodeStatus::Complete
        );
        drop(callbacks);
        assert_eq!(samples, [123, 0, 789, 123]);
        assert_eq!(decoded, 4);
    }

    #[test]
    fn terminal_decode_errors_stop_before_filter_and_synthesis() {
        for error in [Error::BufferPointer, Error::NoMemory] {
            let mut stream = mpeg1_frames(1);
            let mut input = |_: &mut Stream| Flow::Continue;
            let mut decode = |_: &mut Stream, _: &mut Frame| Err(error);
            let mut filter = |_: &Stream, _: &mut Frame| panic!("filter after terminal error");
            let mut synth = |_: &Frame| panic!("synthesis after terminal error");
            let mut callbacks = SyncCallbacks {
                input: &mut input,
                header: None,
                decode: &mut decode,
                filter: Some(&mut filter),
                synth: &mut synth,
                output: None,
                error: Some(Box::new(|_, _| panic!("error callback for terminal error"))),
            };
            assert_eq!(
                run_sync::<()>(&mut stream, &mut callbacks),
                DecodeStatus::Failed(error)
            );
            assert_eq!(stream.error, error);
        }
    }

    #[test]
    fn terminal_header_error_is_reported_as_failure() {
        let mut stream = Stream::new();
        let mut input = |_: &mut Stream| Flow::Continue;
        let mut decode = |_: &mut Stream, _: &mut Frame| panic!("decode after invalid header");
        let mut synth = |_: &Frame| panic!("synthesis after invalid header");
        let mut callbacks = SyncCallbacks {
            input: &mut input,
            header: None,
            decode: &mut decode,
            filter: None,
            synth: &mut synth,
            output: None,
            error: None,
        };
        assert_eq!(
            run_sync::<()>(&mut stream, &mut callbacks),
            DecodeStatus::Failed(Error::BufferPointer)
        );
    }

    #[test]
    fn sync_driver_keeps_callback_order_and_stops_cleanly() {
        let mut stream = Stream::new();
        let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
        bytes.resize(417 + 8, 0);
        stream.buffer(&bytes);
        let events = RefCell::new(vec![]);
        let mut input_calls = 0;
        let mut input = |_: &mut Stream| {
            input_calls += 1;
            events.borrow_mut().push("input");
            if input_calls == 1 {
                Flow::Continue
            } else {
                Flow::Stop
            }
        };
        let mut header = |h: &Header| {
            assert_eq!(h.layer, Layer::III);
            events.borrow_mut().push("header");
            Flow::Continue
        };
        let mut decode = |_: &mut Stream, _: &mut Frame| {
            events.borrow_mut().push("decode");
            Ok(())
        };
        let mut filter = |_: &Stream, _: &mut Frame| {
            events.borrow_mut().push("filter");
            Flow::Continue
        };
        let mut synth = |_: &Frame| {
            events.borrow_mut().push("synth");
            7_u8
        };
        let mut output = |_: &Header, pcm: &u8| {
            assert_eq!(*pcm, 7);
            events.borrow_mut().push("output");
            Flow::Continue
        };
        let mut callbacks = SyncCallbacks {
            input: &mut input,
            header: Some(&mut header),
            decode: &mut decode,
            filter: Some(&mut filter),
            synth: &mut synth,
            output: Some(&mut output),
            error: None,
        };
        assert_eq!(
            run_sync(&mut stream, &mut callbacks),
            DecodeStatus::Complete
        );
        assert_eq!(
            *events.borrow(),
            [
                "input", "header", "decode", "filter", "synth", "output", "input"
            ]
        );
    }

    #[test]
    fn default_bad_crc_handler_mutes_repeated_bad_frame() {
        let mut stream = Stream::new();
        let mut frame = Frame::default();
        frame.sbsample[0][0][0] = 123;
        let mut bad_last_frame = false;
        stream.error = Error::BadCrc;
        assert_eq!(
            default_error(&mut stream, &mut frame, &mut bad_last_frame),
            Flow::Ignore
        );
        assert_eq!(frame.sbsample[0][0][0], 123);
        assert_eq!(
            default_error(&mut stream, &mut frame, &mut bad_last_frame),
            Flow::Ignore
        );
        assert_eq!(frame.sbsample[0][0][0], 0);
    }

    #[test]
    fn header_ignore_skips_frame_decode_and_filtering() {
        use core::cell::Cell;

        let mut stream = Stream::new();
        let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
        bytes.resize(417 + 8, 0);
        stream.buffer(&bytes);
        let decoded = Cell::new(false);
        let mut input_calls = 0;
        let mut input = |_: &mut Stream| {
            input_calls += 1;
            if input_calls == 1 {
                Flow::Continue
            } else {
                Flow::Stop
            }
        };
        let mut header = |_: &Header| Flow::Ignore;
        let mut decode = |_: &mut Stream, _: &mut Frame| {
            decoded.set(true);
            Ok(())
        };
        let mut synth = |_: &Frame| ();
        let mut callbacks = SyncCallbacks {
            input: &mut input,
            header: Some(&mut header),
            decode: &mut decode,
            filter: None,
            synth: &mut synth,
            output: None,
            error: None,
        };
        assert_eq!(
            run_sync(&mut stream, &mut callbacks),
            DecodeStatus::Complete
        );
        assert!(!decoded.get());
    }
}
