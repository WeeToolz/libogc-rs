# libaes Review

Date: 2026-10-01

Status: static review of the AES IOS wrapper is complete. No confirmed behavior bug was found in the reviewed implementation. Target hardware was not tested.

## Scope

- `crates/core/ogc-aes-sys`
- C references: `C-sources/libogc/libogc/aes.c` and `C-sources/libogc/gc/ogc/aes.h`

## Reviewed behavior

- Device open, persistent heap setup, error handling, and close.
- Alignment and length validation, command selection, chunk boundaries, ioctlv argument order, and buffer/IV forwarding.

Rust checks null pointers for non-empty operations and expresses the buffer contracts as an unsafe API. Those checks preserve successful C behavior and return the same invalid-argument error for invalid buffer use.

## Validation boundary

`cargo xtask check-group core` passed, and `git diff --check` passed. This is a static source review, not a hardware verification record.
