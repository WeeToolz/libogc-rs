#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub extern crate self as ogc_es_sys;
pub use ogc_es_sys as sys;

pub use core::primitive::{i32, u8, u16, u32, u64};

pub const ES_EINVAL: i32 = -0x1004;
pub const ES_ENOMEM: i32 = -0x100c;
pub const ES_ENOTINIT: i32 = -0x1100;
pub const ES_EALIGN: i32 = -0x1101;

pub const ES_SIG_RSA4096: u32 = 0x10000;
pub const ES_SIG_RSA2048: u32 = 0x10001;
pub const ES_SIG_ECDSA: u32 = 0x10002;

pub const ES_CERT_RSA4096: u32 = 0;
pub const ES_CERT_RSA2048: u32 = 1;
pub const ES_CERT_ECDSA: u32 = 2;

pub const ES_KEY_NANDFS: u32 = 2;
pub const ES_KEY_COMMON: u32 = 4;
pub const ES_KEY_BACKUP: u32 = 5;
pub const ES_KEY_SDCARD: u32 = 6;
pub const ES_KEY_KOREAN: u32 = 11;

pub const MAX_NUM_TMD_CONTENTS: usize = 512;

/// Restore the HBC AHBPROT check after IOS reload.
///
/// Return 0 when the ES instruction patch succeeds, -1 when the instruction
/// sequence is not found, and -2 when AHBPROT is not available.
#[cfg(target_arch = "powerpc")]
pub fn patch_ahbprot_reset() -> i32 {
    const AHBPROT_STATUS: usize = 0x0D80_0064;
    const MEM2_PROTECTION: usize = 0x0D8B_420A;
    const ES_MODULE_START: usize = 0x939F_0000;
    const ES_MODULE_BYTES: usize = 0x4000;
    const EXPECTED: [u16; 7] = [0x685B, 0x22EC, 0x0052, 0x189B, 0x681B, 0x4698, 0x07DB];

    // These addresses and instruction words match HBC's Wii-only patch.
    unsafe {
        if core::ptr::read_volatile(AHBPROT_STATUS as *const u32) != u32::MAX {
            return -2;
        }
        core::ptr::write_volatile(MEM2_PROTECTION as *mut u16, 2);
        let module = ES_MODULE_START as *mut u16;
        for offset in 0..=ES_MODULE_BYTES / 2 - EXPECTED.len() {
            let candidate = module.add(offset);
            let mut matches = true;
            for (index, expected) in EXPECTED.iter().enumerate() {
                if core::ptr::read_volatile(candidate.add(index)) != *expected {
                    matches = false;
                    break;
                }
            }
            if matches {
                core::ptr::write_volatile(candidate.add(4), 0x23FF);
                ogc_cache::dc_flush_range(candidate.add(4).cast(), 2);
                return 0;
            }
        }
    }
    -1
}

pub type Sigtype = u32;
pub type SigHeader = Sigtype;
pub type SignedBlob = SigHeader;
pub type Sha1 = [u8; 20];
pub type Aeskey = [u8; 16];
pub type SigIssuer = [u8; 0x40];

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SigRsa2048 {
    pub type_: Sigtype,
    pub sig: [u8; 256],
    pub fill: [u8; 60],
}

