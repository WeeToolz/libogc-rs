#![feature(restricted_std)]
mod std_error_probe;
mod std_probe_exit;
use std::process::ExitCode;
fn main() -> ExitCode {
    let mut logger = std_probe_exit::start("wii-std-error-cleanup-probe");
    std_error_probe::run(&mut logger);
    std_probe_exit::finish(logger)
}
