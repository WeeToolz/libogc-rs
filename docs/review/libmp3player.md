# `mp3player-sys` static review

Reviewer: `/root/reviewer_6`  
Implementer: `notOrrytrout`; implementation commit `80b8b02` (MP3 playback backend); callback ABI correction `8864287`.  
Review type: first-pass static comparison.  
Validation: source inspection only; no build or tests run as instructed. No hardware verification was performed.

## Scope and sources

Rust: `crates/media/mp3player-sys/src/lib.rs`, manifest `crates/media/mp3player-sys/Cargo.toml`. This implementation decodes with `nanomp3` (`lib.rs:10-12`) and drives the AUDIO DMA API.

C: public API `../../C-sources/libogc/gc/mp3player.h`; implementation `../../C-sources/libogc/libmad/mp3player.c`.

## Confirmed findings

### Play calls perform decode and reader callbacks synchronously

C `MP3Player_PlayBuffer` and `MP3Player_PlayFile` create `StreamPlay` on an LWP thread and return after thread creation (`mp3player.c:232-260`). That thread reads input (`:308-335`), decodes frames and calls the filter (`:337-345`), while DMA callbacks transfer/refill buffers (`:447-471`). Rust calls `start_playback` directly from both public play functions (`lib.rs:352-379`, `:384-412`). It fills all three DMA buffers synchronously (`:302-317`), and `fill_output` calls `decode_next_frame` and therefore the reader/filter callbacks before starting DMA (`:229-279`, `:165-221`). A slow or blocking file reader consequently blocks the caller in Rust, whereas C returns and performs reads on the stream thread. Rust also completes decoding of short inputs before audio starts. The API comments at `:351` and `:383` describe asynchronous lifetime but do not make the call itself asynchronous.

### Filter callback receives a synthetic, incomplete frame after decode

C invokes `mp3filterfunc(&Stream, &Frame)` after `mad_frame_decode` and before `mad_synth_frame` (`mp3player.c:337-345`); the filter sees the real stream and decoded frame and can affect the PCM that is synthesized. Rust instead decodes into PCM first (`lib.rs:193-209`), then `invoke_filter` creates default `MadStream` and `MadFrame` values and fills only layer, sample rate, bitrate, and mode (`:147-163`). It never provides the input stream state or decoded frame data to the callback. Since output PCM was already produced by `nanomp3`, callback changes cannot affect playback. The corrected `extern "C"` callback ABI in `8864287` matches the C calling convention, but its arguments and processing effect still do not match C.

### Volume changes output in Rust's default DMA path but not C's default DMA path

The C default backend (`#ifndef __SNDLIB_H__`) initializes AUDIO DMA at `mp3player.c:218-229`; in this path `MP3Player_Volume` only clamps and stores `mp3_volume` (`:474-482`). That value is used for `SND_SetVoice` only in the alternate SNDLIB backend (`:191-194`), not in the normal DMA PCM conversion (`:385-404`). Rust always multiplies every decoded sample by `VOLUME / 255` in `scaled_sample` (`lib.rs:224-227`, `:229-269`) and `mp3_player_volume` updates that scalar (`:345-347`). Thus volume zero mutes Rust DMA playback but does not mute the corresponding default C DMA playback; non-maximum volume levels also differ.

### Rust omits the C three-band processing path

C initializes two per-channel EQ states for each stream (`mp3player.c:275-295`), then `Resample` applies `Do3Band` to each sample before writing the output ring (`:385-404`). Rust resamples decoded float PCM and writes it directly after scaling (`lib.rs:229-279`), with no equivalent filter state or `Do3Band` processing. The C default EQ gains are all 1.0 (`mp3player.c:406-416`), but the filter is still stateful and its numerical path (including quantization and `VSA`) is not present in Rust. This is an observable PCM mismatch even at default gains.

### Playback is not implemented with C's selectable audio backends

C selects between AUDIO DMA and SNDLIB at compile time. Its buffer size is 4992 bytes for AUDIO DMA and 8192 bytes for SNDLIB (`mp3player.c:27-34`), and SNDLIB uses voice callbacks and volume control (`:191-194`, `:447-471`). Rust hardcodes 4992-byte buffers (`lib.rs:19-22`) and only calls `ogc_audio_sys` DMA functions (`:281-317`). A Rust build cannot reproduce the C SNDLIB configuration or its callback scheduling and voice-volume behavior.

## Coverage and limitations

The crate has no local test module or tests directory. The callback calling convention was corrected in `8864287`, but that commit does not address callback arguments or timing. No test/build validation was performed. The repository's related `mad-sys` review separately records that its callbacks do not receive C-equivalent decoded frame data; this review independently confirms the MP3 player itself supplies synthetic frame objects after decoding.

## Review state

`mp3player-sys` remains `pending,finding`, owned by `/root/reviewer_6`. No implementation code was changed.

## 2026-10-02 parity implementation at the ES-synced baseline

The five findings above are historical after this pass.

- `MP3Player_PlayBuffer` and `MP3Player_PlayFile` now only install source state and create the 32 KiB LWP stream worker. Input callbacks, decode work, filtering, synthesis, resampling, and ring-buffer writes execute on that worker instead of the caller.
- The player now decodes through `mad-sys` rather than maintaining a second `nanomp3` decoder. The callback receives the live `MadStream` and decoded `MadFrame` immediately after `frame_decode` and before `synth_frame`, so frame changes feed the following synthesis step.
- The default AUDIO DMA path no longer scales PCM by `MP3Player_Volume`. The stored volume is applied only by the optional `sndlib` backend, matching C's `__SNDLIB_H__` split.
- The two C three-band states are initialized at 880 Hz / 5000 Hz for 48 kHz output and each resampled sample passes through the same four-pole low/high state update before it enters the output ring.
- The `sndlib` Cargo feature mirrors C's alternate backend: 8192-byte output buffers, ASND voice 0, callback-driven `SND_AddVoice` scheduling, and live volume changes. The default backend retains 4992-byte AUDIO DMA buffers.

