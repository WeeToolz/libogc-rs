//! Synchronous C-compatible decoder callbacks from `decoder.c`.

use alloc::alloc::{Layout, alloc, dealloc};
use core::ffi::c_void;

use super::{
    abi::*,
    callbacks::{default_error, flow_from_abi},
    frame::{frame_decode, frame_finish, frame_init, header_decode},
    stream::{stream_finish, stream_init},
    synth::{synth_frame, synth_init},
};
use libmad_rs::decoder::Flow;

/// Initialize a public C decoder and install its callbacks.
///
/// # Safety
/// `decoder` may be null or point to writable `MadDecoder` storage. Callback
/// function pointers must follow the C callback contracts when called later.
pub unsafe fn decoder_init(
    decoder: *mut MadDecoder,
    cb_data: *mut c_void,
    callbacks: MadDecoderCallbacks,
) {
    if decoder.is_null() {
        return;
    }
    unsafe {
        *decoder = MadDecoder {
            mode: -1,
            options: 0,
            async_: MadDecoderAsync::default(),
            sync: core::ptr::null_mut(),
            cb_data,
            input_func: callbacks.input,
            header_func: callbacks.header,
            filter_func: callbacks.filter,
            output_func: callbacks.output,
            error_func: callbacks.error,
            message_func: callbacks.message,
        };
    }
}

/// Finish an initialized decoder.
///
/// The bundled libogc build does not enable `USE_ASYNC`, so the C function is
/// a no-op for synchronous decoding. In particular, calling it from a decoder
/// callback must not release the live `decoder.sync` state; `decoder_run` owns
/// that allocation until the run returns.
///
/// # Safety
/// `decoder` may be null or point to a valid `MadDecoder`.
pub unsafe fn decoder_finish(decoder: *mut MadDecoder) -> i32 {
    if decoder.is_null() {
        return -1;
    }
    0
}

/// Run the synchronous decoder driver.
///
/// Async mode is unavailable, matching this libogc build without `USE_ASYNC`.
///
/// # Safety
/// `decoder` must be null or point to writable `MadDecoder` storage. Input
/// buffers and all callback data must remain valid while callbacks run.
pub unsafe fn decoder_run(decoder: *mut MadDecoder, mode: i32) -> i32 {
    if decoder.is_null() {
        return -1;
    }
    unsafe { (*decoder).mode = mode };
    if mode != MAD_DECODER_MODE_SYNC {
        return -1;
    }

    let layout = Layout::new::<MadDecoderSync>();
    let sync = unsafe { alloc(layout).cast::<MadDecoderSync>() };
    if sync.is_null() {
        return -1;
    }

    unsafe { (*decoder).sync = sync };
    let has_input = unsafe { (*decoder).input_func.is_some() };

    let result = if !has_input {
        // C returns from run_sync before it initializes or finishes the
        // nested stream, frame, or synth objects when input_func is absent.
        0
    } else {
        unsafe {
            stream_init(core::ptr::addr_of_mut!((*sync).stream));
            frame_init(core::ptr::addr_of_mut!((*sync).frame));
            synth_init(core::ptr::addr_of_mut!((*sync).synth));
            (*sync).stream.options = (*decoder).options;
        }
        let mut driver = unsafe { DecoderDriver::new(decoder, sync) };
        unsafe { driver.run() }
    };

    unsafe {
        if has_input {
            frame_finish(core::ptr::addr_of_mut!((*sync).frame));
            stream_finish(core::ptr::addr_of_mut!((*sync).stream));
        }
        dealloc(sync.cast::<u8>(), layout);
        (*decoder).sync = core::ptr::null_mut();
    }
    result
}

/// One C `run_sync()` invocation.
///
/// The error callback and its callback-data pointer are snapshotted at run
/// entry, exactly as `decoder.c` does. The other callback pointers and
/// `decoder.cb_data` are read at the point of use, so mutations made by an
/// earlier callback are visible to later stages in the same run.
struct DecoderDriver {
    decoder: *mut MadDecoder,
    sync: *mut MadDecoderSync,
    error_func: MadErrorFn,
    error_data: *mut c_void,
    bad_last_frame: bool,
}

