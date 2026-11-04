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

    // A failed open leaves a live, closed handle. These calls exercise the
    // non-null handle validation paths without sending a USB command.
    let closed_max_lun = unsafe { storage::try_get_max_lun(&mut handle) };
    let closed_inquiry = unsafe { sys::inquiry(&mut handle, 0) };
    let closed_mount = unsafe { storage::try_mount_lun(&mut handle, 0) };
    let mut closed_sector_size = 0x1234_5678;
    let mut closed_sectors = 0x8765_4321;
    let closed_capacity = unsafe {
        storage::read_capacity(&mut handle, 0, &mut closed_sector_size, &mut closed_sectors)
    };
    let closed_read = unsafe { storage::try_read(&mut handle, 0, 0, 1, sector.0.as_mut_ptr()) };
    let closed_write = unsafe { storage::try_write(&mut handle, 0, 0, 1, sector.0.as_ptr()) };
    logger.record(
        "closed USB handle reports zero LUNs and rejects LUN zero before transport",
        closed_max_lun == Ok(0)
            && closed_inquiry == storage::USBSTORAGE_EINVAL
            && has_error(closed_mount, storage::USBSTORAGE_EINVAL)
            && closed_capacity == storage::USBSTORAGE_EINVAL
            && closed_sector_size == 0x1234_5678
            && closed_sectors == 0x8765_4321
            && has_error(closed_read, storage::USBSTORAGE_EINVAL)
            && has_error(closed_write, storage::USBSTORAGE_EINVAL)
            && sector.0 == [0xa5; 512]
            && handle.usb_fd < 0
            && handle.buffer.is_null()
            && handle.sector_size.is_null(),
    );
    let closed_close = unsafe { storage::try_close(&mut handle) };
    logger.record(
        "closing an unopened USB handle reports no interface and stays closed",
        has_error(closed_close, storage::USBSTORAGE_ENOINTERFACE)
            && handle.usb_fd < 0
            && handle.buffer.is_null()
            && handle.sector_size.is_null(),
    );

    // SAFETY: A null handle is an accepted error input. The second handle is
    // live and uniquely borrowed; its zero LUN count rejects the request.
    let null_inquiry = unsafe { sys::inquiry(core::ptr::null_mut(), 0) };
    let invalid_lun_inquiry = unsafe { sys::inquiry(&mut handle, 0) };
    logger.record(
        "USB inquiry rejects null and unavailable LUN without changing handle",
        null_inquiry == storage::USBSTORAGE_ENOINTERFACE
            && invalid_lun_inquiry == storage::USBSTORAGE_EINVAL
            && handle.usb_fd < 0
            && handle.buffer.is_null()
            && handle.sector_size.is_null(),
    );

    let (closed, flushed, reset, max_lun, mounted, suspended, capacity, read, write) = unsafe {
        (
            storage::try_close(core::ptr::null_mut()),
            (
                storage::flush(core::ptr::null_mut(), 0),
                storage::try_flush(core::ptr::null_mut(), 0),
            ),
            storage::try_reset(core::ptr::null_mut()),
            storage::try_get_max_lun(core::ptr::null_mut()),
            storage::try_mount_lun(core::ptr::null_mut(), 0),
            storage::try_suspend(core::ptr::null_mut()),
            storage::try_read_capacity(core::ptr::null_mut(), 0),
            storage::try_read(core::ptr::null_mut(), 0, 0, 1, sector.0.as_mut_ptr()),
            storage::try_write(core::ptr::null_mut(), 0, 0, 1, sector.0.as_ptr()),
        )
    };
    // SAFETY: Null handles are documented error inputs. The aligned sector
    // remains live and is large enough for the requested transfer.
    let (raw_close, raw_read, raw_write) = unsafe {
        (
            storage::close(core::ptr::null_mut()),
            storage::read(core::ptr::null_mut(), 0, 0, 1, sector.0.as_mut_ptr()),
            storage::write(core::ptr::null_mut(), 0, 0, 1, sector.0.as_ptr()),
        )
    };
    // SAFETY: Null handles are documented error inputs; the buffer is live
    // and aligned for one sector throughout the calls.
    let (sys_open, sys_close, sys_read, sys_write) = unsafe {
        (
            sys::open(core::ptr::null_mut(), 0, 0x1234, 0x5678),
            sys::close(core::ptr::null_mut()),
            sys::read(core::ptr::null_mut(), 0, 0, 1, sector.0.as_mut_ptr()),
            sys::write(core::ptr::null_mut(), 0, 0, 1, sector.0.as_ptr()),
        )
    };
    logger.record(
        "USB storage sys open rejects a null handle",
        sys_open == storage::USBSTORAGE_ENOINTERFACE,
    );
    logger.record(
        "raw and safe USB close reject null handles",
        raw_close == storage::USBSTORAGE_ENOINTERFACE
            && sys_close == raw_close
            && has_error(closed, storage::USBSTORAGE_ENOINTERFACE),
    );
    logger.record(
        "raw and safe USB flush map null handle to interface error",
        flushed.0 == storage::USBSTORAGE_ENOINTERFACE
            && has_error(flushed.1, storage::USBSTORAGE_ENOINTERFACE),
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
    let mut raw_sector_size = 0x1234_5678;
    let mut raw_sectors = 0x8765_4321;
    // SAFETY: The null handle is a documented error input. Both output
    // pointers refer to live writable values.
    let raw_capacity = unsafe {
        storage::read_capacity(
            core::ptr::null_mut(),
            0,
            &mut raw_sector_size,
            &mut raw_sectors,
        )
    };
    logger.record(
        "raw safe USB capacity rejects a null handle and preserves outputs",
        raw_capacity == storage::USBSTORAGE_ENOINTERFACE
            && raw_sector_size == 0x1234_5678
            && raw_sectors == 0x8765_4321,
    );
    logger.record(
        "raw and safe USB reads reject null handles and preserve output",
        raw_read == storage::USBSTORAGE_EINVAL
            && sys_read == raw_read
            && has_error(read, storage::USBSTORAGE_EINVAL)
            && sector.0 == [0xa5; 512],
    );
    logger.record(
        "raw and safe USB writes reject null handles",
        raw_write == storage::USBSTORAGE_EINVAL
            && sys_write == raw_write
            && has_error(write, storage::USBSTORAGE_EINVAL),
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

fn probe_c_abi_without_device(logger: &mut ProbeLogger) {
    use ogc_usbstorage_sys as storage;

    logger.section("USB STORAGE C ABI WITHOUT A DEVICE");
    let init = storage::c_initialize();
    logger.record(
        "C USB initialize reports success or no-Wii error",
        init == storage::USBSTORAGE_OK || init == storage::USBSTORAGE_EINIT,
    );

    let open = unsafe { storage::c_open(core::ptr::null_mut(), 0, 0x1234, 0x5678) };
    let close = unsafe { storage::c_close(core::ptr::null_mut()) };
    let reset = unsafe { storage::c_reset(core::ptr::null_mut()) };
    let max_lun = unsafe { storage::c_get_max_lun(core::ptr::null_mut()) };
    let mount = unsafe { storage::c_mount_lun(core::ptr::null_mut(), 0) };
    let suspend = unsafe { storage::c_suspend(core::ptr::null_mut()) };
    let start_stop = unsafe { storage::c_start_stop(core::ptr::null_mut(), 0, 0, 1, 0) };
    logger.record(
        "C USB open rejects a null storage handle",
        open == storage::USBSTORAGE_ENOINTERFACE,
    );
    logger.record(
        "C USB close and reset reject null storage handles",
        close == storage::USBSTORAGE_ENOINTERFACE && reset == storage::USBSTORAGE_ENOINTERFACE,
    );
    logger.record(
        "C USB GetMaxLUN and mount reject null storage handles",
        max_lun == storage::USBSTORAGE_ENOINTERFACE && mount == storage::USBSTORAGE_ENOINTERFACE,
    );
    logger.record(
        "C USB suspend and start-stop reject null storage handles",
        suspend == storage::USBSTORAGE_ENOINTERFACE
            && start_stop == storage::USBSTORAGE_ENOINTERFACE,
    );

    let inquiry = unsafe { storage::c_inquiry(core::ptr::null_mut(), 0) };
    let mut sector_size = 0x1234_5678;
    let mut sectors = 0x8765_4321;
    let capacity = unsafe {
        storage::c_read_capacity(core::ptr::null_mut(), 0, &mut sector_size, &mut sectors)
    };
    logger.record(
        "C USB inquiry rejects a null storage handle",
        inquiry == storage::USBSTORAGE_ENOINTERFACE,
    );
    logger.record(
        "C USB capacity rejects null handle and preserves outputs",
        capacity == storage::USBSTORAGE_ENOINTERFACE
            && sector_size == 0x1234_5678
            && sectors == 0x8765_4321,
    );

    let mut sector = AlignedSector([0xa5; 512]);
    let read = unsafe { storage::c_read(core::ptr::null_mut(), 0, 0, 1, sector.0.as_mut_ptr()) };
    let write = unsafe { storage::c_write(core::ptr::null_mut(), 0, 0, 1, sector.0.as_ptr()) };
    logger.record(
        "C USB read and write reject null storage handles",
        read == storage::USBSTORAGE_EINVAL && write == storage::USBSTORAGE_EINVAL,
    );

    let dvd = storage::c_is_dvd();
    logger.record("C USB DVD query reports no mounted DVD", dvd == 0);
    let ioctl = unsafe { storage::ioctl(0, core::ptr::null_mut()) };
    let bridge = unsafe { storage::__ogc_usbstorage_ioctl_bridge(0, core::ptr::null_mut()) };
    logger.record(
        "USB ioctl and C bridge reject an unmounted request",
        ioctl == -1 && bridge == -1,
    );
    logger.record(
        "USB async control cleanup counter remains clear",
        storage::async_control_cleanup_failures() == 0,
    );

    let raw_flush = unsafe { storage::flush(core::ptr::null_mut(), 0) };
    let raw_flush_default = storage::flush_default();
    let raw_reset = unsafe { storage::reset(core::ptr::null_mut()) };
    let raw_mount = unsafe { storage::mount_lun(core::ptr::null_mut(), 0) };
    let raw_suspend = unsafe { storage::suspend(core::ptr::null_mut()) };
    let raw_start_stop = unsafe { storage::start_stop(core::ptr::null_mut(), 0, 0, 1, 0) };
    logger.record(
        "raw USB flush and default flush report no interface",
        raw_flush == storage::USBSTORAGE_ENOINTERFACE
            && raw_flush_default == storage::USBSTORAGE_ENOINTERFACE,
    );
    logger.record(
        "USB drive flush reports absent media and rejects invalid slots",
        storage::flush_drive(0) == storage::USBSTORAGE_ENOINTERFACE
            && storage::flush_drive(1) == storage::USBSTORAGE_ENOINTERFACE
            && storage::flush_drive(2) == storage::USBSTORAGE_EINVAL,
    );
    logger.record(
        "raw USB reset, mount, suspend, and start-stop reject null handles",
        raw_reset == storage::USBSTORAGE_ENOINTERFACE
            && raw_mount == storage::USBSTORAGE_ENOINTERFACE
            && raw_suspend == storage::USBSTORAGE_ENOINTERFACE
            && raw_start_stop == storage::USBSTORAGE_ENOINTERFACE,
    );

    let mut raw_sector_size = 0x1234_5678;
    let mut raw_sectors = 0x8765_4321;
    let raw_capacity = unsafe {
        storage::read_capacity(
            core::ptr::null_mut(),
            0,
            &mut raw_sector_size,
            &mut raw_sectors,
        )
    };
    logger.record(
        "raw USB capacity preserves outputs when the handle is null",
        raw_capacity == storage::USBSTORAGE_ENOINTERFACE
            && raw_sector_size == 0x1234_5678
            && raw_sectors == 0x8765_4321,
    );

    let raw_dvd = storage::is_dvd();
    logger.record("raw USB DVD query reports no mounted DVD", raw_dvd == 0);
    let Some(clear_status) = storage::USB_STORAGE_INTERFACE.clear_status else {
        logger.record("USB disc-interface clear-status callback exists", false);
        return;
    };
    let clear_status = unsafe { clear_status() };
    logger.record(
        "USB disc-interface clear-status callback succeeds",
        clear_status,
    );

    storage::c_deinitialize();
    logger.record(
        "C USB deinitialize records its result for C callers",
        storage::c_last_deinitialize_status() == storage::USBSTORAGE_OK
            && storage::last_deinitialize_status() == storage::USBSTORAGE_OK,
    );
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("USB STORAGE OPEN AND READ CAPACITY POLICY");
    probe_public_facade(logger);
    probe_c_abi_without_device(logger);

    logger.record(
        "open reset decision matches the C USB mode predicate",
        sys::open_requires_bot_reset(0x20, 64)
            && !sys::open_requires_bot_reset(0x20, 512)
            && sys::open_requires_bot_reset(0x1f, 512)
            && !sys::open_requires_bot_reset(-2, 512),
    );
    logger.record(
        "USB idle remount uses a strict sixty-second boundary",
        !sys::idle_remount_required(160, 100)
            && sys::idle_remount_required(161, 100)
            && sys::idle_remount_required(100, 0),
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
    logger.record(
        "USB read/write cycle status preserves C result rules",
        sys::read_write_cycle_result(0, 1) == 0
            && sys::read_write_cycle_result(1, 1) == sys::USBSTORAGE_ESTATUS
            && sys::read_write_cycle_result(1, 0) == 1
            && sys::read_write_cycle_result(-41, 1) == -41,
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
