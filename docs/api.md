# API Guide

This workspace is organized around Rust-facing APIs. The examples below prefer
typed wrappers and `Result`-returning helpers; raw pointer calls remain available
only in low-level modules where the hardware boundary requires them.

The snippets below are runtime snippets for the Wii PowerPC target. Check
or build crates with `--target ./targets/powerpc-nintendo-none.json`; they are not
examples of supported host execution.

## Video and console

```rust
use ogc_video as video;

video::init();
let con = video::console_get_default();
video::console_select(con);
let _ = video::console_write(b"hello from Rust\n");
video::wait_vsync();
```

## USB and storage

```rust
use ogc_usb as usb;

usb::try_initialize()?;
let mut fd = usb::open_device_handle(0, 0x057e, 0x0306)?;
usb::close_device_handle(&mut fd)?;
```

```rust
use ogc_usbstorage as storage;

storage::try_initialize()?;
let mut handle = storage::sys::UsbStorageHandle::default();
unsafe {
    storage::try_open(&mut handle, 0, 0, 0)?;
    storage::try_mount_lun(&mut handle, 0)?;
}
```

## Networking

```rust
use lwip_sys as lwip;

let fd = lwip::socket(lwip::AF_INET, lwip::SOCK_STREAM, lwip::IPPROTO_TCP);
if fd >= 0 {
    let _ = lwip::close(fd);
}
```

## Keyboard input

```rust
use wiikeyboard_sys as keyboard;

let _ = keyboard::initialize();
let _ = keyboard::init(None);
let _ = keyboard::scan();
let _ = keyboard::flush_events();
```

## Wii Remote and WPAD

```rust
use wpad_sys as wpad;

wpad::init();
wpad::scan_pads();
let _ = wpad::read_pending(0, core::ptr::null_mut());
```

## Audio

```rust
use asnd_sys as asnd;

asnd::init();
let _ = asnd::get_time();
let _ = asnd::get_first_unused_voice();
```

```rust
use aesnd;

aesnd::init();
let _voice = aesnd::allocate_voice(None);
```

## MOD playback

```rust
use modplay_capi as modplay;

let mut player = core::mem::MaybeUninit::<modplay::ModPlay>::zeroed();
unsafe {
    modplay::mod_play_init(player.as_mut_ptr());
    let _ = modplay::mod_play_set_frequency(player.as_mut_ptr(), 48_000);
}
```

## SMB

```rust
use tinysmb_sys as smb;

let mut conn: smb::SmbConn = 0;
unsafe {
    let _ = smb::init(core::ptr::null(), core::ptr::null(), core::ptr::null(), core::ptr::null());
    let _ = smb::connect(&mut conn, core::ptr::null(), core::ptr::null(), core::ptr::null(), core::ptr::null());
}
```

## Notes

- Prefer the wrapper crates for Rust code; they expose typed errors and `Result`
  helpers where fallible hardware calls are modeled.
- The `*_sys` crates are low-level implementation boundaries, not exported low-level Rust API
  low-level adapters.
- If a function takes a raw pointer, treat it as `unsafe` unless the docs for
  that crate state a safe invariant.
