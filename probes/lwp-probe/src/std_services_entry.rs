#![feature(restricted_std)]
mod futex_probe;
mod std_allocator_probe;
mod std_diagnostic_probe;
mod std_probe_exit;
mod std_sync_probe;
use std::process::ExitCode;

fn main() -> ExitCode {
    let missing = std_diagnostic_probe::before_console();
    let mut logger = std_probe_exit::start("wii-std-services-probe");
    std_diagnostic_probe::run(&mut logger, missing);
    std_allocator_probe::run(&mut logger);
    futex_probe::run(&mut logger);
    std_sync_probe::run(&mut logger);
    std_probe_exit::finish(logger)
}
