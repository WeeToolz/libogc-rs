# `mad-sys` static review

This file is a chronological review record. Its first sections describe an
older implementation and findings at the time of that review; later sections
record fixes and remaining work. Do not treat the initial `nanomp3` description
or initial finding list as the current state without reading the follow-ups and
checking the current source and ledger. The current decoder is included under
`vendor/libmad-rs`. See [the evidence summary](../evidence-summary.md) for a
recent Dolphin probe failure.

Reviewer: `/root/reviewer_6`  
Implementer: `notOrrytrout`; latest relevant commit `8864287`, following decoder lifecycle/error commits `612778b` and `7fb7e0a`.  
Review type: first-pass static comparison.  
Validation: no build or tests run, as instructed. No hardware verification was performed.

## Scope and sources

Rust: `crates/media/mad-sys/src/lib.rs`. It exposes Rust functions and `repr(C)` data types and uses the `nanomp3` decoder (`:8-10`), rather than linking the C libmad implementation.  
C public headers: `../../C-sources/libogc/gc/mad.h`, `../../C-sources/libogc/libmad/stream.h`, `frame.h`, `synth.h`, and `decoder.h`.  
C implementation: `../../C-sources/libogc/libmad/stream.c`, `frame.c`, `decoder.c`, `synth.c`, `version.c`.

The Rust crate does not reproduce all libmad algorithms, constants, or symbols. Confirmed ABI and observable behavior mismatches follow. The crate remains open as `finding`.

## Confirmed findings

### `MadDecoderAsync` does not match the C ABI

C defines the nested async state as three `s32` values (`pid`, `in`, `out`) in `../../C-sources/libogc/libmad/decoder.h:41-50`. Rust declares `pid: i64`, then two `i32` values (`crates/media/mad-sys/src/lib.rs:140-146`). On the 32-bit PowerPC target this makes the Rust nested struct larger and shifts the offsets of `in`, `out`, `sync`, and all later `MadDecoder` fields relative to C. Rust `MadDecoder` is `repr(C)` and embeds this type (`:156-170`), so the mismatch affects the ABI layout of the whole decoder object.

Rust initialization also differs from C field initialization: C `mad_decoder_init` sets mode to `-1` and async `in`/`out` to `-1` (`decoder.c:75-92`); Rust sets mode to `0` and zeroes async fields (`lib.rs:499-519`).

### `stream_skip` replaces pending skip length instead of accumulating it

C `mad_stream_skip` adds each requested length to `stream->skiplen` (`../../C-sources/libogc/libmad/stream.c:93-99`). Rust assigns the new length (`crates/media/mad-sys/src/lib.rs:261-267`). Two calls before frame decode therefore skip only the last requested amount in Rust, but their sum in C.

### `stream_sync` accepts buffers that C rejects and changes stream state on failure

C scans for sync, then rejects the stream when fewer than `MAD_BUFFER_GUARD` (8) bytes remain from the sync position (`../../C-sources/libogc/libmad/stream.c:105-121`; constant in `stream.h:27-28`). Rust's `find_sync` only requires a two-byte sync word and `stream_sync` returns success without the 8-byte guard (`crates/media/mad-sys/src/lib.rs:217-220`, `:273-307`). For a buffer containing a sync word with fewer than eight remaining bytes, Rust returns success while C returns `-1`. Rust also writes `error=BUFLEN` on the failure path (`:285-299`), whereas C's sync helper returns `-1` without setting that field.

### `header_decode` consumes a full frame instead of decoding only its header

C `mad_header_decode` parses the header, computes its frame boundary, and returns with the frame marked incomplete (`../../C-sources/libogc/libmad/frame.c:299-424`). C `mad_frame_decode` then uses that state to decode audio data (`:436-455`). Rust `header_decode` calls its `frame_decode` and copies the decoded frame header (`crates/media/mad-sys/src/lib.rs:337-347`); it therefore consumes audio data and advances `next_frame` to the next frame. A caller using the C header-then-frame sequence cannot get the same stream state or decode the same frame in Rust.

### Rust does not provide C frame data or header fields to filters and callbacks

C `mad_frame_decode` fills `frame.sbsample` and `frame.header` for the selected MPEG layer (`../../C-sources/libogc/libmad/frame.h:49-74`, `frame.c:369-424`, `:436-455`). C's decoder calls `filter_func` on that decoded frame and then calls `mad_synth_frame` on the possibly modified frame (`../../C-sources/libogc/libmad/decoder.c:422-435`).

