//! IOS USB v5 host selection and layouts from libogc usb.c.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostKind {
    Vendor,
    Hid,
}

/// USB v5 IOS buffers must be cache-line aligned and wholly inside MEM2.
pub(crate) fn host_buffer_valid(address: usize, length: usize) -> bool {
    let physical = address & 0x3fff_ffff;
    length != 0
        && address & 31 == 0
        && physical >= 0x1000_0000
        && physical
            .checked_add(length)
            .is_some_and(|end| end <= 0x1400_0000)
}

/// Preserve the IOS byte count; reserve a full private cache line when needed.
pub(crate) fn needs_bounce(address: usize, length: usize) -> bool {
    length != 0 && (!host_buffer_valid(address, length) || !length.is_multiple_of(32))
}

pub(crate) fn bulk_header(id: i32, endpoint: u8, length: u16) -> [u8; 64] {
    let mut header = [0; 64];
    header[0..4].copy_from_slice(&id.to_be_bytes());
    header[12..14].copy_from_slice(&length.to_be_bytes());
    header[18] = endpoint;
    header
}

pub(crate) fn iso_header(
    id: i32,
    endpoint: u8,
    packets: u8,
    packet_sizes: usize,
    data: usize,
) -> [u8; 64] {
    let mut header = [0; 64];
    header[0..4].copy_from_slice(&id.to_ne_bytes());
    header[8..12].copy_from_slice(&(data as u32).to_ne_bytes());
    header[12..16].copy_from_slice(&(packet_sizes as u32).to_ne_bytes());
    header[16] = packets;
    header[17] = endpoint;
    header
}

pub(crate) const fn iso_vector_counts(direction_in: bool) -> (usize, usize) {
    if direction_in { (1, 3) } else { (2, 2) }
}

pub(crate) fn control_header(
    id: i32,
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
) -> [u8; 64] {
    let mut header = [0; 64];
    header[0..4].copy_from_slice(&id.to_be_bytes());
    header[8] = request_type;
    header[9] = request;
    header[10..12].copy_from_slice(&value.to_be_bytes());
    header[12..14].copy_from_slice(&index.to_be_bytes());
    header[14..16].copy_from_slice(&length.to_be_bytes());
    header
}

pub(crate) fn check_host_version(
    host: HostKind,
    vendor_v5: bool,
    mut request: impl FnMut(i32) -> (i32, u32),
) -> bool {
    // HID request 6 is GETVERSION on V4, but ATTACHFINISH on V5. A
    // validated V5 vendor host lets us avoid that legacy probe on V5 HID.
    if host == HostKind::Hid && !vendor_v5 && request(6).0 == 0x0004_0001 {
        return false;
    }
    let (result, version) = request(0);
    result == 0 && version == 0x0005_0001
}

pub(crate) fn host_for_device(id: i32) -> Option<HostKind> {
    if (0..32).contains(&id) {
        None
    } else if id < 0 {
        Some(HostKind::Vendor)
    } else {
        Some(HostKind::Hid)
    }
}

pub(crate) fn use_legacy_list(vendor: bool, _hid: bool) -> bool {
    !vendor
}

pub(crate) fn timeout_elapsed(start: u64, now: u64, timeout: u64) -> bool {
    now.wrapping_sub(start) >= timeout
}

pub(crate) fn descriptor_layout(host: HostKind) -> (usize, usize) {
    match host {
        HostKind::Vendor => (0xc0, 20),
        HostKind::Hid => (0x60, 36),
    }
}

