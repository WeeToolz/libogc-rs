#![no_std]

//! C-compatible libmad API backed by the pure Rust `libmad-rs` codec.

extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub mod imp;

pub use imp::*;
