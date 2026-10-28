# CONF static parity review

## Scope and result

Compared both Rust crates with the matching libogc C header and implementation:

- Rust wrapper: `crates/core/ogc-conf/src/lib.rs`
- Rust constants and ABI records: `crates/core/ogc-conf-sys/src/lib.rs`
- C declarations: `C-sources/libogc/gc/ogc/conf.h`
- C implementation: `C-sources/libogc/libogc/conf.c`

This is a static source review. It found one confirmed data-output difference, one parser difference for malformed text data, and two apparent defects shared by C and Rust. No implementation or tests were changed as part of the review.

## Coverage map

| C surface | Rust counterpart | Review |
| --- | --- | --- |
| Error constants and success value (`conf.h:37-44`) | `ogc-conf-sys/src/lib.rs:6-13`, re-exported by wrapper (`ogc-conf/src/lib.rs:12-25`) | Values match. |
| Value-type enums (`conf.h:50-57`) | `ogc-conf-sys/src/lib.rs:15-20` | Values match. |
| Video, region, area, shutdown, LED, sound, language, aspect, and sensor-bar enums (`conf.h:59-127`) | `ogc-conf-sys/src/lib.rs:22-72` | Values match. |
| Pad limits (`conf.h:129-132`) | `ogc-conf-sys/src/lib.rs:74-77` | Formulas and values match. |
| Packed pad records (`conf.h:134-165`) | `ogc-conf-sys/src/lib.rs:79-150` | Fields and byte layouts match. Rust tests assert sizes 70, 1121, 86, and 517 bytes and alignment 1 (`ogc-conf/src/lib.rs:765-774`). |
| Init and save (`conf.h:167-168`; `conf.c:63-93,248-281`) | `init`, `save_changes` (`ogc-conf/src/lib.rs:332-396`) | Control flow and status handling traced; see checksum delta below. |
| Length, type, get, and set (`conf.h:169-171`; `conf.c:128-246`) | `get_length`, `get_type`, `get`, `set` (`ogc-conf/src/lib.rs:117-132,185-291,398-427`) | Entry lookup, type length, capacity checks, scalar zero-fill, byte copies, and dirty-state update traced. C declares Get but not Set in the header; C defines Set in its implementation. |
| Scalar setting getters (`conf.h:172-184`; `conf.c:283-410`) | `get_shutdown_mode` through `get_display_offset_h` (`ogc-conf/src/lib.rs:429-585`) | Each setting key, expected result length, and returned value was compared. |
| Pad device getters/setters (`conf.h:185-188`; `conf.c:412-451`) | `get_pad_devices`, `set_pad_devices`, `get_pad_guest_devices`, `set_pad_guest_devices` (`ogc-conf/src/lib.rs:587-637`) | Record byte lengths, count validation, and nullable guest-set behavior compared. Rust uses references and `Option` in place of C pointers. |
| Nickname, aspect ratio, EULA, parental data, WiiConnect24 (`conf.h:189-194`; `conf.c:453-530`) | Matching Rust getters (`ogc-conf/src/lib.rs:639-722`) | Keys, offsets, conversions, return checks, and output sizes compared. |
| Region, area, video (`conf.h:195-197`; `conf.c:532-581`) | `get_region`, `get_area`, `get_video` and parsers (`ogc-conf/src/lib.rs:293-329,724-749`) | Accepted values match; text parser termination difference is noted below. |
| C-private helpers: text lookup, entry lookup, buffer write | Rust-private `get_txt`, `find_entry`, `write_exact` plus state helpers (`ogc-conf/src/lib.rs:62-133,135-183,371-396`) | Main paths traced. C helper names are not declared in the public header. |

The Rust `ogc-conf-sys` crate contains constants and packed records only. It does not declare C extern functions. The wrapper re-exports `sys` and `ipc` (`ogc-conf/src/lib.rs:10-11`) and provides Rust APIs; C ABI/link-symbol compatibility was not a goal of this comparison.

## Confirmed C/Rust difference

### Save preserves the `SCv0` signature

The prior Rust save path calculated a checksum and replaced bytes 0 through 3 before writing. C preserves those bytes: `CONF_Init` requires the `SCv0` signature (`conf.c:73-88`) and `__CONF_WriteBuffer` writes the full buffer unchanged (`conf.c:248-268`). Rust now writes the buffer unchanged through `conf_buffer_for_write`, so a successful save preserves the signature as C does (`ogc-conf/src/lib.rs:92-98,373-397`). The PowerPC unit test `save_buffer_preserves_sc_v0_signature` checks the write payload (`ogc-conf/src/lib.rs`).

