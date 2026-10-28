# Static C Parity Review: SHA

Date: 2026-10-01

Status: static review complete; no confirmed SHA result or context mismatch found. Rust replaces the Wii IOS SHA service with a software implementation, which is an intentional backend and platform difference. No hardware verification was performed.

## Scope

- Public API, SHA-1 compression, padding, lifecycle state, and context types in `crates/core/ogc-sha/src/lib.rs`.
- C API declarations in `../../C-sources/libogc/gc/ogc/sha.h`.
- C implementation in `../../C-sources/libogc/libogc/sha.c`.
- This review compares the public SHA behavior and data layout. It does not verify the `/dev/sha` IOS implementation on hardware.

## Compared behavior

- `SHA_MSGBLOCK_SIZE` is `0x10000` in C and Rust (`sha.h:41`; Rust `src/lib.rs:10`). `ShaContext` has five 32-bit state words followed by upper and lower 32-bit bit length in both languages (`sha.h:43-48`; Rust `src/lib.rs:18-24`). The Rust 32-byte-aligned wrapper provides the alignment required by C for contexts passed to IOS (`sha.c:128-146`; Rust `src/lib.rs:57-61,69-82`).
- Rust initializes the standard SHA-1 IV and implements the 80-round compression function (`src/lib.rs:32-55,85-134`). It uses the SHA-1 big-endian word schedule, round constants, rotations, and wrapping additions. The known `abc` digest in its existing test matches the standard vector (`src/lib.rs:13-16,271-279`).
- `calculate` processes a full message from the SHA-1 initial state. `input` processes complete 64-byte blocks and advances the context bit count. `finalize` processes the final full blocks, pads the remaining bytes, appends the total bit count in big-endian order, and emits five big-endian state words (`src/lib.rs:136-168,194-244`). These correspond to C `SHA_Calculate`, `SHA_Input`, and `SHA_Finalize`, whose commands are ResetContext, AddData, and FinalizeHash (`sha.c:41-46,128-179`).
- Both implementations reject zero-length input for calculate, input, and finalize; both require `SHA_Input` data length to be a multiple of 64 bytes (`sha.c:149-169`; Rust `src/lib.rs:194-216,228-244`). Rust returns `-4` for a non-multiple input length, matching C. `initialize_context` initializes the same state IV (`sha.c:128-147`; Rust `src/lib.rs:32-45,202-208`).
- C chunks aligned input at `SHA_MSGBLOCK_SIZE` and copies unaligned input through a 4 KiB aligned buffer (`sha.c:47-95`). Rust hashes slices directly and processes all available full blocks (`src/lib.rs:136-141,143-168`). This changes buffering and alignment requirements but preserves the SHA-1 result for the same byte sequence.

## Intentional Rust differences

- C `SHA_Init` opens `/dev/sha` and creates an IOS heap; `SHA_Close` closes the IOS handle (`sha.c:97-126`). Rust has no IOS handle or heap. Its `init` checks the software compressor with `abc`, and `close` toggles a local lifecycle flag (`src/lib.rs:170-192`). Rust can therefore provide SHA-1 without the Wii IOS service and on PowerPC targets where the C header is excluded by `HW_RVL` (`sha.h:33`). This changes service-availability errors and platform coverage, not the digest algorithm.
- C requires a 32-byte-aligned context and final digest, and returns `-4` for bad alignment (`sha.c:128-169`). Rust encodes context alignment in `AlignedShaContext`, uses a 20-byte digest array, and safely accepts arbitrary input slice alignment (`src/lib.rs:57-82,194-244`). These typed APIs prevent invalid pointer/size use and avoid the C alignment restriction.
- The Rust context APIs use mutable references where SHA input updates context state; C declares context pointers `const` even though the IOS vector passes context back as an output vector (`sha.c:55-58,149-159`). The Rust mutability makes the state update explicit.
- Rust's operations require a successful `init` and return `-1` before initialization or after close. The C calls depend on valid `__sha_fd` and `__sha_hid`; service or allocation failures are reported from IOS and can use different negative codes (`sha.c:97-114,128-169`). This is part of the backend replacement described above.

## Findings

No confirmed mismatch was found in the SHA-1 digest construction, context layout, input block-size rule, empty-input handling, or ordinary lifecycle success/failure state. Existing target-gated tests cover context layout, known digest vectors, incremental hashing, and lifecycle calls (`src/lib.rs:246-306`). Tests were not run as part of this static review. No hardware verification was performed.
