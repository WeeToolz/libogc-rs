use crate::probe_support::ProbeLogger;
use ogc_wiisd::WII_SD_INTERFACE;

#[repr(align(32))]
struct Sector([u8; 512]);

pub fn run(logger: &mut ProbeLogger) {
    logger.section("WII SD DISC INTERFACE");
    logger.note(
        "Write callback test uses an unaligned zero-sector request; it sends no data command.",
    );

    let Some(startup) = WII_SD_INTERFACE.startup else {
        logger.record("Wii SD startup callback exists", false);
        return;
    };
    let Some(is_inserted) = WII_SD_INTERFACE.is_inserted else {
        logger.record("Wii SD inserted callback exists", false);
        return;
    };
    let Some(read_sectors) = WII_SD_INTERFACE.read_sectors else {
        logger.record("Wii SD read callback exists", false);
        return;
    };
    let Some(write_sectors) = WII_SD_INTERFACE.write_sectors else {
        logger.record("Wii SD write callback exists", false);
        return;
    };
    let Some(shutdown) = WII_SD_INTERFACE.shutdown else {
        logger.record("Wii SD shutdown callback exists", false);
        return;
    };

    // SAFETY: These callbacks operate on the registered Wii SD interface.
    let started = unsafe { startup() };
    // SAFETY: Startup completed or reported that the SD interface is unavailable.
    let inserted = unsafe { is_inserted() };
    logger.note_i32("Wii SD startup result", started as i32);
    logger.note_i32("Wii SD inserted result", inserted as i32);
    logger.record(
        "Wii SD startup and inserted state agree",
        started == inserted,
    );

    let mut sector = Sector([0xa5; 512]);
    let before_read = sector.0;
    // SAFETY: The buffer is aligned and holds one sector. If no card is ready,
    // production returns before it accesses the buffer.
    let read = unsafe { read_sectors(0, 1, sector.0.as_mut_ptr().cast()) };
    let data_changed = sector.0 != before_read;
    logger.note_i32("Wii SD sector zero read result", read as i32);
    logger.record(
        "Wii SD read result matches card presence and updates its output",
        if inserted {
            read && data_changed
        } else {
            !read && !data_changed
        },
    );

    #[cfg(feature = "dolphin-automation")]
    {
        logger.note("Dolphin-only write check uses sector zero and restores its original bytes.");
        let original = Sector(sector.0);
        let mut pattern = Sector([0; 512]);
        for (index, byte) in pattern.0.iter_mut().enumerate() {
            *byte = (index as u8).wrapping_mul(37).wrapping_add(0x5b);
        }

        // SAFETY: This Dolphin-only check uses the private virtual SD card.
        // It saves sector zero first and restores it after the round trip.
        let wrote_pattern =
            inserted && read && unsafe { write_sectors(0, 1, pattern.0.as_ptr().cast()) };
        logger.record("Wii SD one-sector write completes", wrote_pattern);

        let mut observed_pattern = Sector([0; 512]);
        // SAFETY: The destination is one aligned sector and the write ran on
        // Dolphin's private virtual SD card.
        let pattern_read =
            wrote_pattern && unsafe { read_sectors(0, 1, observed_pattern.0.as_mut_ptr().cast()) };
        logger.record(
            "Wii SD read returns the written sector bytes",
            pattern_read && observed_pattern.0 == pattern.0,
        );

        // SAFETY: Restore the saved sector even when the pattern write or
        // read-back check failed. This DOL uses a disposable Dolphin profile.
        let restored_write =
            inserted && read && unsafe { write_sectors(0, 1, original.0.as_ptr().cast()) };
        let mut restored_sector = Sector([0; 512]);
        // SAFETY: The destination is one aligned sector on the private card.
        let restored_read =
            restored_write && unsafe { read_sectors(0, 1, restored_sector.0.as_mut_ptr().cast()) };
        logger.record(
            "Wii SD original sector bytes are restored",
            restored_read && restored_sector.0 == original.0,
        );
    }

    let before_write = sector.0;
    // SAFETY: The aligned array remains live. The offset makes the pointer
    // unaligned, so a zero-sector request exits before any SDIO write command.
    let write = unsafe { write_sectors(0, 0, sector.0.as_ptr().add(1).cast()) };
    logger.note_i32("Wii SD zero-sector write result", write as i32);
    logger.record(
        "Wii SD zero-sector write does not change the buffer",
        write == inserted && sector.0 == before_write,
    );

    // SAFETY: The registered interface owns its open state and shutdown closes it.
    let stopped = unsafe { shutdown() };
    // SAFETY: The shutdown callback has returned.
    let inserted_after_shutdown = unsafe { is_inserted() };
    // SAFETY: The second shutdown checks that a closed interface rejects shutdown.
    let stopped_again = unsafe { shutdown() };
    logger.record(
        "Wii SD shutdown releases the interface and is one-shot",
        stopped == started && !inserted_after_shutdown && !stopped_again,
    );

    // SAFETY: Startup after a successful shutdown tests reinitialization.
    let restarted = unsafe { startup() };
    // SAFETY: Query the restarted interface state.
    let inserted_after_restart = unsafe { is_inserted() };
    // SAFETY: Close a successful restart before the probe exits.
    let stopped_after_restart = unsafe { shutdown() };
    logger.record(
        "Wii SD restart restores and then releases interface state",
        restarted == started
            && inserted_after_restart == inserted
            && stopped_after_restart == restarted,
    );
}
