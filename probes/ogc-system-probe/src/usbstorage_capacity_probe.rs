use crate::probe_support::ProbeLogger;
use ogc_usbstorage::sys;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("USB STORAGE OPEN AND READ CAPACITY POLICY");

    logger.record(
        "open reset decision matches the C USB mode predicate",
        sys::open_requires_bot_reset(0x20, 64)
            && !sys::open_requires_bot_reset(0x20, 512)
            && sys::open_requires_bot_reset(0x1f, 512)
            && !sys::open_requires_bot_reset(-2, 512),
    );

    logger.record(
        "GetMaxLUN supports the full representable C count",
        sys::lun_count_from_reported_max(0) == Some(1)
            && sys::lun_count_from_reported_max(7) == Some(8)
            && sys::lun_count_from_reported_max(8) == Some(9)
            && sys::lun_count_from_reported_max(254) == Some(255)
            && sys::lun_count_from_reported_max(255).is_none(),
    );
    logger.record(
        "GetMaxLUN timeout aborts while other errors default to LUN zero",
        sys::lun_count_after_get_max_lun(Err(sys::USBSTORAGE_ETIMEDOUT))
            == Err(sys::USBSTORAGE_ETIMEDOUT)
            && sys::lun_count_after_get_max_lun(Err(-1)) == Ok(1)
            && sys::lun_count_after_get_max_lun(Ok(0)) == Ok(1),
    );

    let mut short_read = [0xaa; 4];
    let short_read_result = sys::copy_read_result(&[0x11, 0x22, 0x33, 0x44], &mut short_read, 2);
    logger.record(
        "short USB read copies its received prefix before returning residue",
        short_read_result == sys::USBSTORAGE_EDATARESIDUE && short_read == [0x11, 0x22, 0xaa, 0xaa],
    );
    logger.record(
        "zero-byte USB read returns data residue",
        sys::copy_read_result(&[0; 4], &mut [0xaa; 4], 0) == sys::USBSTORAGE_EDATARESIDUE,
    );
    logger.record(
        "zero-byte USB write result returns data residue",
        sys::transfer_result(0, 4) == sys::USBSTORAGE_EDATARESIDUE,
    );
    logger.record(
        "USB data transfer preserves non-timeout transport errors",
        sys::transfer_result(-41, 4) == -41,
    );
    logger.record(
        "BOT retries preserve cycle errors unless reset or transfer times out",
        sys::bot_cycle_retry_error(-41, -42).is_none()
            && sys::bot_cycle_retry_error(-41, sys::USBSTORAGE_ETIMEDOUT)
                == Some(sys::USBSTORAGE_ETIMEDOUT)
            && sys::bot_cycle_retry_error(sys::USBSTORAGE_ETIMEDOUT, 0)
                == Some(sys::USBSTORAGE_ETIMEDOUT),
    );

    let response = [0, 0, 0, 9, 0, 0, 2, 0];
    let cycle_result = sys::bot_csw_cycle_result(sys::USBSTORAGE_OK, 1);
    let accepted = sys::decode_read_capacity(cycle_result, 1, response);
    logger.record(
        "ReadCapacity accepts CSW status and residue, then decodes capacity",
        accepted.result == sys::USBSTORAGE_OK
            && accepted.sector_size == 512
            && accepted.sectors == 9,
    );

    let timeout = sys::bot_csw_cycle_result(sys::USBSTORAGE_ETIMEDOUT, 1);
    let failed = sys::decode_read_capacity(timeout, 1, response);
    logger.record(
        "ReadCapacity preserves a negative BOT cycle result",
        failed.result == sys::USBSTORAGE_ETIMEDOUT
            && failed.sector_size == 0
            && failed.sectors == 0,
    );
}
