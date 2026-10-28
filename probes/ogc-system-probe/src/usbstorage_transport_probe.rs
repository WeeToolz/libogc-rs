//! Physical USB BOT recovery and suspend/resume checks without sector writes.
use crate::probe_support::ProbeLogger;
use core::mem::MaybeUninit;
use core::ptr;
use ogc_lwp::sys as lwp;
use ogc_usb as usb;
use ogc_usbstorage as storage;

const MAX_SECTOR_BYTES: usize = 16 * 1024;
#[repr(align(32))]
struct Sector([u8; MAX_SECTOR_BYTES]);

// Keep the handle valid even if close fails. Do not lose a live descriptor or
// its recovery state by returning a stack allocation to the caller.
static mut HANDLE: MaybeUninit<storage::UsbStorageHandle> = MaybeUninit::uninit();

fn operation(logger: &mut ProbeLogger, label: &str, result: i32) -> bool {
    logger.note_i32(label, result);
    logger.record(label, result >= 0);
    result >= 0
}

fn capacity(
    logger: &mut ProbeLogger,
    handle: *mut storage::UsbStorageHandle,
    lun: u8,
) -> Option<(u32, u32)> {
    let mut size = 0;
    let mut sectors = 0;
    logger.note("STEP USB ReadCapacity");
    let rc = unsafe { storage::read_capacity(handle, lun, &mut size, &mut sectors) };
    if !operation(logger, "USB ReadCapacity completes", rc) {
        return None;
    }
    logger.note_u32("USB sector bytes", size);
    logger.note_u32("USB capacity value", sectors);
    let valid =
        size >= 512 && size as usize <= MAX_SECTOR_BYTES && size.is_power_of_two() && sectors > 1;
    logger.record("USB capacity fits the aligned probe buffers", valid);
    valid.then_some((size, sectors))
}

fn read_matches(
    logger: &mut ProbeLogger,
    handle: *mut storage::UsbStorageHandle,
    lun: u8,
    expected: &[u8],
    label: &str,
) -> bool {
    let mut observed = Sector([0; MAX_SECTOR_BYTES]);
    // A no-op read cannot pass, regardless of the medium's actual bytes.
    for (byte, prior) in observed.0.iter_mut().zip(expected) {
        *byte = !*prior;
    }
    logger.note("STEP USB read sector 0");
    let rc = unsafe { storage::read(handle, lun, 0, 1, observed.0.as_mut_ptr()) };
    let complete = operation(logger, label, rc);
    let matched = complete && &observed.0[..expected.len()] == expected;
    logger.record("USB sector bytes match the earlier physical read", matched);
    matched
}

fn exercise(
    logger: &mut ProbeLogger,
    handle: *mut storage::UsbStorageHandle,
) -> Option<(u8, u32, u32, Sector)> {
    let count = unsafe { storage::get_max_lun(handle) };
    logger.note_i32("USB LUN count", count);
    logger.record("USB LUN count is valid", (1..=255).contains(&count));
    if !(1..=255).contains(&count) {
        return None;
    }
    let mut selected = None;
    for lun in 0..count as u8 {
        logger.note_u32("STEP USB mount LUN", lun as u32);
        let rc = unsafe { storage::mount_lun(handle, lun) };
        logger.note_i32("USB mount LUN result", rc);
        if rc == storage::USBSTORAGE_INVALID_LUN {
            continue;
        }
        if !operation(logger, "USB mount LUN completes", rc) {
            return None;
        }
        selected = Some(lun);
        break;
    }
    logger.record("USB readable LUN is present", selected.is_some());
    let lun = selected?;
    let (size, sectors) = capacity(logger, handle, lun)?;
    let mut baseline = Sector([0xa5; MAX_SECTOR_BYTES]);
    logger.note("STEP USB baseline read sector 0");
    let rc = unsafe { storage::read(handle, lun, 0, 1, baseline.0.as_mut_ptr()) };
    if !operation(logger, "USB baseline read completes", rc)
        || !read_matches(
            logger,
            handle,
            lun,
            &baseline.0[..size as usize],
            "USB independent read completes",
        )
    {
        return None;
    }
    for cycle in 0..3 {
        logger.note_u32("USB recovery cycle", cycle + 1);
        logger.note("STEP USB BOT reset and both endpoint halt clears");
        let started = lwp::now_ticks();
        let rc = unsafe { storage::reset(handle) };
        let elapsed = lwp::now_ticks().wrapping_sub(started);
        logger.note_u64("USB reset elapsed ticks", elapsed);
        if !operation(logger, "USB BOT reset completes", rc) {
            return None;
        }
        // The production reset requires 60 + 10 + 10 ms of recovery delays.
        // Reject a no-op reset even when the following read would still work.
        let delayed = elapsed >= lwp::TICKS_PER_SECOND * 80 / 1000;
        logger.record("USB BOT reset includes required recovery delays", delayed);
        if !delayed
            || !read_matches(
                logger,
                handle,
                lun,
                &baseline.0[..size as usize],
                "USB read after reset completes",
            )
        {
            return None;
        }
        logger.note("STEP USB suspend");
        let rc = unsafe { storage::suspend(handle) };
        let suspended = unsafe { (*handle).suspended } == 1;
        let accepted = operation(logger, "USB suspend completes", rc);
        logger.record(
            "USB handle records completed suspend",
            accepted && suspended,
        );
        if !accepted || !suspended {
            return None;
        }
        let rc = unsafe { storage::suspend(handle) };
        if !operation(logger, "USB repeated suspend completes", rc) {
            return None;
        }
        let still_suspended = unsafe { (*handle).suspended } == 1;
        logger.record(
            "USB repeated suspend preserves suspended state",
            still_suspended,
        );
        if !still_suspended {
            return None;
        }
        lwp::sleep_ticks(lwp::TICKS_PER_SECOND / 10);
        if !read_matches(
            logger,
            handle,
            lun,
            &baseline.0[..size as usize],
            "USB read automatically resumes transport",
        ) {
            return None;
        }
        let resumed = unsafe { (*handle).suspended } == 0;
        logger.record("USB handle records resumed transport", resumed);
        if !resumed {
            return None;
        }
        let stable = capacity(logger, handle, lun) == Some((size, sectors));
        logger.record("USB capacity is unchanged after reset and resume", stable);
        if !stable {
            return None;
        }
    }
    Some((lun, size, sectors, baseline))
}

