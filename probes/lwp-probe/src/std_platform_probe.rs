//! Public std APIs must report native Wii limits through their actual adapters.
use ogc_system_probe::probe_support::ProbeLogger;
use std::io::Read;

pub fn run(logger: &mut ProbeLogger) {
    logger.record(
        "stdin reports unsupported input",
        std::io::stdin()
            .read(&mut [0; 1])
            .is_err_and(|error| error.kind() == std::io::ErrorKind::Unsupported),
    );
    logger.record(
        "child process creation reports Unsupported",
        std::process::Command::new("wii-std-no-process-service")
            .spawn()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::Unsupported),
    );
    logger.record(
        "captured child output reports Unsupported",
        std::process::Command::new("wii-std-no-process-service")
            .output()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::Unsupported),
    );
    logger.record(
        "backtrace capture reports Unsupported",
        std::backtrace::Backtrace::force_capture().status()
            == std::backtrace::BacktraceStatus::Unsupported,
    );
    logger.record(
        "platform constants describe the PowerPC bare-metal target",
        std::env::consts::ARCH == "powerpc"
            && std::env::consts::OS == "none"
            && std::env::consts::FAMILY.is_empty()
            && std::env::consts::EXE_SUFFIX == ".dol"
            && std::env::consts::EXE_EXTENSION == "dol"
            && std::env::consts::DLL_SUFFIX.is_empty(),
    );
}
