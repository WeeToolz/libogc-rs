# `gc-elf` independent source audit

- Reviewer: `/root/reviewer_5`
- Implementer: extraction in commit `530d086` (author `notOrrytrout`)
- Scope: `DolProfile::LegacyElf2Dol` compared directly with the GameCube tools C converter. `DolProfile::Libogc` is a Rust-only profile in this source scope; no C counterpart was identified.
- C source: `../../C-sources/gamecube-tools/elftool/elf2dol.c`
- Rust source: `crates/tools/gc-elf/src/lib.rs`
- Validation: static source inspection only. No builds, tests, or hardware verification were performed.

## Confirmed mismatch: initialized DATA segment BSS tails

In C `read_elf_segments`, executable PT_LOAD segments add their uninitialized tail when `memsz > filesz` by calling `add_bss(map, paddr + filesz, memsz - filesz)` (`elf2dol.c:220-243`). For non-executable segments, C calls `add_bss` only when `filesz == 0`; if `filesz > 0`, it records a DATA segment and does not add the trailing `memsz - filesz` range (`elf2dol.c:251-272`). The DOL header is serialized with the resulting BSS fields (`elf2dol.c:370-415`).

Rust `parse_elf` preserves `file_size` and `memory_size` for each segment (`crates/tools/gc-elf/src/lib.rs:151-192`). In `pack_legacy`, it adds a BSS range for every segment with `memory_size > file_size` before classifying the segment as TEXT, DATA, or pure BSS (`:214-235`). `set_bss` writes the range into the DOL header (`:280,358-379`). Thus a non-executable PT_LOAD segment with `filesz > 0` and `memsz > filesz` yields a nonempty BSS range in Rust, while C emits the initialized DATA segment and leaves this tail out of BSS.

This is a concrete observable difference in the profile named `LegacyElf2Dol`. It may be an intentional correction to C's behavior, but that intent must be decided by the implementation owner; parity with the C converter is not achieved for this case. Keep `gc-elf` open as `finding`.

## Other behavior relevant to disposition

- C `add_bss` establishes the first BSS range and only extends it when the next range begins exactly at the current end (`elf2dol.c:149-162`). Rust `set_bss` sorts ranges and merges them into a single enclosing range, while returning an error for a gap when `strict_contiguous` is true (`crates/tools/gc-elf/src/lib.rs:358-379`). Disjoint, overlapping, or out-of-order ranges can therefore produce different output or errors. This is a separate behavior difference for implementer disposition.
- Rust validates file bounds and address arithmetic (`crates/tools/gc-elf/src/lib.rs:166-185`); C relies on file operations and unchecked 32-bit address arithmetic. These checks are safer and are not recorded as bugs.
- For ordinary file-backed segments, the inspected paths agree on physical addresses, TEXT/DATA classification, TEXT before DATA ordering, and 64-byte file alignment (`elf2dol.c:220-281,301-334`; `crates/tools/gc-elf/src/lib.rs:178-181,214-273`).

An earlier grouped report in `docs/review/libgc-tools.md` described the same BSS case. This note records an independent recheck against the source and does not replace or edit that report.

## Additional comparison beyond the BSS finding

### No usable loadable segments: Rust rejects an input that C converts

C rejects an absent program-header table (`elf2dol.c:202-210`) but does not require any PT_LOAD entry after reading a valid table. It skips empty PT_LOAD entries where `memsz == 0` (`:220-280`). `map_dol` then assigns dummy TEXT and DATA offsets when the corresponding section counts are zero (`:324-333`), and `write_dol` emits the zero-initialized DOL header (`:461-468`, `:370-415`). Therefore, a valid ELF header and program-header table with no PT_LOAD segments, or only zero-memory PT_LOAD entries, completes successfully and produces a DOL with the dummy section offsets.

Rust also skips zero-memory PT_LOAD entries (`crates/tools/gc-elf/src/lib.rs:151-165`), but then returns `ElfError("ELF image has no loadable segments")` when the segment list is empty (`:194-195`). This is an additional observable `LegacyElf2Dol` input/return behavior difference. It may be a deliberate validation improvement, but the legacy profile does not match C for this case.

## Areas checked with no additional mismatch found for nonempty supported inputs

