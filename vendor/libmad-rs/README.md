# libmad-rs

This copy comes from local libmad-rs commit
`4d13614e86d834590bcd895740a5d85b46ac8d9d`. Its manifest and one test fixture
path were adjusted for this repository. The Layer III reference fixture is
not included; see [dependency notes](../../docs/dependencies.md) for how to
provide it externally. Its generation details were not recorded in the source
repository.

This crate is a pure Rust port derived from libmad, the MPEG audio decoder by
Underbit Technologies, Inc. It preserves the upstream GPL-2.0-or-later terms;
see [LICENSE](LICENSE) and the original `C-sources/libmad-c/COPYING` file.

The port follows the upstream C behavior unless that behavior depends on C
undefined behavior or host details. Such differences will be documented here.

## Port status

Foundation work covers fixed-point arithmetic, bit access, timers, stream
buffer state, and MPEG frame header parsing. Layer I and II are ported,
including allocation, joint stereo, scalefactors, CRC checks, requantization,
and direct subband output. Layer III now includes side-information parsing,
main-data reservoir assembly, scalefactor band-width tables, scalefactor and
exponent decoding, fixed-point requantization, Huffman pair/quad decoding,
joint stereo processing, short-block reordering, alias reduction, long/short
IMDCT, overlap, and frequency inversion. `Layer3Decoder::decode` integrates
these stages for one frame. The synchronous decoder callback driver still
receives frame decode and synthesis operations through callbacks. The
`mad-sys` crate provides the C-compatible Wii/GameCube ABI structs and stream,
frame, synthesis, and synchronous decoder wrappers. It uses only the local
`libmad-rs` codec.

Historical host C versus Rust comparisons used `FPM_64BIT` with
`OPT_ACCURACY` and five generated streams: MPEG-1 stereo at 44.1 kHz, MPEG-2
mono at 22.05 kHz, MPEG-1 mono at 32 kHz, MPEG-2 mono at 16 kHz, and MPEG-2.5
mono at 11.025 kHz. Frame headers and counts matched. Those comparisons had
maximum subband differences of 18, 17, 14, 14, and 14 fixed-point units, and
maximum active PCM differences of 96, 82, 124, 106, and 112 units. They do not
measure parity with the `FPM_DEFAULT` build targeted by the current arithmetic
port.

The current parity target is the existing host C build configured with
`FPM_DEFAULT`, without `OPT_ACCURACY` or `OPT_SPEED`. `synth.c` enables its SSO
path for this profile. Rust now ports the corresponding FPM_DEFAULT multiply
rounding in Layer III requantization and stereo processing; the factorized
active fast long IMDCT and short IMDCT operation graphs;
and the SSO factorized DCT, Q16 filter values, Q14 window coefficients, and
wrapping polyphase accumulation. The fresh end-to-end comparison starts both
decoders from MP3 bytes. All 419 frames across 15 streams match: six recorded
header fields, 512,640 active subband values, 512,640 PCM values, recoverable
error counts, and final error codes. The corpus includes MPEG-1, MPEG-2, and
MPEG-2.5; all four block types; mixed blocks; mid/side and intensity stereo;
their combined mode; and both MPEG-2 intensity scale choices. This establishes
bit-exact output for the tested host build and corpus. It does not establish
runtime parity with the Wii/GameCube `FPM_PPC` C build. The detailed
comparison report belonged to the original decoder repository and is not
included in this checkout. See the [MAD review](../../docs/review/libmad.md)
for available project evidence and current limits.
Rust clears unused PCM samples each frame, while C leaves samples beyond the
output length and in unused channels unchanged.

`Stream::buffer` owns a copy of its input so the stream cannot outlive the
borrowed bytes. The C API stores the caller's buffer pointer. This changes
ownership and lifetime only; parsing sees the same byte sequence. `fixed::div`
returns zero for a zero denominator, where C has undefined behavior. Bit
writing is provided as a safe helper; upstream's `mad_bit_write` body is
disabled in `bit.c` and has no compiled C behavior to match.

The ABI wrapper exposes reservoir bytes through `MadStream.main_data` and
persistent overlap samples through `MadFrame.overlap`. It imports caller
changes before Layer III decode and publishes the updated state afterward.
Callers can inspect or change reservoir bytes. They must preserve the
allocation pointer and let `mad_stream_finish` release it. The wrapper allocates
overlap storage as needed; `mad_frame_mute` clears it and `mad_frame_finish`
releases it.

The Rust and C ABI synchronous decoders use one stage-control loop. Each
adapter retains its own callback and frame storage. The C adapter reads callback
changes during a run, except for the error callback and its data pointer, which
are saved at run entry. Input `Ignore` retries input. Terminal errors other than
buffer exhaustion produce failure; buffer exhaustion requests more input.
The Rust API returns `DecodeStatus::Failed(error)` for terminal errors.
Async mode returns an error, as it does in a libmad build without `USE_ASYNC`.

The ABI layouts use 32-bit `long` widths for Wii/GameCube PowerPC. Both
checked-in `powerpc-nintendo-none` target specs in `RustPorts/libogc-rs` passed
`cargo check` with nightly `build-std=core,alloc` and warnings denied. The
GameCube check also enabled `cfg(gamecube)`. These checks validate compilation
for both console targets; hardware playback remains unverified.
