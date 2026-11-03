use crate::probe_support::ProbeLogger;
use ogc_usbstorage::sys;

#[repr(align(32))]
struct AlignedSector([u8; 512]);

fn has_error<T>(result: ogc_usbstorage::Result<T>, code: i32) -> bool {
    result.is_err_and(|error| error.raw() == code)
}

fn probe_public_facade(logger: &mut ProbeLogger) {
    use ogc_usbstorage as storage;

    logger.section("USB STORAGE SAFE FACADE WITHOUT A DEVICE");
    let mut handle = storage::UsbStorageHandle::default();
    let mut sector = AlignedSector([0xa5; 512]);

    let initialized = storage::try_initialize();
    let init_ok = initialized.is_ok()
        || initialized == Err(storage::ErrorCode::new(storage::USBSTORAGE_EINIT));
    logger.record(
        "safe USB initialize reports success or no-Wii error",
        init_ok,
    );

    let raw_open = unsafe { storage::open(&mut handle, 0, 0x1234, 0x5678) };
    let opened = unsafe { storage::try_open(&mut handle, 0, 0x1234, 0x5678) };
    logger.note_i32("raw USB open status without device", raw_open);
    logger.record(
        "safe USB open preserves the raw no-device error and cleans its handle",
        raw_open < 0
            && has_error(opened, raw_open)
            && handle.usb_fd < 0
            && handle.buffer.is_null()
            && handle.sector_size.is_null(),
    );

    let (closed, reset, max_lun, mounted, suspended, capacity, read, write) = unsafe {
        (
            storage::try_close(core::ptr::null_mut()),
            storage::try_reset(core::ptr::null_mut()),
            storage::try_get_max_lun(core::ptr::null_mut()),
            storage::try_mount_lun(core::ptr::null_mut(), 0),
            storage::try_suspend(core::ptr::null_mut()),
            storage::try_read_capacity(core::ptr::null_mut(), 0),
            storage::try_read(core::ptr::null_mut(), 0, 0, 1, sector.0.as_mut_ptr()),
            storage::try_write(core::ptr::null_mut(), 0, 0, 1, sector.0.as_ptr()),
        )
    };
    logger.record(
        "safe USB close maps null handle to interface error",
        has_error(closed, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "safe USB reset maps null handle to interface error",
        has_error(reset, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "safe USB GetMaxLUN maps null handle to interface error",
        has_error(max_lun, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "safe USB mount maps null handle to interface error",
        has_error(mounted, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "safe USB suspend maps null handle to interface error",
        has_error(suspended, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "safe USB capacity maps null handle to interface error",
        has_error(capacity, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "safe USB read rejects a null handle",
        has_error(read, storage::USBSTORAGE_EINVAL),
    );
    logger.record(
        "safe USB write rejects a null handle",
        has_error(write, storage::USBSTORAGE_EINVAL),
    );

    let default_flush = storage::flush_default();
    let safe_default_flush = storage::try_flush_default();
    logger.record(
        "default USB flush reports no mounted storage",
        default_flush == storage::USBSTORAGE_ENOINTERFACE
            && safe_default_flush == Err(storage::ErrorCode::new(storage::USBSTORAGE_ENOINTERFACE)),
    );

    let raw_dvd = storage::is_dvd();
    let safe_dvd = storage::try_is_dvd();
    logger.record(
        "safe USB DVD query maps the raw no-device result",
        raw_dvd == 0 && safe_dvd == Ok(false),
    );

    let raw_start_stop = unsafe { storage::start_stop(core::ptr::null_mut(), 0, 0, 1, 0) };
    let safe_start_stop = unsafe { storage::try_start_stop(core::ptr::null_mut(), 0, 0, 1, 0) };
    logger.record(
        "safe USB start-stop reports a missing storage interface",
        raw_start_stop == storage::USBSTORAGE_ENOINTERFACE
            && safe_start_stop == Err(storage::ErrorCode::new(storage::USBSTORAGE_ENOINTERFACE)),
    );

    let deinitialized = storage::try_deinitialize();
    logger.record(
        "safe USB deinitialize completes after the no-device probe",
        deinitialized.is_ok()
            && handle.usb_fd < 0
            && handle.buffer.is_null()
            && handle.sector_size.is_null(),
    );
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("USB STORAGE OPEN AND READ CAPACITY POLICY");
    probe_public_facade(logger);

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
