#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(not(gamecube))]
pub use ogc_aes_sys as aes;
pub use ogc_aram as aram;
pub use ogc_audio as audio;
#[cfg(not(gamecube))]
pub use ogc_bt_sync as bt_sync;
pub use ogc_cache as cache;
pub use ogc_card as card;
#[cfg(not(gamecube))]
pub use ogc_conf as conf;
pub use ogc_core as core;
#[cfg(not(gamecube))]
pub use ogc_di as di;
pub use ogc_dsp as dsp;
pub use ogc_dvd as dvd;
#[cfg(not(gamecube))]
pub use ogc_es as es;
pub use ogc_exi as exi;
#[cfg(not(gamecube))]
pub use ogc_filesystem as filesystem;
#[cfg(not(gamecube))]
pub use ogc_filesystem as isfs;
pub use ogc_gcsd as gcsd;
pub use ogc_gu as gu;
pub use ogc_gx as gx;
pub use ogc_input_sys as input;
#[cfg(not(gamecube))]
pub use ogc_ios as ios;
#[cfg(not(gamecube))]
pub use ogc_ipc as ipc;
pub use ogc_irq as irq;
pub use ogc_lwp as lwp;
#[cfg(not(gamecube))]
pub use ogc_network as network;
#[cfg(not(gamecube))]
pub use ogc_sha as sha;
pub use ogc_si as si;
#[cfg(not(gamecube))]
pub use ogc_stm as stm;
pub use ogc_sys as sys;
pub use ogc_system as system;
#[cfg(not(gamecube))]
pub use ogc_usb as usb;
pub use ogc_usbgecko as usbgecko;
#[cfg(not(gamecube))]
pub use ogc_usbstorage as usbstorage;
pub use ogc_video as video;
#[cfg(not(gamecube))]
pub use ogc_wd as wd;
#[cfg(not(gamecube))]
pub use ogc_wiilaunch as wiilaunch;
#[cfg(not(gamecube))]
pub use ogc_wiisd as wiisd;

pub mod cast;
pub mod color;
pub use color::*;

pub struct TargetConfig;

impl TargetConfig {
    pub const ARCH: &'static str = "powerpc-eabi";
    pub const BYTE_ORDER: u32 = sys::TARGET_BYTE_ORDER;
}