**BUG FIX.** `/root/worker_a` removed the checksum mutation and added the regression test. `/root/reviewer_5` owns the independent re-review of this fix. The malformed setting-text difference and shared C/Rust behaviors below remain documented.

## Shared C/Rust behavior to flag

### Parental getters reject the normal record length

Both implementations require the `IPL.PC` get result to equal 1 (`conf.c:491-519`; Rust `ogc-conf/src/lib.rs:680-709`), although each asks `CONF_Get`/`get` for a `0x4A`-byte record. For an ordinary full-size record, the result is 0x4A, so both helpers return `CONF_EBADVALUE` before copying the password or answer. This is inherited behavior, not a Rust/C parity difference.

## Difference for malformed setting text

C parses `setting.txt` as NUL-terminated strings with `strchr` (`conf.c:95-125`). Rust scans all 0x100 bytes as a slice and does not stop at an embedded NUL (`ogc-conf/src/lib.rs:135-183`). If malformed or unusual input contains a NUL before a later `=` or line ending, Rust can find and parse bytes that C string functions stop before. Expected device files may not exercise this case; behavior on such input is not verified.

## Corrupt SYSCONF input

C reads the entry count and offsets through typed pointers and then indexes the buffer (`conf.c:128-141`); its behavior for corrupt counts, offsets, and truncated entries can include out-of-bounds access or unaligned access. Rust bounds-checks portions of lookup (`ogc-conf/src/lib.rs:117-132`), but later entry slices in length/get/set paths assume fields are present (`ogc-conf/src/lib.rs:185-207,210-248,250-291`). Exact error or panic parity for corrupt SYSCONF data was not established. This is an uncertain malformed-input area, not a confirmed normal-input delta.

## Rust API additions and surface changes

- Rust uses lower-case Rust names and `&str`, byte slices, fixed-size arrays, and references for inputs and outputs. C uses `CONF_` names and raw pointers (`conf.h:167-197`; Rust `ogc-conf/src/lib.rs:332-749`). This is an API-shape difference, not a claim of source-level compatibility.
- Rust `set` corresponds to the C implementation's `CONF_Set`, which is implemented at `conf.c:215-246` but is absent from the public header declarations (`conf.h:167-197`).
- Rust provides `set_pad_guest_devices(Option<&ConfPadGuests>)`, mapping `None` to an empty record (`ogc-conf/src/lib.rs:625-637`); C accepts a nullable pointer and maps null to an empty record (`conf.c:442-450`).
- `ogc-conf-sys` exposes constants and records as Rust public items; it has no C function bindings (`ogc-conf-sys/src/lib.rs:6-150`).

## Completeness boundary

The declaration inventory covers every constant group, record, and function declared in `conf.h:37-197`, plus the C-defined `CONF_Set` and private helpers in `conf.c`. The Rust public functions in `ogc-conf/src/lib.rs:332-749` were mapped to their C counterparts. The review did not run tests or hardware checks, inspect `ogc-ipc` backend semantics for open/read/write/close, audit alternate libogc versions or forks, perform an ABI/link-symbol audit, or exhaustively assess malformed input. The stated findings apply to the compared source files and paths only.

## Fix handoff: embedded-NUL setting text parity

Implementer: current parity pass. Independent re-review: pending.

`get_txt` now applies the same string boundary as C's `strchr` calls when it searches for `=` and CR/LF terminators. The search prefix ends at the first NUL byte, while the later raw newline scan can still advance past an embedded NUL just as C's explicit `while (*line++ != '\n')` loop does. This removes the documented malformed `setting.txt` case where Rust could parse bytes that C string functions could not see.

The existing `SCv0` save fix remains unchanged. This is a static source-parity change; no hardware validation is claimed.

## 2026-10-03 bounded save-path re-review: PASSED

Reviewer: reviewer_d. Implementer: `/root` (commit `4509018`, “Preserve CONF signature when saving”). Scope is limited to the save-buffer change; the embedded-NUL parser finding above remains open and is not re-reviewed here.

C `CONF_Init` reads exactly `CONF_FILE_SIZE` bytes and validates that bytes 0 through 3 equal `SCv0` (`C-sources/libogc/libogc/conf.c:63-92`). C `__CONF_WriteBuffer` passes the original `__conf_buffer` pointer and full `CONF_FILE_SIZE` directly to `IOS_Write`, then clears dirty state only after a full-length write (`conf.c:248-269`). There is no checksum rewrite in this path.

