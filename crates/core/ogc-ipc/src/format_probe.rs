use core::sync::atomic::{AtomicUsize, Ordering};

use super::{FormatCall, FormatState, Ioctlv};

/// One bounded formatter vector sample for Dolphin probes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FormatProbeVector {
    pub data_address: usize,
    pub length: u32,
    pub sample_length: u8,
    pub sample: [u8; 8],
}

/// A fixed-size snapshot returned by the one-shot formatter probe hook.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FormatProbeSnapshot {
    pub input_count: u32,
    pub output_count: u32,
    pub asynchronous: u32,
    pub vector_count: u32,
    /// Fill output vectors with this byte before formatter cleanup.
    pub output_pattern: u8,
    pub vectors: [FormatProbeVector; super::IOS_MAXFMT_PARAMS],
}

/// Set this to a live [`FormatProbeSnapshot`] pointer to intercept the next
/// formatted request. The bridge clears it before it parses the call. This
/// symbol exists only when the Dolphin probe feature is enabled.
pub static FORMAT_PROBE_SNAPSHOT: AtomicUsize = AtomicUsize::new(0);

/// Sentinel returned after the one-shot probe captured a formatted request.
pub const FORMAT_PROBE_COMPLETE: i32 = i32::MIN + 101;

pub(super) unsafe fn parse_or_capture(
    call: &FormatCall,
) -> Result<(*mut FormatState, usize, usize, *mut Ioctlv), i32> {
    let snapshot = FORMAT_PROBE_SNAPSHOT.swap(0, Ordering::AcqRel) as *mut FormatProbeSnapshot;
    if snapshot.is_null() {
        return unsafe { super::parse_format_call(call) };
    }
    let result = unsafe { capture_format_probe(call, snapshot) };
    Err(result)
}

unsafe fn capture_format_probe(call: &FormatCall, snapshot: *mut FormatProbeSnapshot) -> i32 {
    if snapshot.is_null() {
        return super::IPC_EINVAL;
    }
    let (state, cnt_in, cnt_io, argv) = match unsafe { super::parse_format_call(call) } {
        Ok(parsed) => parsed,
        Err(error) => return error,
    };
    let total = cnt_in + cnt_io;
    let snapshot_ref = unsafe { &mut *snapshot };
    snapshot_ref.input_count = cnt_in as u32;
    snapshot_ref.output_count = cnt_io as u32;
    snapshot_ref.asynchronous = u32::from(call.asynchronous != 0);
    snapshot_ref.vector_count = total as u32;
    for index in 0..total {
        let entry = unsafe { &*argv.add(index) };
        let sample_len = (entry.len as usize).min(8);
        let record = &mut snapshot_ref.vectors[index];
        record.data_address = entry.data as usize;
        record.length = entry.len;
        record.sample_length = sample_len as u8;
        if sample_len > 0 && !entry.data.is_null() {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    entry.data.cast::<u8>(),
                    record.sample.as_mut_ptr(),
                    sample_len,
                );
            }
        }
    }
    let output_pattern = snapshot_ref.output_pattern;
    if output_pattern != 0 {
        for index in cnt_in..total {
            let entry = unsafe { &*argv.add(index) };
            if !entry.data.is_null() && entry.len != 0 {
                unsafe {
                    core::ptr::write_bytes(
                        entry.data.cast::<u8>(),
                        output_pattern,
                        entry.len as usize,
                    )
                };
            }
        }
    }
    unsafe { super::format_release(super::IPC_OK, state, output_pattern != 0, false) };
    FORMAT_PROBE_COMPLETE
}
