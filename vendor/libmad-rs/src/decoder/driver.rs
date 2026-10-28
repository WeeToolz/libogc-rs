//! Shared synchronous stage control for the Rust and C ABI adapters.

use super::Flow;

/// Adapter operations used by the shared decoder loop.
///
/// Each adapter retains its own frame, PCM, and callback storage. A successful
/// header operation returns `Continue` when no header callback is installed.
#[doc(hidden)]
pub trait SyncDriver {
    type Error: Copy;

    fn input(&mut self) -> Flow;
    fn header(&mut self) -> Result<Flow, Self::Error>;
    fn decode(&mut self) -> Result<(), Self::Error>;
    fn filter(&mut self) -> Flow;
    fn output(&mut self) -> Flow;
    fn dispatch_error(&mut self, error: Self::Error) -> Flow;
    fn error(&self) -> Self::Error;
    fn recoverable(error: Self::Error) -> bool;
    fn needs_input(&self) -> bool;
}

/// Run the libmad synchronous stage sequence.
#[doc(hidden)]
pub fn run<D: SyncDriver>(driver: &mut D) -> Result<(), D::Error> {
    'input: loop {
        match driver.input() {
            Flow::Stop => return Ok(()),
            Flow::Break => return Err(driver.error()),
            Flow::Ignore => continue 'input,
            Flow::Continue => {}
        }
        'frame: loop {
            let header_flow = match driver.header() {
                Ok(flow) => flow,
                Err(error) => {
                    if !D::recoverable(error) {
                        break 'frame;
                    }
                    match driver.dispatch_error(error) {
                        Flow::Stop => return Ok(()),
                        Flow::Break => return Err(error),
                        Flow::Ignore | Flow::Continue => continue 'frame,
                    }
                }
            };
            match header_flow {
                Flow::Stop => return Ok(()),
                Flow::Break => return Err(driver.error()),
                Flow::Ignore => continue 'frame,
                Flow::Continue => {}
            }
            if let Err(error) = driver.decode() {
                if !D::recoverable(error) {
                    break 'frame;
                }
                match driver.dispatch_error(error) {
                    Flow::Stop => return Ok(()),
                    Flow::Break => return Err(error),
                    // IGNORE keeps the failed frame for filtering and output.
                    Flow::Ignore => {}
                    Flow::Continue => continue 'frame,
                }
            }
            match driver.filter() {
                Flow::Stop => return Ok(()),
                Flow::Break => return Err(driver.error()),
                Flow::Ignore => continue 'frame,
                Flow::Continue => {}
            }
            match driver.output() {
                Flow::Stop => return Ok(()),
                Flow::Break => return Err(driver.error()),
                Flow::Ignore | Flow::Continue => {}
            }
        }
        if !driver.needs_input() {
            return Err(driver.error());
        }
    }
}