Rust instead decodes directly to float PCM in `frame_decode`, sets only layer, mode, bitrate, samplerate, options, and an opaque pointer (`crates/media/mad-sys/src/lib.rs:373-427`). It leaves `sbsample` and other header fields such as mode extension, emphasis, flags, private bits, CRC, and duration at their prior/default values. Rust's `synth_frame` then reads the saved decoder PCM through the opaque pointer and never reads `frame.sbsample` (`:469-492`). A filter that reads or modifies C frame subband data therefore receives zeros in Rust, and its frame modifications do not affect Rust's synthesized output.

### The half-sample-rate option is ignored during synthesis

C `mad_synth_frame` halves both the PCM sample rate and sample count and uses `synth_half` when `MAD_OPTION_HALFSAMPLERATE` is set (`../../C-sources/libogc/libmad/synth.c:831-855`). Rust copies stream options to the frame (`crates/media/mad-sys/src/lib.rs:411-425`) but `synth_frame` always emits the original sample rate and all decoded samples (`:469-492`). The option changes C output but has no effect on Rust output.

### `decoder_run` maps callback flows and empty input differently from C

C `run_sync` returns success for an absent input callback, treats input `MAD_FLOW_IGNORE` as a request for another input callback, and returns failure for `MAD_FLOW_BREAK` (`../../C-sources/libogc/libmad/decoder.c:314-357`, `:453-461`). The current Rust implementation matches these input cases: no callback returns 0, `IGNORE` retries input, and `BREAK` returns -1 (`../libmad-rs/crates/mad-sys/src/imp/decoder.rs:73-109`). Regression tests now cover each result and retry count. The older callback mismatch report is stale for input callbacks. Header, filter, output, and error callback findings remain separate.

C also treats `MAD_FLOW_IGNORE` from `header_func` as skip-current-frame and `MAD_FLOW_BREAK` as failure (`decoder.c:374-400`); from `filter_func`, `IGNORE` skips synthesis and `BREAK` fails (`:422-433`). Rust does not special-case `IGNORE` from `header_func`, and returns success for `BREAK` from header, filter, and output callbacks (`lib.rs:590-618`). These flow values are part of the public callback contract and have different results and processing order.

### Decoder sync-state ownership differs after `decoder_run`

C allocates `decoder->sync` for a run, then frees it and sets the pointer to null before returning (`../../C-sources/libogc/libmad/decoder.c:552-560`). Rust allocates the sync state once and leaves the pointer in `decoder.sync` after `decoder_run` completes (`crates/media/mad-sys/src/lib.rs:550-559`, `:621`). This leaves a different publicly visible decoder state and retains memory until an explicit Rust `decoder_finish` call.

### The Rust symbols have different values and shapes from C version metadata

C exports NUL-terminated arrays with version text, full copyright and author strings, and a build string containing configured flags (`../../C-sources/libogc/libmad/version.c:29-58`; version macros in `gc/mad.h:39-59`). Rust's public statics contain only `"0.15.0 (beta)"`, `"GPL"`, an author without the C email, and `"nanomp3-rust"` (`crates/media/mad-sys/src/lib.rs:182-185`). These are not value-compatible replacements for the C metadata strings.

## Coverage and limitations

The existing tests check a few constants, nonzero struct sizes, error strings, and a synthetic decoder path (`crates/media/mad-sys/src/lib.rs:643-773`). They do not assert field offsets/sizes against the C declarations, stream skip accumulation, guard-byte sync behavior, header-only decode state, callback flow results, frame data visible to filters, or half-rate output. Tests were not run.

The Rust path uses `nanomp3` and hard-codes successful frame headers to Layer III (`lib.rs:415-423`); C's decoder table dispatches Layer I, II, and III (`../../C-sources/libogc/libmad/frame.c:53-61`). Static review did not validate codec output for real bitstreams. This remains an additional parity limitation for implementation disposition.

## Review state

`mad-sys` remains `pending,finding`, owned by `/root/reviewer_6`. No implementation code was changed.

## Follow-up implementation — stream helper parity

Implementation pass: 2026-10-01.

The current source already has the previously reported decoder ABI/init corrections: `MadDecoderAsync.pid` is `i32`, `in` and `out` default to `-1`, and `decoder_init` sets `mode` to `-1`. Those items should no longer be treated as open findings.