impl DecoderDriver {
    unsafe fn new(decoder: *mut MadDecoder, sync: *mut MadDecoderSync) -> Self {
        let error_func = unsafe { (*decoder).error_func };
        let error_data = if error_func.is_some() {
            unsafe { (*decoder).cb_data }
        } else {
            core::ptr::null_mut()
        };
        Self {
            decoder,
            sync,
            error_func,
            error_data,
            bad_last_frame: false,
        }
    }

    unsafe fn run(&mut self) -> i32 {
        if libmad_rs::decoder::driver::run(self).is_ok() {
            0
        } else {
            -1
        }
    }

    unsafe fn dispatch_error(&mut self, error: i32) -> Flow {
        if let Some(callback) = self.error_func {
            flow_from_abi(unsafe {
                callback(
                    self.error_data,
                    &mut (*self.sync).stream,
                    &mut (*self.sync).frame,
                )
            })
        } else {
            unsafe { default_error(error, &mut (*self.sync).frame, &mut self.bad_last_frame) }
        }
    }
}

impl libmad_rs::decoder::driver::SyncDriver for DecoderDriver {
    type Error = i32;

    fn input(&mut self) -> Flow {
        let Some(input) = (unsafe { (*self.decoder).input_func }) else {
            return Flow::Stop;
        };
        flow_from_abi(unsafe { input((*self.decoder).cb_data, &mut (*self.sync).stream) })
    }

    fn header(&mut self) -> Result<Flow, i32> {
        let Some(header) = (unsafe { (*self.decoder).header_func }) else {
            return Ok(Flow::Continue);
        };
        if unsafe { header_decode(&mut (*self.sync).frame.header, &mut (*self.sync).stream) } == -1
        {
            return Err(unsafe { (*self.sync).stream.error });
        }
        Ok(flow_from_abi(unsafe {
            header((*self.decoder).cb_data, &(*self.sync).frame.header)
        }))
    }

    fn decode(&mut self) -> Result<(), i32> {
        if unsafe { frame_decode(&mut (*self.sync).frame, &mut (*self.sync).stream) } == -1 {
            Err(unsafe { (*self.sync).stream.error })
        } else {
            self.bad_last_frame = false;
            Ok(())
        }
    }

    fn filter(&mut self) -> Flow {
        let Some(filter) = (unsafe { (*self.decoder).filter_func }) else {
            return Flow::Continue;
        };
        flow_from_abi(unsafe {
            filter(
                (*self.decoder).cb_data,
                &(*self.sync).stream,
                &mut (*self.sync).frame,
            )
        })
    }

    fn output(&mut self) -> Flow {
        unsafe { synth_frame(&mut (*self.sync).synth, &(*self.sync).frame) };
        let Some(output) = (unsafe { (*self.decoder).output_func }) else {
            return Flow::Continue;
        };
        flow_from_abi(unsafe {
            output(
                (*self.decoder).cb_data,
                &(*self.sync).frame.header,
                &mut (*self.sync).synth.pcm,
            )
        })
    }

    fn dispatch_error(&mut self, error: i32) -> Flow {
        unsafe { DecoderDriver::dispatch_error(self, error) }
    }
    fn error(&self) -> i32 {
        unsafe { (*self.sync).stream.error }
    }
    fn recoverable(error: i32) -> bool {
        super::error::mad_recoverable(error)
    }
    fn needs_input(&self) -> bool {
        unsafe { (*self.sync).stream.error == MAD_ERROR_BUFLEN }
    }
}