The output ring is still single-producer/single-consumer like C, but its Rust indices and primed state are atomics and the byte storage uses `UnsafeCell`. This avoids introducing Rust data-race UB while preserving the C scheduling model. Source decoding, resampling, and both public play entry points share the same worker and ring implementation.

One limitation belongs below this crate: `mad-sys` still uses its nanomp3-backed decoder bridge and PCM-derived public subband data. `mp3player-sys` no longer duplicates that decoder, so future libmad fidelity work is centralized in `mad-sys`.

No PowerPC compile or hardware run was available in this environment. The implementation therefore remains at re-review until the repository's target build and hardware/audio checks are run.

## 2026-10-03 independent re-review: PASSED

Reviewer: reviewer_d. Implementer: notOrrytrout. Implementation commit: `c5173e2` (`Complete ES and MP3 player parity`); the Rust `mad-sys` backend replacement is in `1a473e7`, and callback ABI correction is in `8864287`.

I inspected the complete player change in `c5173e2` and traced the current implementation against `gc/mp3player.h`, `libmad/mp3player.c`, and the C libmad stream, frame, and synthesis sources. The worker lifecycle matches the C model: both play entry points install source/filter state and create an LWP thread at priority 80 with a 32 KiB stack (`mp3player.c:232-260`; Rust `mp3player-sys/src/lib.rs:569-585,644-701`). C reads and decodes on that worker, invokes the filter after frame decode and before synthesis, resamples, and waits for queued audio to drain before backend cleanup (`mp3player.c:275-374`). Rust follows that order, passes the live `MadStream` and decoded `MadFrame` to the filter before `synth_frame`, and drains its output before cleanup (`lib.rs:400-473,513-567`). This closes the earlier synchronous-call and synthetic-filter-argument findings.

The input refill and guard-byte flow match the C sequence: preserve bytes from `next_frame`, append reader/buffer input, append `MAD_BUFFER_GUARD` on EOF, pass the combined length to the stream, and use the recoverable-error / lost-sync guard test (`mp3player.c:308-354`; Rust `lib.rs:326-352,417-467`; C error classes in `libmad/stream.h:31-58`). The Rust resampler uses the same 16.16 source-rate increment, nearest lower source sample selection, `FixedToShort` clipping/shift, mono/stereo channel handling, and interleaved big-endian 16-bit output as C (`mp3player.c:97-107,385-404`; Rust `lib.rs:355-397`). The two EQ states use the same 880/5000 Hz, 48 kHz coefficients, state update order, default unity band gains, VSA term, and per-channel processing (`mp3player.c:406-445`; Rust `lib.rs:83-151`). Rust uses `libm::sinf` for the coefficient calculation in place of the platform `sinf`; no exact floating-point or output-bit identity is claimed.

The ring-buffer producer/consumer scheduling and backend buffer cadence also match the C structure. Both use a 32 KiB output ring, begin playback when it contains at least half capacity, transfer up to a 4992-byte AUDIO DMA buffer or 8192-byte SND buffer, and signal/sleep on the worker queue as data is consumed/space is needed (`mp3player.c:27-34,109-199,447-471`; Rust `lib.rs:27-31,44-80,223-323,475-510`). Rust atomics and `UnsafeCell` implement the same single-producer/single-consumer ownership without using unsynchronized Rust data races. The AUDIO backend starts and cycles three buffers at the same indices as C and flushes consumed output before DMA (`mp3player.c:183-196,447-453`; Rust `lib.rs:254-262,475-483`). The `sndlib` feature uses ASND voice 0, 48 kHz stereo 16-bit samples, C-equivalent callback pointer/status checks, and updates live channel volume (`mp3player.c:191-194,447-480`; Rust `lib.rs:264-283,485-510,634-638`). The default AUDIO path keeps volume out of the PCM conversion, as C does; volume is stored/applied only by the SND backend (`mp3player.c:474-482`; Rust `lib.rs:634-638`).

The lower `mad-sys` boundary is separate from the player changes. Current `mad-sys` forwards its public ABI through the `libmad-rs` implementation (`crates/media/mad-sys/src/imp.rs:1-24`; `crates/media/mad-sys/src/lib.rs:1-12`). The player now correctly consumes that boundary's frame/stream/synthesis API, and the bridge passes callback-edited subband samples into synthesis (`../libmad-rs/crates/mad-sys/src/imp/synth.rs:10-20,101-115`). The library's decoder and polyphase synthesis algorithms are a Rust port, not the checked-in C libmad implementation. This player review does not establish exhaustive bit-exact decoder/synthesis parity at that lower boundary; keep any such MAD codec audit assigned to `mad-sys` rather than treating it as a remaining player scheduling/API mismatch.

Validation: `cargo check -p mp3player-sys --target powerpc-unknown-linux-gnu` passed. `cargo check -p mp3player-sys --features sndlib --target powerpc-unknown-linux-gnu` also passed. There is no local MP3 player test module, and these commands compile only; no tests ran. No hardware verification was performed. Result: `PASSED` for the `mp3player-sys` implementation changes reviewed here; this does not certify bit-exact output or the separate `mad-sys` codec algorithms.