This pass fixes the small stream-helper differences against current `libmad/stream.c` and `bit.c`:

- `stream_init` now gives both bit pointers the `mad_bit_init(..., NULL)` state (`cache = 0`, `left = 8`).
- `stream_buffer` now sets `sync = 1` and initializes `stream.ptr` to the supplied buffer, instead of resetting the public error field.
- `stream_skip` now accumulates requested skip lengths with 32-bit wrapping semantics instead of replacing the previous value.
- `stream_sync` now starts from the public bit-pointer position, requires the eight-byte `MAD_BUFFER_GUARD`, updates only the bit pointer on success, and does not modify `stream.error` or consume `skiplen` on failure/success.

The larger decoder/frame/synthesis differences documented above remain open. These changes need build/test and independent review.

## Follow-up implementation — decoder/frame/synthesis parity

Implementation pass: 2026-10-01, after the stream-helper parity pass.

This pass addresses the larger observable decoder/frame/synthesis mismatches above against the current libogc `libmad` sources:

- `header_decode` is now a real header-only operation. It parses the MPEG version/layer, bitrate, sample rate, mode/mode-extension, emphasis, protection/CRC fields, public/private flags, frame duration, padding, and free-format bitrate state; computes `this_frame`/`next_frame`; enforces `MAD_BUFFER_GUARD`; and returns with `MAD_FLAG_INCOMPLETE` set instead of consuming audio data.
- `frame_decode` now honors an already-incomplete header and decodes that same frame. It no longer hard-codes successful frames to Layer III or overwrites the header with a reduced nanomp3 summary.
- decoded samples are copied into the public `frame.sbsample` storage and `synth_frame` reads that storage. A filter callback therefore sees nonzero frame data and modifications it makes are reflected in later synthesis, instead of synthesis bypassing the public frame through hidden PCM state.
- `MAD_OPTION_HALFSAMPLERATE` now halves both the public PCM sample rate and sample count, and synthesis phase advances by the libmad `MAD_NSBSAMPLES` count modulo 16.
- `decoder_run` now follows the synchronous C callback-flow contract: absent input succeeds; input `IGNORE` requests more input; `BREAK` fails; header/filter `IGNORE` skips the current frame at the same stage as C; output `IGNORE` continues; and `BREAK` from header/filter/output/error callbacks fails.
- synchronous decoder state is now owned only for the duration of `decoder_run`; it is released and `decoder.sync` is reset to null before every return path.
- `decoder_message` now returns `-1` for this libogc build, matching the C build where `USE_ASYNC` is disabled.
- `stream_init` leaves `main_data` null and decoder-private state is allocated lazily when frame decoding needs it.
- `frame_finish` no longer clears the public header/options/sample data, and `synth_mute` now clears only the synthesis filterbank while preserving phase/PCM, matching the C lifecycle contracts.
- the remaining public libmad error constants/error strings and the version/copyright/author/build strings now match the C sources used by libogc (`FPM_PPC`).

### Remaining implementation-engine limitation

The Rust crate still uses `nanomp3` as its compressed-audio engine instead of a direct Rust port of libmad's Layer I/II/III decode and polyphase synthesis algorithms. To preserve callback observability, this pass maps decoded PCM into the `sbsample[channel][slot][subband]` storage and synthesizes from that public storage. This makes filter mutations affect output, but those values are **not** libmad's exact subband coefficients. A filter that depends on the numerical meaning of libmad subband-domain samples can therefore still produce different output.

This remaining algorithmic difference requires either a direct port of libmad's layer decoders and synthesis filterbank or a separately validated mathematical compatibility layer. It is not represented as resolved by this implementation pass.

Build/test and independent review are still required. No hardware result is claimed here.

## Independent re-review: decoder ABI and initialization fix (`2fcad9c`)

**Result: PASSED for this fix scope.** Reviewer: `reviewer_e`. Implementer: `worker_b` (`notOrrytrout` in the commit metadata). This re-review covers only the decoder struct layout and `decoder_init` values changed by `2fcad9c`; the other findings above remain open.

C declares `mode` and `options` as enum/`s32`, the three async members as `s32`, followed by a sync pointer, callback data pointer, and six callback pointers (`../../C-sources/libogc/libmad/decoder.h:29–68`). Rust's active public definitions use `i32` for mode, options, `pid`, `in_`, and `out`, then the same pointer and callback sequence (`../libmad-rs/crates/mad-sys/src/imp/abi.rs:225–265`). On the 32-bit PowerPC target, this gives `MadDecoderAsync` size 12 and offsets 0/4/8. `MadDecoder` has offsets: mode 0, options 4, async 8, sync 20, `cb_data` 24, callbacks 28/32/36/40/44/48, and total size 52. These match the C field sequence and target widths; there is no padding gap in this layout.

