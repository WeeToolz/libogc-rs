# Independent AESND and ASND Audit

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Existing grouped review: `/root/luna-aesnd` (`docs/review/libaesnd.md`).

Scope: independent static re-check of `aesnd`, `aesnd-sys`, and `asnd-sys` against the checked-in libogc C sources. No implementation code was changed. No build, test, or hardware verification was performed.

## Source paths inspected

- AESND Rust: `crates/media/aesnd-sys/src/lib.rs`, `crates/media/aesnd/src/lib.rs`.
- AESND C: `../../C-sources/libogc/gc/aesndlib.h`, `../../C-sources/libogc/libaesnd/aesndlib.c`, `../../C-sources/libogc/libaesnd/dspcode/dspmixer.s`.
- ASND Rust: `crates/media/asnd-sys/src/lib.rs`.
- ASND C: `../../C-sources/libogc/gc/asndlib.h`, `../../C-sources/libogc/libasnd/asndlib.c`, `../../C-sources/libogc/libasnd/dsp_mixer/dsp_mixer.s`.

## Historical findings re-checked

This section records the 2026-10-01 independent audit. Later parity passes supersede these findings; the current-source status is at the end of this document.

1. **AESND voice sample playback gap confirmed (`aesnd-sys`).** C's `__aesndfillbuffer` and `__aesndhandlerequest` copy voice sample data into stream buffers and advance the sample cursor (`libaesnd/aesndlib.c:242-303`). The C DMA callback launches the DSP command for a selected voice (`:371-407`), and the mixer assembly processes samples (`dspcode/dspmixer.s:76-97,126-159`). Rust's `audio_dma_callback` chooses `audio_buffer`, invokes an optional application callback, starts DMA, and calls `tick_voice` (`crates/media/aesnd-sys/src/lib.rs:543-592`); `tick_voice` advances playback bookkeeping but no Rust path reads sample bytes or mixes voices into that buffer. Thus voice playback alone does not produce C-equivalent audio. The application callback can write DMA samples, so the precise claim is that Rust voice playback itself does not generate the output.

2. **AESND DSP metrics remain zero (`aesnd-sys`).** Rust initializes `dsp_process_time_us` to zero (`crates/media/aesnd-sys/src/lib.rs:280-317`), and the crate only reads it in metric getters (`:643-649`). C records elapsed DSP time in `__dsp_requestcallback` (`libaesnd/aesndlib.c:322-328`) and converts/reports time and usage (`:508-529`).

3. **AESND default frequency mismatch remains for GameCube (`aesnd-sys`).** C selects `54000000.0f/1124.0f` under `HW_DOL`, and 48000 under `HW_RVL` (`gc/aesndlib.h:10-14`). Rust fixes `DSP_DEFAULT_FREQ` at 48,000 (`crates/media/aesnd-sys/src/lib.rs:16`) and uses it to calculate frequency ratio (`:365-369`). The Rust crate target description includes Wii and GameCube. The Wii default matches; the GameCube base does not.

4. **The prior ASND little-endian 16-bit finding is not present in the current Rust source.** The existing grouped note says formats 5 and 7 fall through to silence. Current `read_voice_frame` has explicit `VOICE_MONO_16BIT_LE` and `VOICE_STEREO_16BIT_LE` branches using `i16::from_le_bytes` (`crates/media/asnd-sys/src/lib.rs:217-267`). `set_voice_inner` preserves the low three format bits and stores the selected format (`:684-729`). C's selector table maps entries 5 and 7 to `mono_16bits_le` and `stereo_16bits_le` (`libasnd/dsp_mixer/dsp_mixer.s:774-782`). These two formats therefore agree at the format-selection and byte-decoding level in the current tree. The earlier finding is stale and should be removed or marked fixed in the shared review note.

5. **ASND end-state clearing mismatch confirmed (`asnd-sys`).** Rust `end()` stops DMA and clears pause, global time/callback/metrics, current buffer, and every voice (`crates/media/asnd-sys/src/lib.rs:980-995`). C `ASND_End` stops DMA, unregisters its callback, shuts down DSP, and clears only `asnd_inited` (`libasnd/asndlib.c:396-410`). C's global counter, pause state, callback, and voice records are not cleared there; first-time `ASND_Init` clears voice records and resets pause/counters (`:357-385`). Querying state after `end()` can therefore return different values.

6. **ASND paused status precedence mismatch confirmed (`asnd-sys`).** C first sets `SND_UNUSED` when the voice is inactive and then overrides it with `SND_WAITING` if `VOICE_PAUSE` is set (`libasnd/asndlib.c:639-651`). Rust returns `SND_UNUSED` for an inactive voice before checking the pause flag (`crates/media/asnd-sys/src/lib.rs:1216-1227`). A stopped voice subsequently paused therefore reports `SND_WAITING` in C and `SND_UNUSED` in Rust.

## Limits and workflow state

The Rust ASND implementation uses a software mixer (`crates/media/asnd-sys/src/lib.rs:802-959`) where C uses a DSP task. Exact resampling, interpolation, callback timing, and audible output parity are not established by this static audit. AESND/ASND ABI and API listings in `docs/review/libaesnd.md` were not exhaustively re-enumerated in this addendum; the checks above target the existing behavioral claims and their directly related paths.

The shared `config/static-review-scope.csv` has unrelated staged and unstaged edits. Its AESND rows are owned by the existing reviewer and the index contains broad concurrent ledger changes. I did not edit or stage the ledger. This was the workflow state at the time of the audit. It is superseded by the current-source status below.


## Current-source status after lifecycle cleanup

This audit is historical. Findings 1-4 were already superseded by later AESND/ASND source changes before this pass. Findings 5 and 6 are now also resolved: `ASND_End` retains C-visible state, and `ASND_StatusVoice` checks pause before inactivity. A fresh lifecycle comparison also corrected repeated `AESND_Init` and `AESND_Reset` DMA-callback behavior. See `libaesnd.md` for the current status.