/// Message exchange is unavailable in the sync-only configuration.
///
/// # Safety
/// `decoder` may be null or point to readable `MadDecoder` storage. Other
/// arguments are unused in this build.
pub unsafe fn decoder_message(
    decoder: *mut MadDecoder,
    _message: *mut c_void,
    _len: *mut u32,
) -> i32 {
    if decoder.is_null() {
        return -1;
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imp::abi::{MadHeader, MadPcm};
    use crate::imp::stream::stream_buffer;

    struct DecoderTestContext {
        decoder: *mut MadDecoder,
        input: [u8; 40],
        calls: usize,
        events: [i32; 5],
        event_count: usize,
    }

    impl DecoderTestContext {
        fn push(&mut self, event: i32) {
            self.events[self.event_count] = event;
            self.event_count += 1;
        }
    }

    unsafe extern "C" fn one_frame_input(data: *mut c_void, stream: *mut MadStream) -> i32 {
        let context = unsafe { &mut *data.cast::<DecoderTestContext>() };
        context.calls += 1;
        context.push(1);
        if context.calls == 1 {
            unsafe { stream_buffer(stream, context.input.as_ptr(), context.input.len() as u32) };
            MAD_FLOW_CONTINUE
        } else {
            MAD_FLOW_STOP
        }
    }

    unsafe extern "C" fn record_header(data: *mut c_void, header: *const MadHeader) -> i32 {
        let context = unsafe { &mut *data.cast::<DecoderTestContext>() };
        context.push(2);
        assert_eq!(unsafe { (*header).layer }, MAD_LAYER_I);
        MAD_FLOW_CONTINUE
    }

    unsafe extern "C" fn change_frame(
        data: *mut c_void,
        _stream: *const MadStream,
        frame: *mut MadFrame,
    ) -> i32 {
        let context = unsafe { &mut *data.cast::<DecoderTestContext>() };
        context.push(3);
        unsafe {
            (*frame).sbsample[0][0][0] = libmad_rs::fixed::Fixed::MAX / 2;
            // The shared driver must see callbacks installed during a run.
            (*context.decoder).output_func = Some(verify_output);
        }
        MAD_FLOW_CONTINUE
    }

    unsafe extern "C" fn verify_output(
        data: *mut c_void,
        _header: *const MadHeader,
        pcm: *mut MadPcm,
    ) -> i32 {
        let context = unsafe { &mut *data.cast::<DecoderTestContext>() };
        context.push(4);
        let pcm = unsafe { &*pcm };
        assert_eq!(pcm.channels, 1);
        assert_eq!(pcm.length, 384);
        assert!(
            pcm.samples[0][..pcm.length as usize]
                .iter()
                .any(|&sample| sample != 0)
        );
        MAD_FLOW_STOP
    }

    unsafe extern "C" fn stop_input(_data: *mut c_void, _stream: *mut MadStream) -> i32 {
        MAD_FLOW_STOP
    }

    unsafe extern "C" fn continue_without_buffer(data: *mut c_void, _: *mut MadStream) -> i32 {
        unsafe { *data.cast::<usize>() += 1 };
        MAD_FLOW_CONTINUE
    }

    unsafe extern "C" fn count_unexpected_header(data: *mut c_void, _: *const MadHeader) -> i32 {
        unsafe { *data.cast::<usize>() += 100 };
        MAD_FLOW_BREAK
    }

    unsafe extern "C" fn count_unexpected_terminal_error(
        data: *mut c_void,
        _: *mut MadStream,
        _: *mut MadFrame,
    ) -> i32 {
        unsafe { *data.cast::<usize>() += 100 };
        MAD_FLOW_STOP
    }

    #[test]
    fn terminal_buffer_pointer_error_fails_and_releases_sync_state() {
        // Exercise both header parsing paths in the ABI adapter.
        for header in [
            None,
            Some(count_unexpected_header as unsafe extern "C" fn(_, _) -> _),
        ] {
            let mut calls = 0usize;
            let mut decoder = MadDecoder::default();
            let callbacks = MadDecoderCallbacks {
                input: Some(continue_without_buffer),
                header,
                error: Some(count_unexpected_terminal_error),
                ..MadDecoderCallbacks::default()
            };
            unsafe {
                decoder_init(&mut decoder, (&mut calls as *mut usize).cast(), callbacks);
                assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), -1);
                assert!(decoder.sync.is_null());
                assert_eq!(decoder_finish(&mut decoder), 0);
            }
            assert_eq!(calls, 1);
        }
    }

    struct IgnoreInputContext {
        calls: usize,
        initial_error: i32,
    }

    unsafe extern "C" fn ignore_input_once(data: *mut c_void, stream: *mut MadStream) -> i32 {
        let context = unsafe { &mut *data.cast::<IgnoreInputContext>() };
        context.calls += 1;
        if context.calls == 1 {
            unsafe { (*stream).error = context.initial_error };
            MAD_FLOW_IGNORE
        } else {
            MAD_FLOW_STOP
        }
    }

    #[test]
    fn input_ignore_retries_with_non_buffer_error() {
        let mut context = IgnoreInputContext {
            calls: 0,
            initial_error: MAD_ERROR_NONE,
        };
        let mut decoder = MadDecoder::default();
        let callbacks = MadDecoderCallbacks {
            input: Some(ignore_input_once),
            ..MadDecoderCallbacks::default()
        };
        unsafe {
            decoder_init(
                &mut decoder,
                (&mut context as *mut IgnoreInputContext).cast(),
                callbacks,
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
        }
        assert_eq!(context.calls, 2);
        assert!(decoder.sync.is_null());
    }

    #[test]
    fn sync_run_without_input_skips_nested_state_cleanup() {
        let mut decoder = MadDecoder::default();
        unsafe {
            decoder_init(
                &mut decoder,
                core::ptr::null_mut(),
                MadDecoderCallbacks::default(),
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
        }
        assert!(decoder.sync.is_null());
    }

    #[test]
    fn input_ignore_with_buffer_length_error_retries_input() {
        let mut context = IgnoreInputContext {
            calls: 0,
            initial_error: MAD_ERROR_BUFLEN,
        };
        let mut decoder = MadDecoder::default();
        let callbacks = MadDecoderCallbacks {
            input: Some(ignore_input_once),
            ..MadDecoderCallbacks::default()
        };
        unsafe {
            decoder_init(
                &mut decoder,
                (&mut context as *mut IgnoreInputContext).cast(),
                callbacks,
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
        }
        assert_eq!(context.calls, 2);
        assert!(decoder.sync.is_null());
    }

    #[test]
    fn sync_run_stop_returns_success_and_releases_state() {
        let mut decoder = MadDecoder::default();
        let callbacks = MadDecoderCallbacks {
            input: Some(stop_input),
            ..MadDecoderCallbacks::default()
        };
        unsafe {
            decoder_init(&mut decoder, core::ptr::null_mut(), callbacks);
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
            assert!(decoder.sync.is_null());
            assert_eq!(decoder_finish(&mut decoder), 0);
        }
    }

    #[test]
    fn async_mode_is_rejected_without_allocating_sync_state() {
        let mut decoder = MadDecoder::default();
        unsafe {
            decoder_init(
                &mut decoder,
                core::ptr::null_mut(),
                MadDecoderCallbacks::default(),
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_ASYNC), -1);
            assert!(decoder.sync.is_null());
        }
    }

    #[test]
    fn synchronous_callbacks_keep_c_order_and_filter_subbands_before_synthesis() {
        let mut context = DecoderTestContext {
            decoder: core::ptr::null_mut(),
            input: [0; 40],
            calls: 0,
            events: [0; 5],
            event_count: 0,
        };
        context.input[..4].copy_from_slice(&[0xff, 0xfe, 0x10, 0xc0]);
        let callbacks = MadDecoderCallbacks {
            input: Some(one_frame_input),
            header: Some(record_header),
            filter: Some(change_frame),
            output: None,
            ..MadDecoderCallbacks::default()
        };
        let mut decoder = MadDecoder::default();
        context.decoder = &mut decoder;
        unsafe {
            decoder_init(
                &mut decoder,
                (&mut context as *mut DecoderTestContext).cast(),
                callbacks,
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
        }
        assert_eq!(context.events, [1, 2, 3, 4, 0]);
        assert_eq!(context.event_count, 4);
    }

    struct ErrorMutationContext {
        decoder: *mut MadDecoder,
        bytes: [u8; 40],
        original_calls: usize,
        replacement_calls: usize,
    }

    unsafe extern "C" fn replace_error_handler_during_input(
        data: *mut c_void,
        stream: *mut MadStream,
    ) -> i32 {
        let context = unsafe { &mut *data.cast::<ErrorMutationContext>() };
        unsafe {
            stream_buffer(stream, context.bytes.as_ptr(), context.bytes.len() as u32);
            (*context.decoder).error_func = Some(count_replacement_error);
            (*context.decoder).cb_data = (&mut context.replacement_calls as *mut usize).cast();
        }
        MAD_FLOW_CONTINUE
    }

    unsafe extern "C" fn count_original_error(
        data: *mut c_void,
        stream: *mut MadStream,
        frame: *mut MadFrame,
    ) -> i32 {
        let context = unsafe { &mut *data.cast::<ErrorMutationContext>() };
        context.original_calls += 1;
        assert_eq!(unsafe { (*stream).error }, MAD_ERROR_BADBITALLOC);
        assert_eq!(unsafe { (*frame).header.layer }, MAD_LAYER_I);
        MAD_FLOW_STOP
    }

    unsafe extern "C" fn count_replacement_error(
        data: *mut c_void,
        _stream: *mut MadStream,
        _frame: *mut MadFrame,
    ) -> i32 {
        unsafe { *data.cast::<usize>() += 1 };
        MAD_FLOW_BREAK
    }

    #[test]
    fn run_uses_original_error_callback_and_data_after_input_replaces_both() {
        let mut decoder = MadDecoder::default();
        let mut context = ErrorMutationContext {
            decoder: &mut decoder,
            bytes: [0; 40],
            original_calls: 0,
            replacement_calls: 0,
        };
        // Unprotected Layer I mono frame with reserved allocation 15.
        context.bytes[..5].copy_from_slice(&[0xff, 0xff, 0x10, 0xc0, 0xf0]);
        let callbacks = MadDecoderCallbacks {
            input: Some(replace_error_handler_during_input),
            error: Some(count_original_error),
            ..MadDecoderCallbacks::default()
        };
        unsafe {
            decoder_init(
                &mut decoder,
                (&mut context as *mut ErrorMutationContext).cast(),
                callbacks,
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
        }
        assert_eq!(context.original_calls, 1);
        assert_eq!(context.replacement_calls, 0);
        assert_eq!(
            decoder.cb_data,
            (&mut context.replacement_calls as *mut usize).cast()
        );
        assert!(decoder.sync.is_null());
    }

    struct FinishContext {
        decoder: *mut MadDecoder,
        saw_live_sync_after_finish: bool,
    }

    unsafe extern "C" fn finish_inside_input(data: *mut c_void, _stream: *mut MadStream) -> i32 {
        let context = unsafe { &mut *data.cast::<FinishContext>() };
        assert_eq!(unsafe { decoder_finish(context.decoder) }, 0);
        context.saw_live_sync_after_finish = unsafe { !(*context.decoder).sync.is_null() };
        MAD_FLOW_STOP
    }

    unsafe extern "C" fn first_error_callback(
        data: *mut c_void,
        _stream: *mut MadStream,
        _frame: *mut MadFrame,
    ) -> i32 {
        unsafe { *data.cast::<u32>() += 1 };
        MAD_FLOW_CONTINUE
    }

    unsafe extern "C" fn replacement_error_callback(
        _data: *mut c_void,
        _stream: *mut MadStream,
        _frame: *mut MadFrame,
    ) -> i32 {
        MAD_FLOW_BREAK
    }

    #[test]
    fn decoder_finish_inside_sync_callback_is_a_noop() {
        let mut decoder = MadDecoder::default();
        let mut context = FinishContext {
            decoder: &mut decoder,
            saw_live_sync_after_finish: false,
        };
        let callbacks = MadDecoderCallbacks {
            input: Some(finish_inside_input),
            ..MadDecoderCallbacks::default()
        };
        unsafe {
            decoder_init(
                &mut decoder,
                (&mut context as *mut FinishContext).cast(),
                callbacks,
            );
            assert_eq!(decoder_run(&mut decoder, MAD_DECODER_MODE_SYNC), 0);
        }
        assert!(context.saw_live_sync_after_finish);
        assert!(decoder.sync.is_null());
    }

    #[test]
    fn decoder_driver_snapshots_error_callback_and_data_at_run_entry() {
        let mut decoder = MadDecoder::default();
        let mut sync = MadDecoderSync::default();
        let mut original_data = 11_u32;
        let mut replacement_data = 22_u32;
        decoder.error_func = Some(first_error_callback);
        decoder.cb_data = (&mut original_data as *mut u32).cast();
        let mut driver = unsafe { DecoderDriver::new(&mut decoder, &mut sync) };

        decoder.error_func = Some(replacement_error_callback);
        decoder.cb_data = (&mut replacement_data as *mut u32).cast();

        assert_eq!(
            decoder.error_func.map(|f| f as *const () as usize),
            Some(replacement_error_callback as *const () as usize)
        );
        assert_eq!(decoder.cb_data, (&mut replacement_data as *mut u32).cast());

        assert_eq!(
            driver.error_func.map(|f| f as *const () as usize),
            Some(first_error_callback as *const () as usize)
        );
        assert_eq!(driver.error_data, (&mut original_data as *mut u32).cast());
        assert_eq!(
            unsafe { driver.dispatch_error(MAD_ERROR_BADCRC) },
            Flow::Continue
        );
        assert_eq!(original_data, 12);
        assert_eq!(replacement_data, 22);
    }
}