Rust likewise stores the full 0x4000-byte file in `ConfState.buffer` (`crates/core/ogc-conf/src/lib.rs:28,36`) and checks the `SCv0` signature during init (`:331-367`). The fix removes the old checksum calculation/write to bytes 0..3; `conf_buffer_for_write` returns its entire fixed-size input reference unchanged (`:92-96`), and `save_changes` passes that exact reference to `write_exact` (`:370-393`). `write_exact` passes its slice directly to `FileDescriptor::write` and requires the full slice length (`:79-85`). The IPC layer sets the IOS write data pointer from that same slice and sets `req.data.write.len = buf.len()` (`crates/core/ogc-ipc/src/lib.rs:395-423`). Since the input is `&[u8; 0x4000]` and no transformation occurs, this preserves every byte: the four-byte `SCv0` signature, the following bytes, and all checksum/entry/data bytes elsewhere in the SYSCONF file. `ogc-conf-sys` provides constants and packed records only; it adds no alternate write path (`crates/core/ogc-conf-sys/src/lib.rs:1-150`).

The new `save_buffer_preserves_sc_v0_signature` test checks that the write-buffer helper preserves both the signature and a following sentinel word (`ogc-conf/src/lib.rs:754-763`). The implementation's identity return establishes preservation of the remaining bytes as well. `cargo check -p ogc-conf --tests --target powerpc-unknown-linux-gnu` passed, so the test target compiled. Tests did not run. No hardware verification was performed. Result: `PASSED` for the bounded save-buffer fix; the crate remains open for the previously recorded embedded-NUL parser mismatch.

## Independent re-review — embedded-NUL parser fix and file errors

**Result: NEEDS_CHANGES.** Reviewer: `/root/reviewer_d`. Implementer: commit author `notOrrytrout`. Fix: `cfc925d`.

The embedded-NUL fix matches C's string boundary: Rust bounds searches for `=` and line terminators at the first NUL, while its raw newline skip still advances across bytes as C's `while (*line++ != '\n')` does (`ogc-conf/src/lib.rs:135–153`; `C-sources/libogc/libogc/conf.c:104–125`). The fix remains incomplete for mixed CR/LF lines. Rust chooses the first CR or LF (`lib.rs:154–157`); C searches the full line for CR first and searches for LF only if no CR exists (`conf.c:108–110`). For `GAME=JP\nOTHER=x\r`, Rust returns `JP`, while C selects the later CR and can return a larger value or `CONF_ETOOBIG`.

Two file-error results also differ. C `CONF_Init` maps every non-exact `IOS_Read` result, including negative errors, to `CONF_EBADFILE` after close (`conf.c:76–85`). Rust `read_exact` maps short positive reads to `CONF_EBADFILE` but propagates negative `FileDescriptor::read` errors (`ogc-conf/src/lib.rs:71–77, 345–357`; IPC returns negative read errors at `ogc-ipc/src/lib.rs:372–384`). C `__CONF_WriteBuffer` likewise maps any write count other than `CONF_FILE_SIZE`, including negative returns, to `CONF_EBADFILE` (`conf.c:262–265`); Rust propagates negative write errors but maps short writes to `CONF_EBADFILE` (`ogc-conf/src/lib.rs:79–85, 385–393`; IPC at `ogc-ipc/src/lib.rs:406–425`). Open-error propagation and ignored close results match in the inspected paths.

