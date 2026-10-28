# MP3Player parity implementation handoff

Date: 2026-10-02
Baseline: compiled ES-synced tree derived from `4c36294`

This pass ports the remaining MP3Player-specific behavior from `libmad/mp3player.c` without adding a second decoder implementation.

## Architecture

Both play APIs configure one shared source state and create an LWP worker with the C stack size (32768 bytes) and priority (80). The worker owns stream refill, frame decode, filtering, synthesis, resampling, and output production. A 32768-byte single-producer/single-consumer ring provides the same backpressure boundary used by C. DMA/ASND callbacks only consume that ring.

The Rust ring uses atomic cursors plus `UnsafeCell` storage. This is intentionally different from C's plain integer cursors: it preserves the same one-producer/one-consumer algorithm without relying on a Rust data race.

## Decode and filter path

`mp3player-sys` no longer depends directly on `nanomp3`. It initializes `MadStream`, `MadFrame`, and `MadSynth`, carries undecoded bytes across 32768-byte refills, appends `MAD_BUFFER_GUARD` at EOF, handles recoverable errors with the same lost-sync guard exception, calls the user's filter before synthesis, and then resamples the resulting `MadPcm`.

This makes `mad-sys` the single decoding implementation used by MP3Player. Remaining codec/subband fidelity belongs to the existing `mad-sys` parity work rather than being duplicated here.

## PCM path

The player now uses C's fixed-point clipping and 16-bit conversion, two stateful three-band filters (880 Hz low split, 5000 Hz high split, 48000 Hz mix rate), 16.16 source-rate resampling, and big-endian interleaved stereo output. Mono duplicates the left sample into the right channel.

The default AUDIO backend does not apply `MP3Player_Volume`, matching C. Volume remains stored and is used by the alternate SND backend.

## Audio backends

The default build mirrors `#ifndef __SNDLIB_H__`: three 4992-byte buffers and AUDIO DMA callback scheduling.

The `sndlib` Cargo feature mirrors the alternate C branch: three 8192-byte buffers, ASND voice 0, `set_voice`/`add_voice` callback scheduling, pointer/status checks, and `change_volume_voice` behavior.

## Lifecycle

Initialization is again explicit: the play entry points do not implicitly call `MP3Player_Init`, matching C. `MP3Player_Stop` requests worker termination, wakes a blocked producer, joins the LWP thread, and leaves backend teardown and queue closure on the worker's single cleanup path. A separate thread-created flag prevents the C implementation's immediate double-play race without duplicating cleanup logic.

## Validation status

`git diff --check` passes. This environment does not provide the Rust/PowerPC Cargo toolchain, so target compilation and hardware playback remain required before independent acceptance.
