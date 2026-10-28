# CONF checksum write independent audit

- Reviewer: `/root/reviewer_5`
- Original reviewer: `/root/luna-conf` (existing finding in `docs/review/libconf.md`)
- Implementer: none; independent static verification
- C sources: `../../C-sources/libogc/libogc/conf.c`, `../../C-sources/libogc/gc/ogc/conf.h`
- Rust sources: `crates/core/ogc-conf/src/lib.rs`, `crates/core/ogc-conf-sys/src/lib.rs`
- Validation: static source inspection only. No builds, tests, or hardware verification were performed.

## Confirmed write difference and effect on the file signature

The existing review note correctly identifies that Rust changes the first four bytes before saving while C writes the config buffer without such a change. Direct comparison confirms this and shows a more significant consequence: those four bytes are the `SCv0` signature, not a checksum field according to the compared C implementation.

C `CONF_Init` reads the full `SYSCONF` file and rejects it unless bytes 0 through 3 equal `"SCv0"` (`conf.c:73-78,87-88`). Rust performs the same signature check after reading the file (`ogc-conf/src/lib.rs:343-363`). The entry count begins at offset 4 in both parsers (`conf.c:128-141`; `ogc-conf/src/lib.rs:117-124`), further confirming the first four bytes' role as the format marker.

When changes are dirty, Rust `save_changes` calls `set_checksum` before opening the file (`ogc-conf/src/lib.rs:371-385`). `set_checksum` sums big-endian 32-bit words from `buffer[4..]`, then copies the result into `buffer[..4]` (`:92-103`). A successful save therefore replaces `SCv0` with the computed sum in the persisted file. On a later `init`, the Rust signature check returns `CONF_EBADFILE` unless the computed sum happens to equal the literal `SCv0` value. C `CONF_Set` updates only the located entry bytes and sets the dirty flag (`conf.c:215-246`); C `__CONF_WriteBuffer` writes the entire buffer unchanged (`:248-268`). Thus C preserves `SCv0` and Rust overwrites it.

The compared C `conf.c` contains no checksum calculation or validation. Its `CONF_Init` checks the `SCv0` marker, and its save path writes the existing full buffer. Rust also does not validate a checksum on load; it only checks the same marker. The Rust checksum write is therefore not a matching implementation of a C checksum update in this source version.

## Save error ordering

Rust mutates the in-memory signature before attempting `open_file` (`ogc-conf/src/lib.rs:381-385`). If open fails, the function returns with the in-memory buffer changed and still dirty. C opens the file before writing and does not alter the signature (`conf.c:255-268`). The persisted-file mismatch remains the primary finding; this additional ordering difference applies on failed saves.

For successful and short writes, both paths write the full `0x4000`-byte buffer, close the descriptor, report `CONF_EBADFILE` for an incomplete write, and clear the dirty flag only after a complete write (`conf.c:248-268`; Rust `write_exact` at `ogc-conf/src/lib.rs:79-85` and `save_changes` at `:387-395`).

## Conclusion

The checksum write mismatch is independently confirmed for both `ogc-conf` and `ogc-conf-sys`'s wrapper surface. The key effect is that Rust replaces the C-validated `SCv0` signature during save. Both crates remain pending with a finding; the original `docs/review/libconf.md` note is unchanged. `ogc-conf-sys` itself contains constants and packed records rather than save logic (`crates/core/ogc-conf-sys/src/lib.rs:1-150`), so the defect is implemented in `ogc-conf` and exposed by the grouped config API; the sys crate remains linked for the grouped review scope.

## 2026-10-02 source-of-truth recheck

The uploaded Rust baseline already contains the correction described by this audit. `save_changes` opens the SYSCONF file and writes `conf_buffer_for_write(&state.buffer)` without replacing bytes 0..4 with a checksum, so the `SCv0` signature is preserved. The historical mismatch above is therefore stale for the current source-of-truth tree.
