//! Shared production code for the Letter Flip game and hardware probe.
#![no_std]

#[cfg(target_arch = "powerpc")]
pub mod card_storage;
pub mod card_storage_status;
pub mod engine;
pub mod font;
pub mod narration;
pub mod render;
pub mod save;
pub mod session;
pub mod settings;
#[cfg(target_arch = "powerpc")]
pub mod sound;
pub mod speech;
pub mod tone;
#[cfg(target_arch = "powerpc")]
pub mod voice;