C `mad_decoder_init` sets mode to -1, options to 0, async `{pid,in,out}` to `{0,-1,-1}`, sync to null, then copies `data` and all six callback arguments (`../../C-sources/libogc/libmad/decoder.c:58–93`). Rust `decoder_init` writes the same values and callback fields (`../libmad-rs/crates/mad-sys/src/imp/decoder.rs:20–43`), and `MadDecoderAsync::default` is `{0,-1,-1}` (`../libmad-rs/crates/mad-sys/src/imp/abi.rs:227–241`). The correction therefore resolves both the prior field-width/layout mismatch and the initialization mismatch.

The exact diff in `2fcad9c` added a regression test with size/offset and initialized-value assertions. A later crate migration (`1a473e7`) moved the active implementation into the sibling `../libmad-rs` source tree. The current active test sources no longer contain those layout/initialization assertions; current decoder tests exercise run behavior instead (`../libmad-rs/crates/mad-sys/src/imp/decoder.rs:309–365`). Thus the original added assertions did pin the correction when introduced, but that coverage is absent from the current source tree.

Validation: `cargo check -p mad-sys --target powerpc-unknown-linux-gnu --tests` passed, which compiled the current target test sources. Tests were not executed. The prior implementation handoff reports a PowerPC target check with warnings denied; this review did not rerun that command. No hardware verification was performed. Other `mad-sys` parity findings remain open.

## Independent re-review: lifecycle and stream errors (`612778b`, `7fb7e0a`)

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. Implementer: `notOrrytrout` (commit author). This review covers the decoder/frame lifecycle and stream error reporting changes in `612778b` and `7fb7e0a`, using the active implementation in `../libmad-rs`. Codec algorithm differences remain separate from this bounded result.

### Confirmed mismatches

1. **Calling `decoder_finish` from an active synchronous callback frees live state.** In this libogc build `USE_ASYNC` is disabled, so C `mad_decoder_finish` performs no cleanup and returns 0 (`../../C-sources/libogc/libmad/decoder.c:95–124`). C `mad_decoder_run` owns `decoder->sync` until `run(decoder)` returns, then frees it and clears the pointer (`decoder.c:532–561`). Rust `decoder_finish` unconditionally frees any non-null `decoder.sync` (`../libmad-rs/crates/mad-sys/src/imp/decoder.rs:49–62`); `decoder_run` invokes callbacks while holding and later dereferencing that same pointer (`:86–108, 111–193`). A callback can call `decoder_finish` using a decoder pointer passed through callback data; Rust then frees the active state, and the run loop can use the freed pointer and later free it again. C is a no-op in that callback case. Normal return paths in Rust do release sync state at `:102–108`, matching C's ownership on ordinary completion.

2. **`frame_finish` no longer releases a non-null overlap allocation.** C `mad_frame_finish` calls `mad_header_finish` and frees `frame->overlap` when non-null, then clears that pointer (`../../C-sources/libogc/libmad/frame.c:104–112`). Active Rust `frame_finish` only sets `overlap` to null (`../libmad-rs/crates/mad-sys/src/imp/frame.rs:137–147`). Rust-generated frames currently keep this field null (`:78–86`), so ordinary Rust decode has no overlap allocation to release. However, for a valid public `MadFrame` whose overlap was allocated as the C API specifies, `mad_frame_finish` frees it while Rust's function leaks it. The `612778b` regression test that asserted clearing all frame fields was later removed during source migration; current active tests do not directly exercise frame cleanup.

3. **Rust recognizes an error code that C's stream error API treats as unknown.** C's `enum mad_error` in `stream.h:30–55` has no `MAD_ERROR_BADMODE` value, and `mad_stream_errorstr` has no case for `0x0222`, returning null for unrecognized values (`../../C-sources/libogc/libmad/stream.c:128–158`). Rust defines `MAD_ERROR_BADMODE = 0x0222` and maps it to `"bad bitrate/mode combination"` (`../libmad-rs/crates/mad-sys/src/imp/abi.rs:25–46`, `imp/error.rs:9–34`); `stream_errorstr` exposes that mapping (`imp/stream.rs:270–280`). Setting `stream.error` to `0x0222` therefore returns a string in Rust and null in C. This is distinct from the ordinary error values, whose current text matches C.

