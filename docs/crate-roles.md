# Crate roles and ownership

`config/crate-roles.csv` is the machine-readable source of truth. `cargo xtask check-roles` verifies that every package under `crates/`, `probes/`, and `xtask/` has exactly one entry and that path and package name agree.

| Group | Ownership |
| --- | --- |
| `core` | core runtime, IRQ, IPC, IOS, LWP driver/facades, timers, callbacks, system configuration, and Tuxedo compatibility |
| `storage` | DI, DVD, CARD, USB storage, SD Gecko, Wii SD, filesystems, FAT, and ISO9660 |
| `input` | SI, USB HID, Bluetooth, lwBT, WPAD, keyboard, and normalized input |
| `network` | socket ownership, wireless discovery, lwIP compatibility, and SMB |
| `audio` | audio hardware, DSP, ARAM, ASND, AESND, MP3, MOD, and decoders |
| `debug` | report routing, video console, USB Gecko, libdb, GDB, and system font |
| `hardware` | shared EXI, GX, and geometry facilities not owned by one service group |
| `abi` | base ABI definitions and ABI probe |
| `probes` | target-only diagnostics and Wii sample programs, including `lwp-probe` and the `examples` package |

Milestone 1 ownership is explicit:

- `ogc-lwp-driver` owns contexts, scheduling, synchronization, timers, deferred callbacks, and trace state;
- `ogc-lwp-sys` owns compatibility return-code and type forwarding only;
- `ogc-lwp` owns safe handles only;
- `tuxedo-sys` owns ABI object-to-LWP-handle translation only;
- `ogc-ipc` owns IOS requests and invokes the scheduler wake surface on completion;
- `lwp-probe` owns test orchestration and logging, never replacement runtime behavior.

Use:

```sh
cargo xtask check-group core
cargo xtask check-group storage
cargo xtask check-group input
cargo xtask check-group network
cargo xtask check-group audio
cargo xtask check-group debug
cargo xtask check-all-runtime
cargo xtask verify-complete
```