pub(crate) fn interrupt_header(id: i32, endpoint: u8, length: u16, data: u32) -> [u8; 64] {
    let mut header = [0; 64];
    header[0..4].copy_from_slice(&id.to_ne_bytes());
    if host_for_device(id) == Some(HostKind::Hid) {
        // HID interrupt messages use a direction flag in the union at offset 8.
        header[8..12].copy_from_slice(&u32::from(endpoint & 0x80 == 0).to_ne_bytes());
    } else {
        header[8..12].copy_from_slice(&data.to_ne_bytes());
        header[12..14].copy_from_slice(&length.to_ne_bytes());
        header[14] = endpoint;
    }
    header
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_short_transfers_use_private_mem2_cache_lines() {
        for length in [1, 8, 13, 31] {
            assert!(needs_bounce(0x9000_4000, length));
        }
        assert!(needs_bounce(0x8000_4000, 512));
        assert!(!needs_bounce(0x9000_4000, 512));
        assert!(!needs_bounce(0, 0));
    }

    #[test]
    fn headers_match_powerpc_c_usb_message_layout() {
        let bulk = bulk_header(-32, 0x81, 0x1234);
        assert_eq!(
            &bulk[..20],
            &[
                255, 255, 255, 224, 0, 0, 0, 0, 0, 0, 0, 0, 0x12, 0x34, 0, 0, 0, 0, 0x81, 0
            ]
        );
        let control = control_header(-32, 0xa1, 0xfe, 0x1234, 0x5678, 1);
        assert_eq!(
            &control[..20],
            &[
                255, 255, 255, 224, 0, 0, 0, 0, 0xa1, 0xfe, 0x12, 0x34, 0x56, 0x78, 0, 1, 0, 0, 0,
                0
            ]
        );
        assert!(bulk[20..].iter().all(|byte| *byte == 0));
        assert!(control[20..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn iso_header_matches_powerpc_c_union_layout() {
        let header = iso_header(-32, 0x81, 3, 0x9000_2000, 0x9000_3000);
        assert_eq!(&header[0..4], &(-32i32).to_ne_bytes());
        assert_eq!(&header[8..12], &0x9000_3000u32.to_ne_bytes());
        assert_eq!(&header[12..16], &0x9000_2000u32.to_ne_bytes());
        assert_eq!(&header[16..18], &[3, 0x81]);
        assert!(header[18..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn iso_vector_counts_match_c_ioctlv_contract() {
        assert_eq!(iso_vector_counts(true), (1, 3));
        assert_eq!(iso_vector_counts(false), (2, 2));
    }

    #[test]
    fn watcher_buffer_rejects_mem1_and_out_of_bounds_mem2() {
        assert!(!host_buffer_valid(0x8000_4000, 0x180));
        assert!(host_buffer_valid(0x9000_4000, 0x180));
        assert!(host_buffer_valid(0xd000_4000, 0x180));
        assert!(!host_buffer_valid(0x9000_4001, 0x180));
        assert!(!host_buffer_valid(0x93ff_ffe0, 0x180));
        assert!(!host_buffer_valid(0x9400_0000, 0x180));
        assert!(!host_buffer_valid(0x9000_4000, 0));
    }
    #[test]
    fn v5_hid_version_check_does_not_complete_attachment_before_enumeration() {
        let mut requests = std::vec::Vec::new();
        assert!(check_host_version(HostKind::Hid, true, |request| {
            requests.push(request);
            (0, 0x0005_0001)
        }));
        assert_eq!(requests, [0]);
    }

    #[test]
    fn legacy_hid_version_check_remains_available_without_a_v5_vendor_host() {
        let mut requests = std::vec::Vec::new();
        assert!(!check_host_version(HostKind::Hid, false, |request| {
            requests.push(request);
            (0x0004_0001, 0)
        }));
        assert_eq!(requests, [6]);
    }

    #[test]
    fn vendor_version_check_uses_v5_request_and_rejects_invalid_replies() {
        let mut requests = std::vec::Vec::new();
        assert!(check_host_version(HostKind::Vendor, false, |request| {
            requests.push(request);
            (0, 0x0005_0001)
        }));
        assert_eq!(requests, [0]);
        assert!(!check_host_version(HostKind::Hid, true, |_| (
            -1,
            0x0005_0001
        )));
        assert!(!check_host_version(HostKind::Hid, true, |_| (
            0,
            0x0004_0001
        )));
    }

    #[test]
    fn device_list_falls_back_to_oh0_without_vendor_host() {
        assert!(!use_legacy_list(true, false));
        assert!(!use_legacy_list(true, true));
        assert!(use_legacy_list(false, true));
        assert!(use_legacy_list(false, false));
        assert_eq!(host_for_device(31), None);
        assert_eq!(host_for_device(32), Some(HostKind::Hid));
        assert_eq!(host_for_device(-1), Some(HostKind::Vendor));
    }

    #[test]
    fn watcher_timeout_uses_wrapping_elapsed_time() {
        assert!(!timeout_elapsed(100, 199, 100));
        assert!(timeout_elapsed(100, 200, 100));
        assert!(timeout_elapsed(100, 201, 100));
        assert!(timeout_elapsed(u64::MAX - 10, 9, 20));
    }
    #[test]
    fn hid_and_vendor_descriptor_offsets_match_c_reference() {
        assert_eq!(descriptor_layout(HostKind::Hid), (0x60, 36));
        assert_eq!(descriptor_layout(HostKind::Vendor), (0xc0, 20));
    }
    #[test]
    fn hid_interrupt_header_uses_direction_not_a_pointer() {
        let input = interrupt_header(32, 0x81, 8, 0x12345678);
        assert_eq!(&input[8..12], &0u32.to_ne_bytes());
        let output = interrupt_header(32, 0x01, 8, 0x12345678);
        assert_eq!(&output[8..12], &1u32.to_ne_bytes());
        let vendor = interrupt_header(-32, 0x81, 8, 0x12345678);
        assert_eq!(&vendor[8..12], &0x12345678u32.to_ne_bytes());
        assert_eq!(&vendor[12..14], &8u16.to_ne_bytes());
        assert_eq!(vendor[14], 0x81);
    }
}