### Behavior and coverage that matched

C `mad_decoder_run` allocates sync state for the duration of a run, returns `-1` if `malloc` fails, then frees and clears the state after `run` returns (`decoder.c:532–561`). Rust's ordinary sync run wraps the call result and releases frame/stream state and the box on all returned paths (`../libmad-rs/crates/mad-sys/src/imp/decoder.rs:73–109`). The current target decoder tests verify cleanup after a normal stop and rejection of async mode (`:309–336`), but not a callback that invokes `decoder_finish`, error callbacks that return early, or allocation failure. Rust uses `Box::new` (`:86`); unlike C's checked `malloc`, Rust has no recoverable allocation-failure return path here.

For error text, Rust's strings for each code actually declared by C's `stream.h` match the cases in `stream.c:130–155`. The test added by `7fb7e0a` checked only NONE, BUFLEN, BUFPTR, unknown, and null; after the source migration the active tests under `../libmad-rs/crates/mad-sys/src/imp` no longer test the public `stream_errorstr` wrapper. They test `mad_error_string` for BADMODE instead (`imp/error.rs:48–55`), which preserves the mismatch above.

### Validation and remaining scope

`cargo check -p mad-sys --target powerpc-unknown-linux-gnu --tests` passed; this compiled target test code but did not execute tests. No hardware verification was performed. This bounded review does not close the earlier decoder callback-flow, frame/subband, or codec parity findings. The ledger is unchanged; only this review note was edited.

### Implementer handoff: BADMODE error string result

Implementer: `worker_b`. Status: fixed; needs independent review. Source commit in `../libmad-rs`: `f6476a2` (`Match C BADMODE error string result`). `stream_errorstr` now returns null for `MAD_ERROR_BADMODE`, matching C's default case for unknown error codes. The internal `mad_error_string` mapping remains available to Rust code. Regression coverage checks null for BADMODE and preserves the text for a C-declared error.

Validation: `cargo test -p mad-sys` passed (16 tests); `cargo test -p mad-sys --no-run` also passed. Tests ran on the host codec workspace, not on hardware. The previous powerpc target test compile command was not applicable because the active MAD source is in the separate `libmad-rs` workspace; no hardware verification was performed. Independent static re-review of the BADMODE result passed on 2026-10-03. Other MAD findings remain open.

## Independent re-review: BADMODE error string (`f6476a2`)

**Result: PASSED for this fix scope.** Reviewer: `reviewer a`. Implementer: `worker_b` (`notOrrytrout` in the source commit). Implementation commit: `f6476a220311a607ea5757789797f483ebb7e8e7` in `../libmad-rs`.

C `mad_stream_errorstr` uses an explicit switch over the error values declared in `libmad/stream.h`; `MAD_ERROR_BADMODE` is not declared there, and the function returns null after the switch for every unrecognized value (`../../C-sources/libogc/libmad/stream.c:128–158`). The Rust stream wrapper reads the public stream error and now returns a null pointer for `MAD_ERROR_BADMODE` before consulting its broader internal error-string table (`../libmad-rs/crates/mad-sys/src/imp/stream.rs:274–284`). That internal table still has the BADMODE string for Rust decoder internals (`../libmad-rs/crates/mad-sys/src/imp/error.rs:9–34`), but it no longer leaks that extra internal mapping through the C-compatible stream API. For `MAD_ERROR_NONE`, the wrapper continues to expose `"no error"`, matching the corresponding C case. The change is scoped to the reported mismatch and does not alter other stream error mappings.

The exact fix commit changes only `crates/mad-sys/src/imp/stream.rs` in the separate `libmad-rs` repository. Its regression tests assert that BADMODE returns null and that the C-declared NONE error retains its text. Validation performed independently: `cargo test -p mad-sys` in `../libmad-rs` passed; all 16 unit tests and 0 doc tests ran successfully. Tests ran on the host. No hardware verification was performed. The decoder callback, frame cleanup, codec algorithm, and other findings above remain open; this pass does not mark the crate complete.

## Input callback flow handoff

