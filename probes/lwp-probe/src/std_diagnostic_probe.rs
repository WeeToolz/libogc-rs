//! Production stdout/stderr checks shared by standalone and combined probes.
use ogc_system_probe::probe_support::ProbeLogger;
use std::io::{self, Write};

pub fn before_console() -> io::Result<()> {
    std::io::stderr().write_all(b"before console\n")
}

pub fn run(logger: &mut ProbeLogger, missing: io::Result<()>) {
    logger.section("STD DIAGNOSTIC TRANSPORT");
    logger.record(
        "missing console returns ENODEV",
        missing.is_err_and(|e| e.raw_os_error() == Some(19)),
    );
    logger.record(
        "diagnostic failure remains visible",
        ogc_std_runtime::diagnostic_error() == Some(19),
    );
    let mut column = 0;
    let mut row = 0;
    ogc_video::console_get_position(&mut column, &mut row);
    let stdout = std::io::stdout().write(b"51a7");
    // Stdout is line buffered. Verify the transport after its real flush.
    let stdout_flushed = std::io::stdout().flush();
    let mut after_column = 0;
    let mut after_row = 0;
    ogc_video::console_get_position(&mut after_column, &mut after_row);
    logger.record(
        "std stdout updates the real console cursor",
        stdout.is_ok_and(|count| count == 4)
            && stdout_flushed.is_ok()
            && after_column == column + 4
            && after_row == row,
    );
    ogc_video::console_get_position(&mut column, &mut row);
    let stderr = std::io::stderr().write(b"7257");
    ogc_video::console_get_position(&mut after_column, &mut after_row);
    logger.record(
        "std stderr updates the real console cursor",
        stderr.is_ok_and(|count| count == 4) && after_column == column + 4 && after_row == row,
    );
    logger.record(
        "std diagnostic flush completes",
        std::io::stderr().flush().is_ok(),
    );
    logger.record(
        "invalid byte pointer rejected",
        unsafe { ogc_std_runtime::__wii_std_output(core::ptr::null(), 1, false) } == -22,
    );
}
