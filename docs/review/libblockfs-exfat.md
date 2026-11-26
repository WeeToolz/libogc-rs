# Block filesystem and exFAT parity

Status: review incomplete.

The Rust workspace contains `ogc-blockfs` and `ogc-exfat`, but the static
parity ledger did not contain rows for these workspace crates. The exFAT
reader appears to be an added Rust feature with no direct libogc C API
counterpart. The block filesystem service depends on libogc-backed FAT,
filesystem, SD, and exFAT code and therefore needs a production-path review
against those C components.

No source parity conclusion is made in this update. Next, trace each public
block filesystem operation through its Rust backend and compare the matching
libogc C behavior. For exFAT, record the added-feature boundary and compare
shared block-device behavior where the C source provides a contract. Runtime
acceptance must include Dolphin API probes; physical Wii evidence remains a
separate requirement for hardware-dependent behavior.
