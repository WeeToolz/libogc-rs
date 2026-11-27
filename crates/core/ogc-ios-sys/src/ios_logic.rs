pub(super) const IOS_MIN_VERSION: i32 = 28;
pub(super) const IOS_MAX_VERSION: i32 = 61;
pub(super) const MAX_TICKET_VIEWS: usize = 4;
pub(super) const MAX_STORED_TMD_SIZE: u32 = 4096;

#[repr(align(32))]
pub(super) struct Aligned<T>(pub T);

pub(super) const fn ipc_control_ready(control: u32, mask: u32) -> bool {
    control & mask != 0
}

pub(super) const fn stored_tmd_size_is_supported(size: u32) -> bool {
    size <= MAX_STORED_TMD_SIZE
}

pub(super) const fn initialization_result(es_result: i32, stm_result: i32) -> i32 {
    let mut result = 0;
    if es_result < 0 {
        result = es_result;
    }
    if stm_result < 0 {
        result = stm_result;
    }
    result
}

pub(super) const fn update_error(previous: i32, result: i32) -> i32 {
    if result < 0 { result } else { previous }
}

pub(super) const fn preferred_version_from_title(title_id: u64) -> Option<i32> {
    let title_type = (title_id >> 32) as u32;
    let version = title_id as u32 as i32;
    if title_type == 1 && version >= IOS_MIN_VERSION && version <= IOS_MAX_VERSION {
        Some(version)
    } else {
        None
    }
}

pub(super) const fn ticket_view_count_is_supported(count: u32) -> bool {
    (count as usize) <= MAX_TICKET_VIEWS
}

pub(super) fn finish_ios_replacement(
    new_version: i32,
    requested_version: i32,
    mismatch_result: i32,
    finish: impl FnOnce(),
) -> i32 {
    finish();
    if new_version == requested_version {
        new_version
    } else {
        mismatch_result
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IOS_MAX_VERSION, IOS_MIN_VERSION, MAX_STORED_TMD_SIZE, MAX_TICKET_VIEWS,
        finish_ios_replacement, initialization_result, preferred_version_from_title,
        stored_tmd_size_is_supported, ticket_view_count_is_supported, update_error,
    };

    #[test]
    fn ios_buffers_have_32_byte_alignment_for_packed_data() {
        let mut buffer = super::Aligned([0u8; 4]);
        assert_eq!(core::mem::align_of_val(&buffer), 32);
        assert_eq!(buffer.0.as_mut_ptr() as usize % 32, 0);
    }

    #[test]
    fn ipc_control_accepts_either_ready_bit() {
        let mask = 2 | 4;
        assert!(!super::ipc_control_ready(0, mask));
        assert!(!super::ipc_control_ready(8, mask));
        assert!(super::ipc_control_ready(2, mask));
        assert!(super::ipc_control_ready(4, mask));
        assert!(super::ipc_control_ready(6, mask));
    }

    #[test]
    fn initialization_attempts_both_subsystems_and_returns_last_error() {
        assert_eq!(initialization_result(-10, 0), -10);
        assert_eq!(initialization_result(0, -20), -20);
        assert_eq!(initialization_result(-10, -20), -20);
        assert_eq!(initialization_result(0, 0), 0);
    }

    #[test]
    fn reload_keeps_the_most_recent_error() {
        assert_eq!(update_error(0, -10), -10);
        assert_eq!(update_error(-10, 0), -10);
        assert_eq!(update_error(-10, -20), -20);
        // C keeps the launch failure when closing ES also fails during cleanup.
        assert_eq!(update_error(-30, -20), -20);
    }

    #[test]
    fn preferred_ios_version_uses_c_supported_range() {
        assert_eq!(
            preferred_version_from_title((1u64 << 32) | (IOS_MIN_VERSION - 1) as u64),
            None
        );
        assert_eq!(
            preferred_version_from_title((1u64 << 32) | IOS_MIN_VERSION as u64),
            Some(IOS_MIN_VERSION)
        );
        assert_eq!(
            preferred_version_from_title((1u64 << 32) | IOS_MAX_VERSION as u64),
            Some(IOS_MAX_VERSION)
        );
        assert_eq!(
            preferred_version_from_title((1u64 << 32) | (IOS_MAX_VERSION + 1) as u64),
            None
        );
        assert_eq!(
            preferred_version_from_title((1u64 << 32) | 0x1000_003a),
            None
        );
        assert_eq!(preferred_version_from_title((2u64 << 32) | 58), None);
    }

    #[test]
    fn zero_ticket_views_remain_valid_and_excess_views_fail() {
        assert!(ticket_view_count_is_supported(0));
        assert!(ticket_view_count_is_supported(MAX_TICKET_VIEWS as u32));
        assert!(!ticket_view_count_is_supported(MAX_TICKET_VIEWS as u32 + 1));
    }

    #[test]
    fn replacement_finishes_before_reporting_version_mismatch() {
        let finished = core::cell::Cell::new(false);
        let mismatch = -0x3102;
        let result = finish_ios_replacement(59, 58, mismatch, || finished.set(true));
        assert!(finished.get());
        assert_eq!(result, mismatch);
    }

    #[test]
    fn replacement_finishes_before_returning_matching_version() {
        let finished = core::cell::Cell::new(false);
        let result = finish_ios_replacement(58, 58, -0x3102, || finished.set(true));
        assert!(finished.get());
        assert_eq!(result, 58);
    }

    #[test]
    fn preferred_scan_keeps_zero_size_tmd_calls_within_c_limit() {
        assert!(stored_tmd_size_is_supported(0));
        assert!(stored_tmd_size_is_supported(MAX_STORED_TMD_SIZE));
        assert!(!stored_tmd_size_is_supported(MAX_STORED_TMD_SIZE + 1));
    }
}
