#![feature(restricted_std)]
mod std_panic_probe;
fn main() {
    let mut logger = ogc_system_probe::init_named_probe_logger("wii-std-panic-probe");
    std_panic_probe::run(&mut logger)
}
