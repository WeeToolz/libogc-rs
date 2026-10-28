//! Mass storage needs a usable storage host, independently of HID startup.

use core::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProbeStage {
    NotStarted,
    Host,
    Initialize,
    Enumerate,
    Open,
    MaxLun,
    MountLun,
    Reset,
    Ready,
    DeviceChange,
    AttachFinish,
    TransferRead,
    TransferWrite,
    Flush,
    AsyncCleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsbProbeStatus {
    pub stage: ProbeStage,
    pub result: i32,
    pub candidates: u32,
    pub sense: u32,
}

pub(crate) fn enumeration_failure_stage(
    failure: Option<(ogc_usb_sys::UsbV5FailureStage, i32)>,
) -> ProbeStage {
    match failure.map(|(stage, _)| stage) {
        Some(ogc_usb_sys::UsbV5FailureStage::DeviceChange) => ProbeStage::DeviceChange,
        Some(ogc_usb_sys::UsbV5FailureStage::AttachFinish) => ProbeStage::AttachFinish,
        Some(ogc_usb_sys::UsbV5FailureStage::None) | None => ProbeStage::Enumerate,
    }
}

static SENSE: AtomicU32 = AtomicU32::new(0);

pub(crate) fn record_sense(key: u8, asc: u8, ascq: u8) {
    SENSE.store(
        (u32::from(key) << 16) | (u32::from(asc) << 8) | u32::from(ascq),
        Ordering::Release,
    );
}

static STAGE: AtomicU8 = AtomicU8::new(0);
static RESULT: AtomicI32 = AtomicI32::new(0);
static CANDIDATES: AtomicU32 = AtomicU32::new(0);

pub(crate) fn record(stage: ProbeStage, result: i32) {
    RESULT.store(result, Ordering::Relaxed);
    STAGE.store(stage as u8, Ordering::Release);
}

pub(crate) fn record_candidates(count: usize) {
    CANDIDATES.store(count as u32, Ordering::Release);
}

/// Read diagnostic state after a serialized storage probe finishes. This is
/// not a media-presence or I/O-success API; use the real operation result.
pub fn probe_status() -> UsbProbeStatus {
    let stage = match STAGE.load(Ordering::Acquire) {
        1 => ProbeStage::Host,
        2 => ProbeStage::Initialize,
        3 => ProbeStage::Enumerate,
        4 => ProbeStage::Open,
        5 => ProbeStage::MaxLun,
        6 => ProbeStage::MountLun,
        7 => ProbeStage::Reset,
        8 => ProbeStage::Ready,
        9 => ProbeStage::DeviceChange,
        10 => ProbeStage::AttachFinish,
        11 => ProbeStage::TransferRead,
        12 => ProbeStage::TransferWrite,
        13 => ProbeStage::Flush,
        14 => ProbeStage::AsyncCleanup,
        _ => ProbeStage::NotStarted,
    };
    UsbProbeStatus {
        stage,
        result: RESULT.load(Ordering::Relaxed),
        candidates: CANDIDATES.load(Ordering::Acquire),
        sense: SENSE.load(Ordering::Acquire),
    }
}

pub(crate) fn start_storage(host: impl FnOnce() -> i32, storage: impl FnOnce() -> i32) -> bool {
    record_candidates(0);
    let result = host();
    record(ProbeStage::Host, result);
    if result < 0 {
        return false;
    }
    let result = storage();
    record(ProbeStage::Initialize, result);
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_checks_both_operation_results() {
        assert!(start_storage(|| 0, || 0));
        assert!(!start_storage(|| -8, || panic!("host startup failed")));
        assert!(!start_storage(|| 0, || -10009));
        assert_eq!(
            probe_status(),
            UsbProbeStatus {
                stage: ProbeStage::Initialize,
                result: -10009,
                candidates: 0,
                sense: 0
            }
        );
        record_candidates(3);
        record(ProbeStage::Open, -8);
        assert_eq!(
            probe_status(),
            UsbProbeStatus {
                stage: ProbeStage::Open,
                result: -8,
                candidates: 3,
                sense: 0
            }
        );
    }

    #[test]
    fn enumeration_keeps_the_underlying_v5_watcher_failure_stage() {
        assert_eq!(
            enumeration_failure_stage(Some((ogc_usb_sys::UsbV5FailureStage::DeviceChange, -22,))),
            ProbeStage::DeviceChange
        );
        assert_eq!(
            enumeration_failure_stage(Some((ogc_usb_sys::UsbV5FailureStage::AttachFinish, -22,))),
            ProbeStage::AttachFinish
        );
        assert_eq!(enumeration_failure_stage(None), ProbeStage::Enumerate);
    }
}