- Header validation checks relevant to C are present: magic, ELF32 class, big-endian encoding, ident version, ELF version, executable type, PowerPC machine, nonzero entry point, nonzero program-header count/offset, and 32-byte program-header entries (`elf2dol.c:176-210`; `gc-elf/src/lib.rs:105-149`). Rust also checks the file and address ranges before use; those are safer rejection paths already noted above.
- Both implementations ignore non-PT_LOAD headers, skip zero-memory PT_LOAD entries, classify any `PF_X` segment as TEXT, and classify remaining file-backed segments as DATA; non-executable zero-file segments are BSS (`elf2dol.c:220-281`; `gc-elf/src/lib.rs:151-192,214-253`). The initialized DATA BSS-tail difference is recorded earlier.
- For nonempty segment sets that fit the DOL slot limits, each implementation preserves program-header order within TEXT and DATA groups and writes TEXT before DATA. Both use physical addresses and put entry into the DOL entry field (`elf2dol.c:197,220-281,301-334`; `gc-elf/src/lib.rs:128-130,178-192,214-273`). Both use a zeroed 0x100-byte header, write big-endian fields at DOL header offsets, align file sections to 64 bytes, and write initialized segment bytes from `p_offset` (`elf2dol.c:90-102,301-334,370-415,461-468`; `gc-elf/src/lib.rs:8-10,50-81,203-282`). The prior BSS range accumulation difference still applies independently.
- Both reject too many TEXT or DATA sections, although their diagnostic paths differ (`elf2dol.c:244-246,265-267`; `gc-elf/src/lib.rs:237-249`). C reports errors to stderr and exits; Rust reports `ElfError` through `Result`. These are different interfaces, not separate semantic findings for the conversion result.
- `convert_elf_file` is a Rust library function, not a CLI equivalent. It creates missing output parent directories (`gc-elf/src/lib.rs:93-102`), while C `fopen(dol, "wb")` fails if the parent directory does not exist (`elf2dol.c:370-381`). This is a file-wrapper behavior difference; the Rust crate exposes no direct option parser/CLI counterpart, so no CLI parity claim is made here.

## Fix handoff — legacy ELF to DOL parity

Implementer: `worker_c`. Independent reviewer: `/root` (accepted for the two listed fixes).

The `LegacyElf2Dol` profile now matches the two confirmed cases above. For initialized non-executable DATA segments, it no longer adds the `memsz - filesz` tail to DOL BSS. TEXT segment tails and non-executable segments with `filesz == 0` continue to contribute BSS as in C. When parsing produces no nonempty PT_LOAD entries, the legacy profile now emits the zeroed 0x100-byte DOL header and assigns the dummy TEXT and DATA file offsets `0x100`, as C does.

Regression coverage checks that the existing initialized DATA segment bytes are preserved while its BSS tail is omitted, and that inputs with no PT_LOAD entries or only zero-memory PT_LOAD entries produce the dummy-offset DOL. The Rust-only `Libogc` profile keeps its existing empty-image rejection.

At this intermediate handoff, the distinct disjoint/overlapping/out-of-order BSS behavior was still open. The later legacy BSS accumulation fix below supersedes that remaining source mismatch; independent review status is tracked separately. No hardware verification applies to this host-side converter.

Validation: `cargo fmt -p gc-elf` and `cargo test -p gc-elf` passed (5 tests). `git diff --check` passed. The test suite ran on the host.

## Independent re-review

Reviewer: `/root`. Implementer: `worker_c`, commit `d1079cf`.

The `LegacyElf2Dol` changes match the inspected C paths. C adds BSS for executable tails and non-executable segments only when `filesz == 0`; Rust now follows that rule. C accepts a valid program-header table with no nonempty `PT_LOAD` entry and writes dummy TEXT/DATA offsets at `0x100`; Rust now does the same in the legacy profile while keeping the separate `Libogc` profile's validation.

The regression tests cover initialized DATA tails, no `PT_LOAD` entries, and only zero-memory `PT_LOAD` entries. I found no mismatch in these two fixes. At this review stage the separate disjoint/overlapping BSS range behavior remained open; the later legacy BSS accumulation fix below supersedes it.


## 2026-10-01 legacy BSS accumulation parity fix

The remaining `add_bss` mismatch is resolved in the current `LegacyElf2Dol` path. BSS candidates are now examined in original program-header order instead of TEXT-then-DATA packing order. The first eligible range becomes the DOL BSS range. A later range extends it only when that range starts exactly at the current end; disjoint, overlapping, and out-of-order ranges are ignored, matching the current C `add_bss` implementation. The Rust-only `Libogc` profile keeps its existing merged-range behavior.

Focused host-side tests cover adjacent extension, disjoint ranges, overlaps, and out-of-order ranges. This is a host converter change; no hardware verification applies.

## Independent re-review — latest legacy BSS accumulation fix