impl Default for SigRsa2048 {
    fn default() -> Self {
        Self {
            type_: 0,
            sig: [0; 256],
            fill: [0; 60],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SigRsa4096 {
    pub type_: Sigtype,
    pub sig: [u8; 512],
    pub fill: [u8; 60],
}

impl Default for SigRsa4096 {
    fn default() -> Self {
        Self {
            type_: 0,
            sig: [0; 512],
            fill: [0; 60],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SigEcdsa {
    pub type_: Sigtype,
    pub sig: [u8; 60],
    pub fill: [u8; 64],
}

impl Default for SigEcdsa {
    fn default() -> Self {
        Self {
            type_: 0,
            sig: [0; 60],
            fill: [0; 64],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct Tiklimit {
    pub tag: u32,
    pub value: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Tikview {
    pub view: u32,
    pub ticketid: u64,
    pub devicetype: u32,
    pub titleid: u64,
    pub access_mask: u16,
    pub reserved: [u8; 0x3c],
    pub cidx_mask: [u8; 0x40],
    pub padding: u16,
    pub limits: [Tiklimit; 8],
}

impl Default for Tikview {
    fn default() -> Self {
        Self {
            view: 0,
            ticketid: 0,
            devicetype: 0,
            titleid: 0,
            access_mask: 0,
            reserved: [0; 0x3c],
            cidx_mask: [0; 0x40],
            padding: 0,
            limits: [Tiklimit::default(); 8],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Tik {
    pub issuer: SigIssuer,
    pub fill: [u8; 63],
    pub cipher_title_key: Aeskey,
    pub fill2: u8,
    pub ticketid: u64,
    pub devicetype: u32,
    pub titleid: u64,
    pub access_mask: u16,
    pub reserved: [u8; 0x3c],
    pub cidx_mask: [u8; 0x40],
    pub padding: u16,
    pub limits: [Tiklimit; 8],
}

impl Default for Tik {
    fn default() -> Self {
        Self {
            issuer: [0; 0x40],
            fill: [0; 63],
            cipher_title_key: [0; 16],
            fill2: 0,
            ticketid: 0,
            devicetype: 0,
            titleid: 0,
            access_mask: 0,
            reserved: [0; 0x3c],
            cidx_mask: [0; 0x40],
            padding: 0,
            limits: [Tiklimit::default(); 8],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct TmdContent {
    pub cid: u32,
    pub index: u16,
    pub type_: u16,
    pub size: u64,
    pub hash: Sha1,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Tmd {
    pub issuer: SigIssuer,
    pub version: u8,
    pub ca_crl_version: u8,
    pub signer_crl_version: u8,
    pub vwii_title: u8,
    pub sys_version: u64,
    pub title_id: u64,
    pub title_type: u32,
    pub group_id: u16,
    pub zero: u16,
    pub region: u16,
    pub ratings: [u8; 16],
    pub reserved: [u8; 12],
    pub ipc_mask: [u8; 12],
    pub reserved2: [u8; 18],
    pub access_rights: u32,
    pub title_version: u16,
    pub num_contents: u16,
    pub boot_index: u16,
    pub fill3: u16,
    pub contents: [TmdContent; 0],
}

impl Default for Tmd {
    fn default() -> Self {
        Self {
            issuer: [0; 0x40],
            version: 0,
            ca_crl_version: 0,
            signer_crl_version: 0,
            vwii_title: 0,
            sys_version: 0,
            title_id: 0,
            title_type: 0,
            group_id: 0,
            zero: 0,
            region: 0,
            ratings: [0; 16],
            reserved: [0; 12],
            ipc_mask: [0; 12],
            reserved2: [0; 18],
            access_rights: 0,
            title_version: 0,
            num_contents: 0,
            boot_index: 0,
            fill3: 0,
            contents: [],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct TmdViewContent {
    pub cid: u32,
    pub index: u16,
    pub type_: u16,
    pub size: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TmdView {
    pub version: u8,
    pub filler: [u8; 3],
    pub sys_version: u64,
    pub title_id: u64,
    pub title_type: u32,
    pub group_id: u16,
    pub reserved: [u8; 0x3e],
    pub title_version: u16,
    pub num_contents: u16,
    pub contents: [TmdViewContent; 0],
}

impl Default for TmdView {
    fn default() -> Self {
        Self {
            version: 0,
            filler: [0; 3],
            sys_version: 0,
            title_id: 0,
            title_type: 0,
            group_id: 0,
            reserved: [0; 0x3e],
            title_version: 0,
            num_contents: 0,
            contents: [],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct CertHeader {
    pub issuer: SigIssuer,
    pub cert_type: u32,
    pub cert_name: [u8; 64],
    pub cert_id: u32,
}

impl Default for CertHeader {
    fn default() -> Self {
        Self {
            issuer: [0; 0x40],
            cert_type: 0,
            cert_name: [0; 64],
            cert_id: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct CertRsa2048 {
    pub issuer: SigIssuer,
    pub cert_type: u32,
    pub cert_name: [u8; 64],
    pub cert_id: u32,
    pub modulus: [u8; 256],
    pub exponent: u32,
    pub pad: [u8; 0x34],
}

impl Default for CertRsa2048 {
    fn default() -> Self {
        Self {
            issuer: [0; 0x40],
            cert_type: 0,
            cert_name: [0; 64],
            cert_id: 0,
            modulus: [0; 256],
            exponent: 0,
            pad: [0; 0x34],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct CertRsa4096 {
    pub issuer: SigIssuer,
    pub cert_type: u32,
    pub cert_name: [u8; 64],
    pub cert_id: u32,
    pub modulus: [u8; 512],
    pub exponent: u32,
    pub pad: [u8; 0x34],
}

impl Default for CertRsa4096 {
    fn default() -> Self {
        Self {
            issuer: [0; 0x40],
            cert_type: 0,
            cert_name: [0; 64],
            cert_id: 0,
            modulus: [0; 512],
            exponent: 0,
            pad: [0; 0x34],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct CertEcdsa {
    pub issuer: SigIssuer,
    pub cert_type: u32,
    pub cert_name: [u8; 64],
    pub cert_id: u32,
    pub r: [u8; 30],
    pub s: [u8; 30],
    pub pad: [u8; 0x3c],
}

impl Default for CertEcdsa {
    fn default() -> Self {
        Self {
            issuer: [0; 0x40],
            cert_type: 0,
            cert_name: [0; 64],
            cert_id: 0,
            r: [0; 30],
            s: [0; 30],
            pad: [0; 0x3c],
        }
    }
}

#[path = "imp.rs"]
mod imp;

pub use imp::*;