fn close(logger: &mut ProbeLogger, handle: *mut storage::UsbStorageHandle) -> bool {
    logger.note("STEP USB close and checked cache flush");
    let rc = unsafe { storage::close(handle) };
    let complete = operation(logger, "USB close completes", rc);
    let released = unsafe {
        (*handle).usb_fd < 0 && (*handle).buffer.is_null() && (*handle).sector_size.is_null()
    };
    logger.record(
        "USB close releases descriptor and transfer buffers",
        released,
    );
    complete && released
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("PHYSICAL USB TRANSPORT: RESET SUSPEND RESUME REOPEN");
    logger.note("No sector writes or filesystem mounts are requested by this suite.");
    logger.note(
        "Keep the USB device attached until the summary. STEP names identify a stalled operation.",
    );
    logger.note_i32("IOS version", ogc_ios::get_version());
    logger.note_i32("IOS revision", ogc_ios::get_revision());
    logger.note_hex32("Hollywood revision", ogc_system::get_hollywood_revision());
    let exclusive = !ogc_fat::is_mounted(c"usb");
    logger.record(
        "USB filesystem is not mounted by the probe logger",
        exclusive,
    );
    if !exclusive {
        logger.note("UNTESTED USB transport: use the default DOL over Wiiload or launch from SD; USB logging owns this device.");
        return;
    }
    if !operation(
        logger,
        "USB host initialization completes",
        usb::initialize(),
    ) || !operation(
        logger,
        "USB storage initialization completes",
        storage::initialize(),
    ) {
        logger.note("UNTESTED USB transport: subsystem initialization failed");
        return;
    }
    let mut entries = [usb::UsbDeviceEntry::default(); 16];
    logger.note("STEP USB enumerate storage candidates");
    let (count, list_rc) = crate::storage_probe::collect_usb_storage_candidates(&mut entries);
    if !operation(logger, "USB device enumeration completes", list_rc) {
        logger.note("UNTESTED USB device cases: enumeration failed");
        operation(
            logger,
            "USB storage deinitialization completes",
            storage::deinitialize(),
        );
        return;
    }
    // SAFETY: This entry point runs once. The synchronous APIs own all accesses
    // during requests; permanent storage also preserves a failed close's state.
    let handle = ptr::addr_of_mut!(HANDLE).cast::<storage::UsbStorageHandle>();
    unsafe { handle.write(storage::UsbStorageHandle::default()) };
    let mut selected = None;
    for entry in entries.iter().take(count) {
        logger.note_i32("STEP USB open candidate", entry.device_id);
        let rc = unsafe { storage::open(handle, entry.device_id, entry.vid, entry.pid) };
        logger.note_i32("USB candidate open result", rc);
        if rc >= 0 {
            selected = Some(*entry);
            break;
        }
    }
    logger.record(
        "physical USB mass-storage device opened",
        selected.is_some(),
    );
    let Some(entry) = selected else {
        logger.note("UNTESTED reset/suspend/resume/reopen: no supported USB storage device opened");
        operation(
            logger,
            "USB storage deinitialization completes",
            storage::deinitialize(),
        );
        return;
    };
    logger.note_i32("USB device ID", entry.device_id);
    logger.note_hex32("USB VID/PID", ((entry.vid as u32) << 16) | entry.pid as u32);
    let result = exercise(logger, handle);
    if !close(logger, handle) {
        logger.note(
            "UNTESTED reopen and deinitialization: close or flush failed; inspect the result and handle state above",
        );
        return;
    }
    if let Some((lun, size, sectors, baseline)) = result {
        logger.note("STEP USB reopen same physical device");
        let rc = unsafe { storage::open(handle, entry.device_id, entry.vid, entry.pid) };
        if operation(logger, "USB reopen completes", rc) {
            let rc = unsafe { storage::mount_lun(handle, lun) };
            if operation(logger, "USB reopened LUN mount completes", rc) {
                let stable = capacity(logger, handle, lun) == Some((size, sectors));
                logger.record("USB reopened capacity matches", stable);
                if stable {
                    read_matches(
                        logger,
                        handle,
                        lun,
                        &baseline.0[..size as usize],
                        "USB reopened physical sector read completes",
                    );
                }
            }
            if !close(logger, handle) {
                return;
            }
        }
    } else {
        logger.note(
            "UNTESTED remaining recovery cycles and reopen: an earlier hardware check failed",
        );
    }
    operation(
        logger,
        "USB storage deinitialization completes",
        storage::deinitialize(),
    );
}