Implementer: `worker_b`. Status: fixed; needs independent review. Current `libmad-rs` HEAD already matches C for the scoped input cases; this pass adds regression coverage without changing runtime behavior. The tests verify that an absent input callback returns success, input `MAD_FLOW_IGNORE` invokes the callback again, and `MAD_FLOW_BREAK` returns failure. The stale input-flow mismatch text above now records the active implementation and current paths.

Validation: `cargo test -p mad-sys` in `../libmad-rs` passed all 19 unit tests; doc tests also passed (0 tests). `git diff --check` passed for the source and review note. No ledger changes or hardware verification.

## Follow-up implementation — accumulated skip regression

Implementer: `worker_b`. Test commit: `14427ec` in the active `libmad-rs` repository.

The skip mismatch was already corrected in the active source: the `mad_stream_skip` ABI wrapper adds with `u32::wrapping_add`, and the stream consumer applies the pending sum before decoding the next frame. This matches C `u32` addition, including wraparound. The consumer also preserves C's partial-buffer behavior by subtracting bytes consumed from `skiplen` when the pending skip is larger than the available input.

This pass adds a regression test that calls the public wrapper twice (12 bytes, then 7 bytes), decodes a valid silent Layer I frame placed at offset 19, and checks that frame decoding starts at that offset and clears the pending skip. The core stream tests already cover `u32` wraparound.

Validation: `cargo test -p mad-sys` passed (20 tests); `cargo fmt --check -p mad-sys` passed. No hardware verification was performed. This fix and its coverage need independent static re-review; the review thread owns that step. Other `mad-sys` findings remain open.

## Independent re-review: accumulated stream-skip regression (`14427ec`)

**Result: PASS for the regression coverage; existing implementation matches C for this scope.** Reviewer: `worker_a`. Implementer: `worker_b`.

C `mad_stream_skip` adds each requested `u32` length to `stream->skiplen` (`../../C-sources/libogc/libmad/stream.c:93–99`). The active Rust wrapper uses wrapping addition, and frame decode consumes that accumulated length while retaining any remainder when the input buffer is shorter (`../libmad-rs/crates/mad-sys/src/imp/stream.rs` and `frame.rs`). Commit `14427ec` adds a public-wrapper regression that requests 12 then 7 bytes, places a valid Layer I frame at byte 19, and checks that decode starts there and clears the pending skip. The test uses an independent fixed offset and valid frame header.

I inspected the test commit and compared the active implementation with C. No checks or tests were run during this re-review; the implementer's note records 20 passing `mad-sys` tests and a passing format check in the separate `libmad-rs` workspace. Other MAD decoder, frame cleanup, and codec differences remain open. No hardware verification was performed.

## Implementer handoff — decoder driver, public state, target arithmetic, and ABI completion (2026-10-07)

Status: fixed; **needs independent re-review**. The active implementation is the sibling `libmad-rs` repository supplied with this pass. This handoff supersedes the old `nanomp3`-era blocker descriptions for current-source disposition; historical findings remain above for traceability.

The decoder callback and lifecycle path is now centralized in one `DecoderDriver`. It follows bundled `decoder.c` sync ordering for input, header, frame decode, filter, synthesis, output, and recoverable errors. `MAD_FLOW_STOP`, `BREAK`, `IGNORE`, and `CONTINUE` are handled at the same stage boundaries as C. The error callback pointer and its callback-data pointer are snapshotted once at run entry, while other callback pointers/data are read at their point of use. `mad_decoder_finish` is a no-op for this no-`USE_ASYNC` build, so callback-time finish cannot free the live sync state. Sync allocation is checked and is freed/cleared by `decoder_run` on return.

Layer III public state no longer hides Rust ownership behind C fields. `stream.main_data` points to actual `MAD_BUFFER_MDLEN` reservoir storage, `frame.overlap` points to actual persistent overlap storage, and frame decode imports callback-visible changes before Layer III decode and exports the updated reservoir/overlap afterward. `mad_frame_mute` clears overlap as C does, and `mad_frame_finish` releases it. Layer I/II do not allocate Layer III main-data state.

The codec arithmetic is now target-profiled without duplicating the decoder. Host builds retain the already-recorded exact `FPM_DEFAULT + OPT_SSO` Layer III differential behavior. PowerPC builds select the bundled C configuration: `FPM_PPC`, no `OPT_ACCURACY`, no `OPT_SPEED`, and no `OPT_SSO`. Requantization, IMDCT, alias/stereo arithmetic, the factorized synthesis DCT, and polyphase accumulation use that configured path. PowerPC synthesis uses the full Q28 `D` table and scaled per-product accumulation instead of the host SSO preshift/low-word path.