The original embedded-NUL finding and checksum finding are stale: the parser fix and accepted `4509018` save fix address them. `ogc-conf-sys` contains constants/layout only. The corrupt-area behavior remains uncertain because C unchecked pointer access can be undefined behavior. `cargo check -p ogc-conf --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests were not run. `git diff cfc925d^ cfc925d --check` passed. No hardware verification was performed.
# Independent re-review of embedded-NUL parser fix (2026-10-03)

**Result: NEEDS CHANGES for broader parser parity.** Reviewer: `/root/reviewer_e`; implementer: worker identity not present in this handoff; commit `cfc925d`.

The embedded-NUL case is fixed: C string searches stop at the first NUL while its LF skip scans raw bytes (`C-sources/libogc/libogc/conf.c:95-125`); Rust limits `=` and terminator searches to the pre-NUL prefix but retains raw LF skipping (`crates/core/ogc-conf/src/lib.rs:135-179`). A separate precedence mismatch remains: C searches for CR first and only searches for LF if no CR exists (`conf.c:108-110`), while Rust picks the first CR or LF after `=` (`lib.rs:152-170`). For `GAME=JP\nOTHER=x\r`, C includes intervening bytes up to CR; Rust returns `JP`.

C maps any read length other than the requested size, including negative IOS results, to `CONF_EBADFILE`; Rust propagates negative read errors. C similarly maps any non-full write result to `CONF_EBADFILE`, while Rust propagates negative write errors (`conf.c:76-85,258-268`; Rust `lib.rs:71-85,338-359,380-393`). The separate SCv0 save fix passes: Rust preserves the input buffer before writing, as C does (`lib.rs:92-96,380-393`; `conf.c:248-268`). `ogc-conf-sys` is constants/layout only. Validation: PowerPC `cargo check -p ogc-conf --tests` passed; tests compiled but did not run. `git diff cfc925d^ cfc925d --check` passed. No hardware verification was performed.

## BUG FIX handoff: mixed CR/LF terminator precedence

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. Rust now searches for CR across the setting value first and searches for LF only when no CR exists, matching C's `strchr` order. The negative IOS read/write error mapping remains a separate finding and is not changed here.

Added a regression test for `GAME=JP\nOTHER=x\r`. It checks that parsing uses the later CR and includes the intervening LF and text, instead of stopping at the earlier LF. `cargo check -p ogc-conf --target powerpc-unknown-linux-gnu --lib` and `cargo check -p ogc-conf --tests --target powerpc-unknown-linux-gnu` passed. The test target compiled; tests were not run. No hardware verification was performed.

### Independent re-review of mixed CR/LF precedence (`8bfd642`)

**Result: PASS for the parser fix.** Reviewer: `/root/reviewer_d`; implementer: `/root/worker_a`.

C finds `=` with `strchr`, then searches the full current line for CR first and searches for LF only if no CR exists (`C-sources/libogc/libogc/conf.c:95-125`). Rust now uses the same CR-first precedence within the prefix before the first embedded NUL (`crates/core/ogc-conf/src/lib.rs:135-179`), preserving the earlier embedded-NUL fix. The `GAME=JP\nOTHER=x\r` regression checks that Rust returns through the later CR (`lib.rs:782-790`). Adjacent skip logic still matches C's raw LF scan. No parser regression was found. The malformed CR-before-`=` case is not defined behavior because C computes a negative length for `memcpy`.

Negative IOS read/write error mapping remains an independent mismatch and was not changed. Validation: PowerPC `cargo check -p ogc-conf --tests` passed; tests compiled but did not run. No hardware verification was performed.

## BUG FIX handoff: map negative IOS read/write results to `CONF_EBADFILE`

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. The exact-read and exact-write helpers now map every non-exact result, including negative IPC errors, to `CONF_EBADFILE`, matching C `CONF_Init` and `__CONF_WriteBuffer`. Open errors still propagate unchanged. `init` still closes each opened descriptor after its read attempt, and `save_changes` still closes its descriptor after the write attempt and clears dirty state only after a full write. The separate CR-first parser fix in `8bfd642` is not changed or re-reviewed here.

Added injected negative-read and negative-write regressions. `cargo check -p ogc-conf --target powerpc-unknown-linux-gnu --lib` and `cargo check -p ogc-conf --tests --target powerpc-unknown-linux-gnu` passed; the test target compiled, but tests were not run. `git diff --check` passed. No hardware verification was performed.

### Independent re-review of negative read/write mapping (`36c314c`)

**Result: PASS for this fix.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_a`.

C `CONF_Init` maps every read result other than the exact requested size, including a negative IOS result, to `CONF_EBADFILE` (`../../C-sources/libogc/libogc/conf.c:76–85`). C `__CONF_WriteBuffer` does the same for write results (`:258–268`). Rust's shared exact-read and exact-write helpers now map all non-exact results to `CONF_EBADFILE`; open errors remain outside these helpers and continue to propagate (`crates/core/ogc-conf/src/lib.rs:71–100`). The regression tests inject negative read and write results and check the returned error (`:801–816`). The reviewed fix matches C for these paths. The separate parser and checksum findings remain as recorded.

No checks or tests were run during this independent pass. The handoff records that PowerPC library and test-target checks passed, but tests did not run. No hardware verification was performed.

## 2026-10-05 completion review

`ogc-conf` and `ogc-conf-sys` are now `reviewed`. The SCv0 save signature, embedded-NUL parser behavior, C's CR-before-LF terminator precedence, and negative IOS read/write mapping have all been reconciled against current source. The sys crate is constants/layout only and has no separate confirmed mismatch. Bounds handling for malformed input is treated as safety behavior.
