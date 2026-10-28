# ES `es:` filesystem parity pass

Date: 2026-10-02
Baseline: Git HEAD `4c36294`

This pass closes the concrete Wii `es:` filesystem gap identified by the earlier ES reviews while preserving the existing explicit-initialization contract.

## Implemented

- `init()` registers `es:` after `/dev/es` is open; `close()` removes it before closing the descriptor. `reset()` remains separate, matching C.
- One shared path decoder handles `es:<index>`, `es:ID<cid>`, `es:/<title>/<index>`, and `es:/<title>/ID<cid>`, including hexadecimal `0x` prefixes and overflow errno behavior.
- Stored TMD lookup and signature/size validation resolve the requested `TmdContent` by index or content ID.
- Read-only devoptab callbacks implement open, close, read, seek, fstat, and stat. Mutation and directory callbacks remain absent as in C.
- Explicit-title opens allocate 32-byte-aligned ticket-view storage, fetch ticket views, and call `open_title_content`.
- Each file owns an LWP mutex and a lazy 64 KiB/32-byte-aligned bounce buffer. Aligned reads bypass the bounce buffer.
- `Stat` synthesis follows C for inode, device/uid, shared-content group-read permission, size, block count, and block size.
- `read_content` now returns the IOS byte count instead of normalizing every successful read to zero.
- Missing newlib `ENOMEM`, `EROFS`, and `ERANGE` constants were added once to `ogc-sysbase` for reuse.

## DRY decisions

No second device registry, errno layer, stat layout, or mutex implementation was added. ES uses the existing `ogc-sysbase` devoptab/ABI support and `ogc-lwp-sys` mutex implementation. Path parsing, TMD resolution, and stat construction each have one shared implementation used by the relevant callbacks.

## Validation boundary

Focused target tests cover path parsing and stat synthesis. This environment does not provide the Rust/Cargo toolchain, so compilation was not run here. Wii hardware validation is still required for IOS ES reads and explicit-title ticket-view behavior.
