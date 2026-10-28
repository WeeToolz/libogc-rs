#![no_std]
#![cfg(not(gamecube))]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub use ogc_ios_sys as sys;
pub use sys::{
    _ios_initialize_subsystems as initialize_subsystems, _ios_launch_new_ios as launch_new_ios,
    _ios_load_startup_ios as load_startup_ios, _ios_shutdown_subsystems as shutdown_subsystems,
    IOS_EBADVERSION, IOS_EINVAL, IOS_EMISMATCH, IOS_ETOOMANYVIEWS, get_preferred_version,
    get_revision, get_revision_major, get_revision_minor, get_version, reload_ios,
};

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = initialize_subsystems as fn() -> i32;
        let _ = shutdown_subsystems as fn() -> i32;
        let _ = load_startup_ios as fn() -> i32;
        let _ = launch_new_ios as fn(i32) -> i32;
        let _ = get_preferred_version as fn() -> i32;
        let _ = reload_ios as fn(i32) -> i32;
        let _ = get_version as fn() -> i32;
        let _ = get_revision as fn() -> i32;
        let _ = get_revision_major as fn() -> i32;
        let _ = get_revision_minor as fn() -> i32;
    }
}