/// Return the default error flow and whether the adapter must mute its frame.
#[doc(hidden)]
pub fn default_error_action(bad_crc: bool, bad_last_frame: &mut bool) -> (Flow, bool) {
    if bad_crc {
        let mute = *bad_last_frame;
        *bad_last_frame = true;
        (Flow::Ignore, mute)
    } else {
        (Flow::Continue, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use alloc::{vec, vec::Vec};

    struct Driver {
        events: Vec<&'static str>,
        input: Flow,
        header: Result<Flow, Error>,
        decode: Result<(), Error>,
        filter: Flow,
        output: Flow,
        error_flow: Flow,
        error: Error,
        input_calls: usize,
        header_calls: usize,
    }

    impl Default for Driver {
        fn default() -> Self {
            Self {
                events: vec![],
                input: Flow::Continue,
                header: Ok(Flow::Continue),
                decode: Ok(()),
                filter: Flow::Continue,
                output: Flow::Stop,
                error_flow: Flow::Continue,
                error: Error::None,
                input_calls: 0,
                header_calls: 0,
            }
        }
    }

    impl SyncDriver for Driver {
        type Error = Error;
        fn input(&mut self) -> Flow {
            self.events.push("input");
            self.input_calls += 1;
            if self.input_calls > 1 {
                Flow::Stop
            } else {
                self.input
            }
        }
        fn header(&mut self) -> Result<Flow, Error> {
            self.events.push("header");
            self.header_calls += 1;
            if self.header_calls > 1 {
                return Ok(Flow::Stop);
            }
            if let Err(error) = self.header {
                self.error = error;
            }
            self.header
        }
        fn decode(&mut self) -> Result<(), Error> {
            self.events.push("decode");
            if let Err(error) = self.decode {
                self.error = error;
            }
            self.decode
        }
        fn filter(&mut self) -> Flow {
            self.events.push("filter");
            self.filter
        }
        fn output(&mut self) -> Flow {
            self.events.push("output");
            self.output
        }
        fn dispatch_error(&mut self, _: Error) -> Flow {
            self.events.push("error");
            self.error_flow
        }
        fn error(&self) -> Error {
            self.error
        }
        fn recoverable(error: Error) -> bool {
            error.is_recoverable()
        }
        fn needs_input(&self) -> bool {
            self.error == Error::BufferLength
        }
    }

    #[test]
    fn stage_flows_skip_or_stop_at_the_correct_boundary() {
        for flow in [Flow::Stop, Flow::Break, Flow::Ignore, Flow::Continue] {
            for stage in ["input", "header", "filter", "output"] {
                let mut driver = Driver::default();
                match stage {
                    "input" => driver.input = flow,
                    "header" => driver.header = Ok(flow),
                    "filter" => driver.filter = flow,
                    "output" => driver.output = flow,
                    _ => unreachable!(),
                }
                let result = run(&mut driver);
                assert_eq!(
                    result,
                    if flow == Flow::Break {
                        Err(Error::None)
                    } else {
                        Ok(())
                    }
                );
                let expected: &[&str] = match (stage, flow) {
                    ("input", Flow::Stop | Flow::Break) => &["input"],
                    ("input", Flow::Ignore) => &["input", "input"],
                    ("header", Flow::Stop | Flow::Break) => &["input", "header"],
                    ("header", Flow::Ignore) => &["input", "header", "header"],
                    ("filter", Flow::Stop | Flow::Break) => {
                        &["input", "header", "decode", "filter"]
                    }
                    ("filter", Flow::Ignore) => &["input", "header", "decode", "filter", "header"],
                    ("output", Flow::Continue | Flow::Ignore) => {
                        &["input", "header", "decode", "filter", "output", "header"]
                    }
                    _ => &["input", "header", "decode", "filter", "output"],
                };
                assert_eq!(driver.events, expected, "{stage}: {flow:?}");
            }
        }
    }

    #[test]
    fn recoverable_errors_keep_failed_frames_only_after_decode_ignore() {
        for header_error in [true, false] {
            for flow in [Flow::Stop, Flow::Break, Flow::Ignore, Flow::Continue] {
                let mut driver = Driver {
                    error_flow: flow,
                    ..Driver::default()
                };
                if header_error {
                    driver.header = Err(Error::BadCrc);
                } else {
                    driver.decode = Err(Error::BadCrc);
                }
                let result = run(&mut driver);
                assert_eq!(
                    result,
                    if flow == Flow::Break {
                        Err(Error::BadCrc)
                    } else {
                        Ok(())
                    }
                );
                let reaches_output = !header_error && flow == Flow::Ignore;
                assert_eq!(driver.events.contains(&"filter"), reaches_output);
                assert_eq!(driver.events.contains(&"output"), reaches_output);
                assert_eq!(
                    driver.header_calls,
                    if flow == Flow::Continue || (header_error && flow == Flow::Ignore) {
                        2
                    } else {
                        1
                    }
                );
            }
        }
    }

    #[test]
    fn terminal_errors_fail_and_buffer_exhaustion_requests_input() {
        for header_error in [true, false] {
            for error in [Error::BufferPointer, Error::NoMemory, Error::BufferLength] {
                let mut driver = Driver::default();
                if header_error {
                    driver.header = Err(error);
                } else {
                    driver.decode = Err(error);
                }
                assert_eq!(
                    run(&mut driver),
                    if error == Error::BufferLength {
                        Ok(())
                    } else {
                        Err(error)
                    }
                );
                assert_eq!(
                    driver.input_calls,
                    if error == Error::BufferLength { 2 } else { 1 }
                );
                assert!(!driver.events.contains(&"error"));
                assert!(!driver.events.contains(&"output"));
            }
        }
    }
}