- Result: **PASSED**
- Reviewer: `/root/reviewer_f`
- Implementer: `worker_c`
- Fix commit: `01e61ca` (`Apply USB and ELF parity fixes`), which contains the latest change to `gc-elf`.
- Scope: the prior initialized-DATA-tail and empty-image fixes, plus legacy BSS accumulation, checked against the complete `LegacyElf2Dol` segment read, DOL mapping, serialization, and Rust-only profile paths.
- C inspected: `../../C-sources/gamecube-tools/elftool/elf2dol.c`, especially `add_bss` (149–162), `read_elf_segments` (220–281), `map_dol` (301–334), and `write_dol` (370–415).
- Rust inspected: `crates/tools/gc-elf/src/lib.rs`, especially `parse_elf` (151–200), `pack_legacy` (203–270), `pack_libogc` (272–343), `legacy_bss_range` / `set_legacy_bss` / `set_bss` (345–396), and regression tests (402–516).

### C comparison and result

`parse_elf` preserves the nonempty PT_LOAD entries in program-header order for `LegacyElf2Dol`; sorting is limited to `Libogc` (151–200). This is required because C calls `add_bss` inside its single program-header loop (220–278). `legacy_bss_range` walks that same original segment order (348–370), ignores the same entries that do not add BSS—memory size not greater than file size, and initialized non-executable DATA—and computes the candidate start and end using physical addresses (178–180, 350–361). For executable TEXT tails it uses `paddr + filesz`; for non-executable segments with zero file size it uses `paddr`, matching C’s calls at 241–243 and 255–258.

The accumulator matches `add_bss`: the first candidate initializes the range; each later candidate changes the range only when its start equals the current end; disjoint, overlapping, and out-of-order ranges are ignored (C 149–162; Rust 363–369). This behavior now covers BSS candidates that occur among TEXT and DATA headers in their original order, even though `pack_legacy` emits TEXT slots before DATA slots (Rust 213–221, 267). C likewise gathers ranges in program-header order, then lays out TEXT before DATA (C 220–282, 301–334). Rust serializes the resulting BSS address and size at the DOL header offsets (Rust 63–80), while C writes the same accumulated, big-endian header fields with the header (C 400–402). Initialized non-executable DATA tails remain excluded in both implementations. The earlier “no usable loadable segments” behavior also remains aligned: the legacy path sets dummy section offsets and writes a zeroed header; the Rust-only `Libogc` path retains its empty-image rejection (Rust 194–200, 261–269; C 324–333).

The BSS behavior is isolated by profile. `pack_legacy` calls `set_legacy_bss`; `pack_libogc` continues to collect and merge its own ranges with `set_bss` (Rust 267, 272–343, 383–396). Thus the C parity change does not alter the Rust-only profile.

Rust performs checked physical-address additions during parsing and checked candidate arithmetic when forming BSS endpoints (Rust 171–185, 354–361). C uses 32-bit arithmetic without equivalent overflow rejection. This remains a safer malformed-input rejection difference, rather than a mismatch for representable address ranges. For ordinary representable ranges, the BSS address and size semantics match.

### Regression coverage and validation

The focused `legacy_bss_matches_c_add_bss_order_and_adjacency_rules` test exercises adjacent extension, separated ranges, overlap, and out-of-order input (Rust 483–516). The initialized DATA-tail test and empty-load-image test cover the prior fixes. `cargo test -p gc-elf` ran on the host and passed all 6 unit tests; doc tests also ran with 0 tests. No hardware verification was performed.

I found no remaining mismatch in the reviewed legacy conversion scope. Ledger status is managed by the coordinating reviewer.

## Channel entry preparation — 2026-10-22

`channel.rs::prepare_channel_dol` adds the native real-mode vector from HBC Reborn `wiipax/stub/realmode.S` at 0x80003400. It patches the two destination instructions with the validated DOL entry. The DOL entry field remains unchanged. This path validates section ranges, entry bounds, overlaps, the free text slot, and the BSS range. Existing ELF conversion profiles are unchanged.

The assembly source and 320-byte binary are in `src/channel-entry.S` and `src/channel-entry.bin`. Rebuild with `powerpc-eabi-gcc -c -x assembler-with-cpp`, then `powerpc-eabi-objcopy -O binary -j .realmode`. Tests check independent instruction bytes, offsets, rejection paths, and payload preservation. Physical IOS entry behavior remains unverified. A Dolphin WAD boot can use the DOL entry directly; it does not prove the physical real-mode entry path.
