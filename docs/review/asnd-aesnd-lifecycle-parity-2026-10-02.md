# ASND/AESND lifecycle parity pass

Date: 2026-10-02

This pass rechecked the current Rust audio implementations against `libasnd/asndlib.c` and `libaesnd/aesndlib.c` after the larger AESND playback work. It intentionally targets observable lifecycle/status differences rather than replacing the Rust CPU mixers with libogc DSP tasks.

## ASND

`ASND_End` in C only stops DMA, unregisters its DMA callback, removes the DSP task, and clears `asnd_inited`. It does not clear `global_pause`, `global_counter`, `global_callback`, DSP timing, or `sound_data`. Rust now follows that state-retention rule; `reset_runtime_state` remains centralized in the first-time `init` path.

C `ASND_StatusVoice` first derives unused/working status and then overwrites it with waiting when `VOICE_PAUSE` is set. Rust now uses the same precedence, so a paused inactive voice reports `SND_WAITING`.

## AESND

C calls `AUDIO_RegisterDMACallback(__audio_dma_callback)` on every `AESND_Init`, even when AESND is already initialized. Repeated init does not clear global pause or voice state. Rust now does the same.

C `AESND_Reset` unregisters the DMA callback with `NULL`; it does not restore whatever callback happened to be present before AESND initialization. The Rust-only previous-callback bookkeeping was removed. Reset now leaves PB/command state intact until the next first-time init, while advancing the internal software-mixer session so an in-flight callback cannot commit stale work after reset.

## Remaining architectural difference

Both Rust audio implementations still use CPU-side mixing where C uses DSP tasks. Current source covers the previously identified sample-format, frequency-phase, playback, and metric gaps, but exact DSP scheduling, callback deadlines, and bit-exact output require target hardware validation.
