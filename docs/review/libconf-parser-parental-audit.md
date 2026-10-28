# CONF parser and parental getter independent audit

- Reviewer: `/root/reviewer_5`
- Existing review: `/root/luna-conf`, `docs/review/libconf.md`
- Scope: direct static recheck of the `setting.txt` lookup path and parental password/answer getters
- C source: `../../C-sources/libogc/libogc/conf.c`
- Rust source: `crates/core/ogc-conf/src/lib.rs`
- Validation: source inspection only. No builds, tests, or hardware verification were performed.

## `setting.txt` parser: malformed embedded-NUL difference confirmed, with a narrower trigger

C `__CONF_GetTxt` uses `strchr` on the current line to find `=` and then `\r` or `\n` (`conf.c:95-120`). These searches stop at an embedded NUL. If a delimiter is not found, its line-skip loop advances byte by byte through the NUL until newline (`:122-124`). Rust `get_txt` searches the whole remaining fixed-size byte slice for `=`, then searches the value slice for CR/LF without stopping at NUL (`ogc-conf/src/lib.rs:135-183`).

The difference requires a NUL in the value after a matching key and `=`; a NUL before `=` becomes part of the Rust key slice and therefore does not match the requested name. For example, with a malformed decrypted line `GAME=\0X\n`, C finds the `=` but cannot find CR/LF as a C string before the embedded NUL; it skips the line and eventually returns `CONF_ENOENT`. Rust finds the newline in the slice, returns a two-byte value containing NUL and `X`, and `region_from_text` returns `CONF_EBADVALUE` (`lib.rs:293-301,724-731`). This confirms a malformed-input error-result difference. Ordinary NUL-terminated or CR/LF-terminated values without embedded NUL follow the same lookup and exact-value parsing intent.

## Parental password and answer getters: same length-check defect in both

C `CONF_GetParentalPassword` and `CONF_GetParentalAnswer` each call `CONF_Get("IPL.PC", buf, 0x4A)` and reject any nonnegative result other than 1 (`conf.c:491-518`). C `CONF_Get` returns the actual value length (`:182-212`), and `CONF_GetLength` returns the stored array length for array entries (`:144-168`). Rust calls `get("IPL.PC", &mut buf)` for a 0x4A-byte buffer and also rejects every result except 1 before copying password/answer bytes (`ogc-conf/src/lib.rs:680-710`); Rust `get_inner` returns the actual entry length (`:210-248`).

Therefore, when `IPL.PC` contains the full 0x4A-byte record both getters return `CONF_EBADVALUE` and leave the caller output untouched. This is the same apparent defect in C and Rust, not a Rust/C parity mismatch. The source checkout does not include a SYSCONF sample proving the exact stored `IPL.PC` length, so the full-record case is stated conditionally on that expected record size.

## Malformed short parental record limit

If a malformed array-typed `IPL.PC` record instead reports length 1, both implementations pass the `res == 1` check and then copy the password/answer fields. For C array entries, `CONF_Get` copies only the reported bytes and does not clear the rest of its destination (`conf.c:195-201`); the local `u8 buf[0x4A]` is uninitialized (`:494-500,509-515`), so bytes beyond the one copied byte are indeterminate. Rust initializes its local `[0u8; 0x4a]` to zero (`ogc-conf/src/lib.rs:681,697`), so the extra output bytes are deterministic zeros. For scalar entries C does clear the destination before copying (`conf.c:202-208`), so this difference is limited to malformed short array records. C's array output is indeterminate; no normal-input mismatch is asserted.

## Result

The malformed-text parser difference is confirmed only for embedded NUL in a matching key's value. The full-size parental-record length-check defect is present in both implementations and is not a parity mismatch. The short/corrupt-record output behavior is recorded as an uncertain malformed-input difference. The primary `ogc-conf` and `ogc-conf-sys` finding rows remain pending; the previous checksum audit finding also remains documented separately in `docs/review/libconf-checksum-audit.md`.
