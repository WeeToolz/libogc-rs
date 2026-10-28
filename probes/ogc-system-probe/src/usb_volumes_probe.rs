//! Read-only two-media probe. Missing media or fixture bytes fail.
fn verify_half(file: &mut ogc_fat::FatFile, start: u8) -> Result<(), alloc::string::String> {
    let mut bytes = [0; 32];
    let mut offset = 0;
    while offset < bytes.len() {
        let count = file
            .read(&mut bytes[offset..])
            .map_err(|error| alloc::format!("read: {error:?}"))?;
        if count == 0 {
            return Err(alloc::format!("fixture ended at byte {offset}"));
        }
        offset += count;
    }
    for (index, byte) in bytes.into_iter().enumerate() {
        if byte != start + index as u8 {
            return Err(alloc::format!(
                "fixture mismatch at {index}: {byte}, expected {}",
                start + index as u8
            ));
        }
    }
    Ok(())
}

fn probe() -> Result<(), alloc::string::String> {
    for source in [ogc_fat::MountSource::Usb, ogc_fat::MountSource::Usb2] {
        let name = match source {
            ogc_fat::MountSource::Usb => c"usb",
            _ => c"usb2",
        };
        ogc_fat::mount(
            source,
            ogc_fat::MountOptions {
                mount_name: name,
                read_only: true,
            },
        )
        .map_err(|error| alloc::format!("mount {name:?}: {error:?}"))?;
    }
    let mut first = ogc_fat::FatFile::open(c"usb:/hbc-usb-probe.bin")
        .map_err(|error| alloc::format!("open usb: {error:?}"))?;
    let mut second = match ogc_fat::FatFile::open(c"usb2:/hbc-usb-probe.bin") {
        Ok(file) => file,
        Err(error) => {
            let cleanup = first.close();
            return Err(alloc::format!(
                "open usb2: {error:?}; close usb: {cleanup:?}"
            ));
        }
    };
    let reads = (|| {
        verify_half(&mut first, 0)?;
        verify_half(&mut second, 64)?;
        verify_half(&mut first, 32)?;
        verify_half(&mut second, 96)
    })();
    let close_first = first.close();
    let close_second = second.close();
    if close_first.is_err() || close_second.is_err() {
        return Err(alloc::format!(
            "close usb={close_first:?}; close usb2={close_second:?}; reads={reads:?}"
        ));
    }
    reads?;
    for slot in 0..2 {
        let result = ogc_usbstorage_sys::flush_drive(slot);
        if result < 0 {
            return Err(alloc::format!("flush slot {slot}: {result}"));
        }
    }
    ogc_fat::unmount(c"usb").map_err(|error| alloc::format!("unmount usb: {error:?}"))?;
    let mut second = ogc_fat::FatFile::open(c"usb2:/hbc-usb-probe.bin")
        .map_err(|error| alloc::format!("reopen usb2 after usb shutdown: {error:?}"))?;
    let reads = verify_half(&mut second, 64);
    second
        .close()
        .map_err(|error| alloc::format!("close surviving usb2: {error:?}; reads={reads:?}"))?;
    reads
}

pub fn run(logger: &mut crate::probe_support::ProbeLogger) {
    logger.section("TWO INDEPENDENT USB VOLUMES - READ ONLY");
    let operation = probe();
    let cleanup = ogc_fat::shutdown_all();
    logger.note(&alloc::format!(
        "USB operation={operation:?}; cleanup={cleanup:?}"
    ));
    logger.record(
        "two media have distinct fixture bytes and usb2 survives usb unmount",
        operation.is_ok() && cleanup.is_ok(),
    );
}
