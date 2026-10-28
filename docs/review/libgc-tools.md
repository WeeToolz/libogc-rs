# GameCube tool crates: `gc-binary` and `gc-elf`

Reviewer: `/root/reviewer_6`  
Implementer: extraction in commit `530d086` (author `notOrrytrout`)  
Review type: first-pass static source comparison.  
Validation: no build or tests run, per review-thread instruction. No hardware verification was performed.

## Scope

`gc-binary` provides generic checked byte-range, big-endian integer, and alignment helpers. There is no corresponding `gc-binary` crate or equivalent shared helper API in the C `libogc` tree. These functions are Rust-only tooling support; no C parity claim can be made for the helper API itself. Their checked-range arithmetic rejects overflow and out-of-input ranges, and the integer helpers use big-endian conversion. No confirmed defect was found in this Rust-only scope.

`gc-elf` converts ELF32 PowerPC files to DOL. The authoritative C counterpart is `../../C-sources/gamecube-tools/elftool/elf2dol.c`, which is outside `C-sources/libogc` but is present in the supplied source tree. `DolProfile::LegacyElf2Dol` claims parity with this converter. `DolProfile::Libogc` retains the Rust xtask converter behavior; no C `libogc` converter counterpart for that profile was found, so this profile is recorded as a Rust-only addition and not certified for C parity here.

## Confirmed mismatch: LegacyElf2Dol adds BSS for initialized data segments

In C `read_elf_segments`, executable segments add a BSS tail when `memsz > filesz` (`../../C-sources/gamecube-tools/elftool/elf2dol.c:231-243`). For non-executable segments, C records a whole BSS region only when `filesz == 0` (`:251-272`); when a non-executable data segment has both initialized bytes and a larger memory size, C emits the data segment but does not add its trailing memory tail to the DOL BSS fields. `map_dol` and `write_dol` then serialize these recorded sections and BSS values (`:301-334`, `:370-415`).

Rust `parse_elf` retains both file and memory sizes (`crates/tools/gc-elf/src/lib.rs:151-192`). In `pack_legacy`, the pre-classification block adds `(address + file_size, address + memory_size)` to the BSS list for every segment where `memory_size > file_size` (`:214-235`), including non-executable data segments with nonzero file size. `set_bss` serializes this range (`:280`, `:358-379`). Therefore a non-executable PT_LOAD segment with `filesz > 0` and `memsz > filesz` produces a nonzero BSS range in Rust but not in C. The existing `legacy_profile_uses_physical_addresses_and_64_byte_alignment` fixture itself contains such a data segment and expects Rust BSS `0x208/0x18` (`:405-430`), while the C implementation would leave BSS empty for that segment. The test expectation does not demonstrate C parity.

Keep `gc-elf` open as `finding`. The implementation thread should decide whether this is an intended correction to the C converter's omitted data tail or whether the `LegacyElf2Dol` profile must match C exactly. No implementation change was made in this review.

## Other observed differences and limits

- Rust's legacy `set_bss(..., true)` sorts ranges and rejects disjoint ranges (`crates/tools/gc-elf/src/lib.rs:358-379`), while C `add_bss` starts with the first BSS encountered and only extends it when the next start equals the current end (`../../C-sources/gamecube-tools/elftool/elf2dol.c:149-162`). This changes output/error behavior for disjoint, overlapping, or out-of-order BSS ranges. Rust appears to make the unrepresentable disjoint case explicit; treat this as a behavior difference for implementation disposition, not as evidence that Rust should reproduce silent C truncation.
- Rust validates file ranges and address arithmetic before packing (`crates/tools/gc-elf/src/lib.rs:166-185`), while the C converter relies on file seek/read behavior and unchecked 32-bit address arithmetic. These are safer Rust checks and were not recorded as bugs.
- The main output organization is consistent for ordinary segments: C puts executable segments in text slots, other file-backed segments in data slots, aligns file offsets to 64 bytes, and uses physical addresses (`../../C-sources/gamecube-tools/elftool/elf2dol.c:220-281`, `:301-334`); Rust's legacy profile uses physical addresses and 64-byte alignment and places text before data (`crates/tools/gc-elf/src/lib.rs:178-181`, `:214-273`).

## Review state

- `gc-binary`: reviewed for the Rust-only helper scope; no C counterpart exists for direct semantic parity review.
- `gc-elf`: finding remains open for the `LegacyElf2Dol` BSS mismatch above. `Libogc` profile parity against a C converter was not applicable in the supplied source scope.
- Reviewer: `/root/reviewer_6`; implementer: extraction commit `530d086`.