Metadata now matches bundled `version.c`/headers (`0.15.0 (beta)`, complete author/copyright text, and `FPM_PPC ` build string). `mad_stream_errorstr` again returns null for internal `MAD_ERROR_BADMODE`, matching C's switch default. Missing public fixed (`mad_f_abs`/`mad_f_div`), bit, and timer helpers were added in the sibling `mad-sys` implementation and wired into this workspace wrapper. The C source declares `mad_bit_write` but compiles its implementation out under `#if 0`; the Rust wrapper does not invent a callable implementation for that absent C symbol.

The sibling repository records the full staged parity table and M1–M6 disposition in `docs/mad-sys-parity-2026-10-07.md`. Its existing host differential report covers 419 accepted Layer III frames with 512,640 active subband values and 512,640 PCM values matching its recorded C `FPM_DEFAULT` executable exactly. That recorded host result is retained because the non-PowerPC arithmetic path is unchanged by this target-profile pass.

Validation in this environment is limited: no `cargo`, `rustc`, or `rustfmt` executable is installed, so the new source could not be compiled or executed. `git diff --check` and structural checks are required before packaging. The ledger therefore moves `mad-sys` to `needs_re_review`, not `reviewed`; the repository workflow requires a different reviewer to compile/re-run the relevant checks and approve the implementation.

## Independent re-review — current MAD handoff (2026-10-07)

**Result: NEEDS_CHANGES.** Reviewer: `/root`. Scope: current sibling `libmad-rs` worktree changes and the C parity handoff above.

`cargo test -p mad-sys` in `RustPorts/libmad-rs` fails to compile `crates/mad-sys/src/imp/bit.rs:34`. The two calls `isize::from(begin.left)` and `isize::from(CHAR_BIT - end.left)` fail because Rust does not implement `From<u16>` for `isize`. This prevents the crate and its tests from compiling. The conversions must use valid, bounds-preserving integer conversions before re-review can continue. No test ran. This is a confirmed build blocker; `mad-sys` remains `parity_blocked` until the fix is implemented and independently reviewed.

No hardware verification was performed. This re-review does not approve the decoder, Layer III state, or target arithmetic changes; those require a successful build and further source review after the compile blocker is fixed.

## 2026-10-07 implementation handoff — bit length integer conversions

Implementer: `/root`. `mad_bit_length` now casts the two bounded `u16` bit counts to `isize` directly. Their values are in the 0–8 bit-count range for valid `MadBitPtr` inputs, and the pointer-difference term remains signed until the final `u32` conversion, matching the C expression's final unsigned result.

Validation: `cargo test -p mad-sys` in `RustPorts/libmad-rs` passed all 19 tests, including the fixed independent 11-bit cursor-length assertion. Doc tests passed (0 tests). The compile blocker is fixed; the complete current MAD diff still requires independent re-review before promotion. No PowerPC or hardware test was run.

## Independent bounded conversion review (2026-10-15)

Reviewer: `/root/release_parity_review`. Result: **PASS for the bit-length conversion fix; full parity review incomplete**. The workspace imports ABI implementation files from the sibling `libmad-rs` tree through `crates/media/mad-sys/src/imp.rs`. The current `bit_length` uses signed pointer distance and valid integer casts (`../libmad-rs/crates/mad-sys/src/imp/bit.rs:28-35`), matching the expression and final unsigned conversion in `../../C-sources/libogc/libmad/bit.c:98-103` for valid bit cursors. The unsupported `isize::from(u16)` build blocker is closed.

`cargo test -p mad-sys` in the sibling workspace passed 21 tests and zero doc tests. This includes the independent 11-bit cursor result. The sibling tree has additional uncommitted decoder, frame-state, and arithmetic changes. The complete decoder callback/state comparison and the PowerPC arithmetic comparison from the earlier handoff have not been completed in this review. Keep `review_incomplete`; the passing conversion test does not establish full codec parity. This review made no change to the sibling tree. No hardware was used.

## Complete decoder review — 2026-10-15

Reviewer: `/root/review_mad_final`. Result: **NEEDS CHANGES** for two confirmed
source differences in the sibling `libmad-rs` implementation:

- A decoder with no input callback leaves `MadDecoderSync` uninitialized, then
  calls `frame_finish` and `stream_finish`. Those functions read nested pointers
  from that uninitialized allocation. C returns before nested initialization or
  cleanup in this case (`C-sources/libogc/libmad/decoder.c:319-320,457-463,
  557-562`). Fix the path and add a no-input regression.
- Rust retries `MAD_FLOW_IGNORE` only when `stream.error` is
  `MAD_ERROR_BUFLEN`; C always retries (`decoder.c:352-354`). Current tests
  encode the mismatch. Add coverage with a non-BUFLEN prior error.

The review also checked callback ordering, captured error data, BADMODE/skip
fixes, public helpers, and the configured PowerPC fixed-point profile. It found
no additional confirmed ABI mismatch. Full codec parity remains incomplete:
the recorded corpus covers host `FPM_DEFAULT`, not the current PowerPC profile.
A PowerPC build and independent C/Rust vectors are still required. The review
made no changes; no PowerPC execution or hardware test was run.

## BUG FIX handoff — 2026-10-15, sibling commit `8179f9d`

The synchronous runner now initializes and finishes nested stream/frame/synth
state only when an input callback exists. The no-input path returns success and
frees only the outer sync allocation, as C does. Input `MAD_FLOW_IGNORE` now
retries unconditionally. The regression checks retry after a non-BUFLEN error
and verifies that a no-input run returns with `decoder.sync` cleared.

Validation: `cargo test -p mad-sys` in the sibling `libmad-rs` workspace passed
22 tests. A PowerPC custom-target check passed with nightly and
`-Z build-std=core,alloc`. The PowerPC fixed-point profile still needs an
independent C/Rust vector differential. Reviewer: `/root/review_mad_final`.
The code fix is ready for independent re-review; do not mark full MAD parity
reviewed until the target differential is complete.

Independent re-review: `/root/review_mad_final` passed commit `8179f9d` for both
scoped fixes and confirmed each path against the C early return and retry loop.
The review is now incomplete only because the PowerPC fixed-point profile
still lacks an independent C/Rust vector differential.

## Public API and fixed-point re-review — 2026-10-15

Reviewer: `/root/review_mad_final`. Scope: current `mad-sys` public ABI/helper
surface and configured arithmetic against the bundled headers and C
implementation. Result: **review incomplete; no additional source mismatch
confirmed**.

The `repr(C)` public records use the Wii/GameCube widths and field ordering:
`MadBitPtr`, `MadTimerT`, `MadStream`, `MadHeader`, `MadFrame`, `MadPcm`,
`MadSynth`, and decoder callback/async/sync records. The function inventory
matches the callable bundled C API for bit, fixed, stream, header/frame,
synthesis, timer, and decoder operations. Macro-only helpers (`*_finish` where
defined as empty, options/reset/sign) do not require callable symbols.
`mad_bit_write` is declared by the header but its implementation is disabled
under `#if 0` in the bundled source; Rust correctly does not claim an active
implementation for it. The version metadata and C error-string behavior also
match the source values recorded above.

For Wii/GameCube, `gc/mad.h` selects `FPM_PPC` without `OPT_ACCURACY`; its
`mad_f_mul` result is the signed product scaled from Q56 to Q28 with
truncation. Rust's `fixed::mul` uses the same signed `i64` product and
arithmetic shift by 28. `mul_configured` selects that path for PowerPC and
retains the separate host `FPM_DEFAULT` path. Layer III requantization,
transform, stereo, alias, synthesis DCT, and target polyphase paths were
inspected for use of those helpers. Layer I/II use the same full Q28 multiply.
The PowerPC polyphase path uses the full `D` table and sums each scaled product
with the 32-bit accumulator behavior of the C fallback macros; the host SSO
window and shift remain behind non-PowerPC configuration.

This source comparison does not establish numerical PowerPC parity. The
checked-in differential report and `tools/compare_layer3.py` reference an
existing host `FPM_DEFAULT + OPT_SSO` C executable. There is no checked-in or
otherwise identified `FPM_PPC` C executable, saved PPC vector set, or runnable
PPC comparison result. A target build alone cannot verify the output values.
The smallest missing evidence is to run the same fixed deterministic MPEG
fixtures through the bundled C `FPM_PPC` build and the Rust PowerPC build, then
compare frame headers, active subband values, and active PCM values. Keep
`mad-sys` at `review_incomplete` until this comparison is recorded. The
bounded decoder fix in `8179f9d` passed its independent review; no hardware
test was performed here.
