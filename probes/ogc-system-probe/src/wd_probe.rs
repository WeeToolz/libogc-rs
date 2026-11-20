use crate::probe_support::ProbeLogger;
use ogc_wd as wd;

const BUFFER_SIZE: usize = 256;

fn scan_count_is_valid(buffer: &[u8; BUFFER_SIZE]) -> bool {
    let count = usize::from(u16::from_be_bytes([buffer[0], buffer[1]]));
    buffer[..2] != [0xa5; 2]
        && count <= buffer.len().saturating_sub(2) / core::mem::size_of::<wd::BssDescriptor>()
}

fn scan_parameters() -> wd::ScanParameters {
    wd::default_scan_parameters()
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("WIRELESS DRIVER API PROBE");

    let lock_id = wd::ncd_lock_wireless_driver();
    logger.record("WD driver lock succeeds", lock_id >= 0);
    let unlock = if lock_id >= 0 {
        wd::ncd_unlock_wireless_driver(lock_id)
    } else {
        u32::MAX
    };
    logger.record("WD driver unlock succeeds", unlock == 0);

    let init = wd::try_wd_init(wd::AOSSAP_SCAN as u8);
    logger.record("WD fallible init succeeds", init.is_ok());

    let mut parameters = scan_parameters();
    let mut raw_buffer = [0xa5; BUFFER_SIZE];
    // SAFETY: The parameters and buffer are live, writable, and large enough for the call.
    let raw_scan =
        unsafe { wd::scan(&mut parameters, raw_buffer.as_mut_ptr(), BUFFER_SIZE as u16) };
    logger.record(
        "WD raw scan succeeds with a valid result count",
        raw_scan == 0 && scan_count_is_valid(&raw_buffer),
    );

    let mut result_parameters = scan_parameters();
    let mut result_buffer = [0xa5; BUFFER_SIZE];
    let scan_result = wd::scan_result(&mut result_parameters, &mut result_buffer);
    logger.record(
        "WD scan result wrapper succeeds",
        scan_result.is_ok() && scan_count_is_valid(&result_buffer),
    );

    let deinit = wd::try_wd_deinit();
    logger.record("WD fallible deinit succeeds", deinit.is_ok());

    let mut once_parameters = scan_parameters();
    let mut once_buffer = [0xa5; BUFFER_SIZE];
    // SAFETY: The parameters and buffer are live, writable, and large enough for the call.
    let scan_once = unsafe {
        wd::scan_once(
            &mut once_parameters,
            once_buffer.as_mut_ptr(),
            BUFFER_SIZE as u16,
        )
    };
    logger.record(
        "WD raw scan once succeeds",
        scan_once == 0 && scan_count_is_valid(&once_buffer),
    );

    let mut once_result_parameters = scan_parameters();
    let mut once_result_buffer = [0xa5; BUFFER_SIZE];
    let scan_once_result =
        wd::scan_once_result(&mut once_result_parameters, &mut once_result_buffer);
    logger.record(
        "WD scan once result wrapper succeeds",
        scan_once_result.is_ok() && scan_count_is_valid(&once_result_buffer),
    );

    let mut detailed_parameters = scan_parameters();
    let mut detailed_buffer = [0xa5; BUFFER_SIZE];
    // SAFETY: The parameters and buffer are live, writable, and large enough for the call.
    let detailed = unsafe {
        wd::scan_once_detailed(
            &mut detailed_parameters,
            detailed_buffer.as_mut_ptr(),
            BUFFER_SIZE as u16,
        )
    };
    logger.record(
        "WD detailed scan once succeeds",
        detailed.is_ok() && scan_count_is_valid(&detailed_buffer),
    );

    let mut detailed_result_parameters = scan_parameters();
    let mut detailed_result_buffer = [0xa5; BUFFER_SIZE];
    let detailed_result =
        wd::scan_once_detailed_result(&mut detailed_result_parameters, &mut detailed_result_buffer);
    logger.record(
        "WD detailed scan result wrapper succeeds",
        detailed_result.is_ok() && scan_count_is_valid(&detailed_result_buffer),
    );

    let mut info = wd::WdInfo::default();
    // SAFETY: `info` is a valid, aligned, writable output structure.
    let info_status = unsafe { wd::get_info(&mut info) };
    logger.record(
        "WD raw info query returns a valid record",
        info_status == 0 && info.initialized <= 1 && info.channel <= 14,
    );
    let info_result = wd::get_info_result();
    logger.record(
        "WD info result wrapper returns a valid record",
        info_result.is_ok_and(|info| info.initialized <= 1 && info.channel <= 14),
    );

    let radio_levels = [
        (0xaa, wd::WD_SIGNAL_WEAK),
        (0xab, wd::WD_SIGNAL_FAIR),
        (0xb4, wd::WD_SIGNAL_FAIR),
        (0xb5, wd::WD_SIGNAL_NORMAL),
        (0xc3, wd::WD_SIGNAL_NORMAL),
        (0xc4, wd::WD_SIGNAL_STRONG),
        (u16::MAX, wd::WD_SIGNAL_STRONG),
    ];
    let radio_levels_match = radio_levels.into_iter().all(|(rssi, expected)| {
        let descriptor = wd::BssDescriptor {
            rssi,
            ..wd::BssDescriptor::default()
        };
        // SAFETY: `descriptor` is a valid live descriptor.
        unsafe { wd::get_radio_level(&descriptor) == expected }
    });
    logger.record(
        "WD radio level buckets match RSSI boundaries",
        radio_levels_match,
    );
}
