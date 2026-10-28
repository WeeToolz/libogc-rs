#![feature(restricted_std)]
mod std_probe_exit;
mod std_usb_probe;
use std::process::ExitCode;
fn main() -> ExitCode {
    let mut logger = std_probe_exit::start("wii-std-usb-probe");
    std_usb_probe::run(&mut logger);
    std_probe_exit::finish(logger)
}
