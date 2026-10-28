# AESND and ASND Static Review

Date: 2026-10-01

Scope: read-only source comparison of `aesnd`, `aesnd-sys`, and `asnd-sys` with the checked-in libogc C implementation. This note does not claim real-hardware verification.

## Scope and source coverage

The review enumerated every public item in these Rust crate roots and compared its shape or behavior with the corresponding C declarations and implementations:

| Rust crate | Rust source | C interface and implementation | Coverage |
| --- | --- | --- | --- |
| `aesnd-sys` | `crates/media/aesnd-sys/src/lib.rs` | `../../C-sources/libogc/gc/aesndlib.h`; `../../C-sources/libogc/libaesnd/aesndlib.c` | All constants, callback aliases, `Aesndpb`, lifecycle, callback registration, allocation/free, setters, and play APIs. Traced DMA, stream-fill, DSP-request, and DSP-mixer paths. |
| `aesnd` | `crates/media/aesnd/src/lib.rs` | Same AESND C interface | All wrapper exports, safe ownership types, buffer replacement/drop path, and high-level wrappers. |
| `asnd-sys` | `crates/media/asnd-sys/src/lib.rs` | `../../C-sources/libogc/gc/asndlib.h`; `../../C-sources/libogc/libasnd/asndlib.c` | All constants, types, lifecycle/time/callback APIs, voice setup/queue/control/status/query APIs, DSP metrics, note helpers, owned helpers, and the mixer path. |

The C build files, `libaesnd/CMakeLists.txt` and `libasnd/CMakeLists.txt`, include the checked-in DSP assembly sources `libaesnd/dspcode/dspmixer.s` and `libasnd/dsp_mixer/dsp_mixer.s`. The C build generates and embeds DSP binary data. The generated headers/binaries and build output are not present in the checked-in source tree, so the review compares against the assembly sources and C request handlers, not a built DSP image.

No compile, test, or hardware commands were run for this review. The Rust tests are target-gated in the crate sources; their results were not checked here.

## Public API mapping

### AESND

The public constants in `aesnd-sys/src/lib.rs:13-32`, callback aliases at `:117-122`, and opaque-layout `Aesndpb` at `:46-76` map to `aesndlib.h:6-41`. The field order also follows the private C structure in `aesndlib.c:29-63`; C keeps the structure opaque in the public header.

| Rust API | C API |
| --- | --- |
| `init`, `reset`, `pause`, `get_dsp_process_time`, `get_dsp_process_usage` | `AESND_Init`, `AESND_Reset`, `AESND_Pause`, `AESND_GetDSPProcessTime`, `AESND_GetDSPProcessUsage` |
| `register_audio_callback_with_arg`, `register_audio_callback` | `AESND_RegisterAudioCallbackWithArg`, header inline `AESND_RegisterAudioCallback` |
| `allocate_voice_with_arg`, `allocate_voice`, `free_voice` | `AESND_AllocateVoiceWithArg`, header inline `AESND_AllocateVoice`, `AESND_FreeVoice` |
| `set_voice_stop`, `set_voice_mute`, `set_voice_loop`, `set_voice_format`, `set_voice_stream`, `set_voice_frequency`, `set_voice_volume`, `set_voice_delay`, `set_voice_buffer`, `play_voice` | Matching `AESND_SetVoice*` functions and `AESND_PlayVoice` |
| `register_voice_callback_with_arg`, `register_voice_callback` | `AESND_RegisterVoiceCallbackWithArg`, header inline `AESND_RegisterVoiceCallback` |

The higher-level `aesnd` crate reexports the sys crate and adds `OwnedAudioBuffer`, `VoiceHandle`, and `OwnedPlayback` (`aesnd/src/lib.rs:12-329`), then wraps lifecycle, metrics, callback registration, allocation, and free (`:332-372`). These ownership helpers have no direct C counterpart.

### ASND

ASND constants, note constants, and callback types (`asnd-sys/src/lib.rs:22-92`) map to `asndlib.h:41-183`. Rust's `note()` maps to C's `NOTE(note, octave)` macro (`asndlib.h:146`). The public functions map as follows:

| Rust API | C API |
| --- | --- |
| `a_note2_freq` | `ANote2Freq` |
| `init`, `end`, `pause`, `is_paused`, `get_time`, `get_sample_counter`, `get_samples_per_tick`, `set_time`, `set_callback`, `get_audio_rate` | `ASND_Init`, `ASND_End`, `ASND_Pause`, `ASND_Is_Paused`, `ASND_GetTime`, `ASND_GetSampleCounter`, `ASND_GetSamplesPerTick`, `ASND_SetTime`, `ASND_SetCallback`, `ASND_GetAudioRate` |
| `set_voice`, `set_infinite_voice`, `add_voice` | `ASND_SetVoice`, `ASND_SetInfiniteVoice`, `ASND_AddVoice` |
| `test_voice_buffer_ready`, `test_pointer`, `pause_voice`, `stop_voice`, `status_voice`, `change_volume_voice`, `get_tick_counter_voice`, `get_timer_voice`, `get_first_unused_voice`, `change_pitch_voice` | Matching `ASND_TestVoiceBufferReady`, `ASND_TestPointer`, `ASND_PauseVoice`, `ASND_StopVoice`, `ASND_StatusVoice`, `ASND_ChangeVolumeVoice`, `ASND_GetTickCounterVoice`, `ASND_GetTimerVoice`, `ASND_GetFirstUnusedVoice`, `ASND_ChangePitchVoice` |
| `get_dsp_percent_use`, `get_dsp_process_time` | `ASND_GetDSP_PercentUse`, `ASND_GetDSP_ProcessTime` |

`VoiceConfig`, `InfiniteVoiceConfig`, owned buffer/config/error/voice types and `set_voice_owned`, `set_infinite_voice_owned` are Rust-only convenience APIs (`asnd-sys/src/lib.rs:387-669`, `:1110-1192`). The C header's `SND_*` names are preprocessor compatibility aliases for `ASND_*` (`asndlib.h:151-178`); Rust exposes the idiomatic function names above rather than duplicate aliases.

## Historical findings from the initial review

The numbered items in this section describe the 2026-10-01 source state. Later sections record the fixes; do not treat these items as current findings.

1. **AESND playback does not produce voice samples.** C initializes and starts the DSP mixer task (`libaesnd/aesndlib.c:410-424`, `:427-469`), starts DSP work for the selected voice (`:371-407`), and copies source samples into stream buffers (`:239-303`). The DSP mixer source processes 96 samples per request and handles sample formats (`libaesnd/dspcode/dspmixer.s:76-97`, `:126-159`). Rust's DMA callback selects an initially zeroed audio buffer and passes it to the optional callback (`aesnd-sys/src/lib.rs:543-566`), then only advances bookkeeping in `tick_voice` (`:584-590`, `:441-540`). No Rust code reads source samples or writes a voice mix into `audio_buffer`. With no application audio callback that writes samples, allocated and played voices therefore send silence. This is a functional playback mismatch, not an unverified timing claim.

2. **AESND DSP metrics are permanently zero.** Rust initializes `dsp_process_time_us` to zero (`aesnd-sys/src/lib.rs:299-317`) and only reads it in the metric getters (`:643-649`); there is no assignment elsewhere in the crate. C records DSP elapsed time in its request callback (`libaesnd/aesndlib.c:322-328`) and returns the converted process time and percentage (`:508-529`). Rust therefore reports zero process time and usage for all runs.

3. **AESND GameCube default frequency differs.** The C header selects `54000000.0f/1124.0f` for `HW_DOL` and `48000` for `HW_RVL` (`gc/aesndlib.h:10-14`). Rust hardcodes `DSP_DEFAULT_FREQ = 48_000.0` (`aesnd-sys/src/lib.rs:13-16`), while its crate-level target message permits both Wii and GameCube (`:3-5`). On the C GameCube configuration, frequency ratios from `set_voice_frequency_inner` (`:365-369`) use a different base.

4. **ASND little-endian 16-bit playback decodes as silence.** Rust declares mono and stereo LE formats (`asnd-sys/src/lib.rs:42-45`) and computes their frame sizes (`:191-199`), but `read_voice_frame` handles format 5 and 7 in no branch; both reach the zero-sample fallback (`:217-267`). The C DSP sample selector explicitly dispatches format entries 5 and 7 to `mono_16bits_le` and `stereo_16bits_le` (`libasnd/dsp_mixer/dsp_mixer.s:774-782`). Such voices are accepted by Rust `set_voice` (`:684-729`) but mix as silence.

5. **ASND `end()` clears observable state that C retains.** Rust clears the time counter, callback, pause/time metrics, and voice state in `end()` (`asnd-sys/src/lib.rs:980-995`). C `ASND_End` stops DMA and the DSP task, unregisters the DMA callback, and clears only `asnd_inited` (`libasnd/asndlib.c:396-410`). C's query/setter functions continue to use `global_pause`, `global_counter`, `global_callback`, and `sound_data` after end (`:694-747` and `:751-798`); those values are retained until the next first-time initialization clears them (`:357-385`). Calls such as `ASND_GetTime`, `ASND_Is_Paused`, and `ASND_SetCallback` after `ASND_End` thus differ.

6. **ASND paused status precedence differs for an inactive voice.** C starts `ASND_StatusVoice` at `SND_WORKING`, sets `SND_UNUSED` when inactive, then overrides that result with `SND_WAITING` whenever `VOICE_PAUSE` is set (`libasnd/asndlib.c:639-651`). Rust returns early by branch priority: an inactive voice is `SND_UNUSED` even if `pause_voice` has set its pause flag (`asnd-sys/src/lib.rs:1202-1227`). Therefore the valid sequence `stop_voice(v); pause_voice(v, 1); status_voice(v)` returns `SND_WAITING` in C and `SND_UNUSED` in Rust.

## Intentional Rust implementation differences and additions

- `asnd-sys` implements the mixer in Rust on the audio DMA callback path (`asnd-sys/src/lib.rs:802-959`) instead of installing the C DSP task (`libasnd/asndlib.c:348-389`). It writes mixed output and flushes the cache before starting the next DMA (`asnd-sys/src/lib.rs:895-898`, `:946-958`). This is a deliberate implementation change in the Rust source, but matching DSP interpolation, cycle timing, and audible output has not been verified.
- `aesnd-sys` models voice progress in software and keeps the C-shaped PB/callback API, while the C implementation depends on an installed DSP mixer and stream-copy path. The absence of sample mixing is recorded above as a confirmed functional gap.
- Rust adds owned sample buffers and RAII voice/playback wrappers in `aesnd` and `asnd-sys`. These are not C API functions. Their lifetime guarantees have only been reviewed statically, not validated against asynchronous hardware operation.
- `a_note2_freq` uses a compile-time ratio table and checked shifts, and uses Euclidean modulo for negative notes (`asnd-sys/src/lib.rs:732-751`); C constructs its table from the `1.0594386f` recurrence and uses C remainder/indexing (`libasnd/asndlib.c:803-833`). Normal nonnegative note inputs map by design; negative and extreme inputs differ or are outside safe C behavior. No confirmed normal-input mismatch is claimed here.

## Uncertain differences

- ASND's Rust software mixer uses integer frame selection, volume scaling, summing, and final clamping (`asnd-sys/src/lib.rs:813-893`). The C path uses the DSP accelerator and assembly mixer (`libasnd/dsp_mixer/dsp_mixer.s`). The output may differ for resampling/interpolation, clipping, exact sample boundaries, and update timing. This source review did not prove the exact output delta for every format or pitch.
- C ASND invokes stream callbacks from DSP request lookahead around per-voice processing (`libasnd/asndlib.c:129-250`, `:257-346`); Rust invokes callbacks from its software buffer-boundary path (`asnd-sys/src/lib.rs:774-845`). Exact callback timing and double-buffer refill deadlines need target runtime validation.
- Rust ASND DSP-use metrics time the CPU mixer (`asnd-sys/src/lib.rs:901-959`); C reports DSP-task time (`libasnd/asndlib.c:785-799`). The quantity is not directly comparable, but this is expected from the replacement implementation rather than evidence that the Rust calculation itself is incorrect.
- `OwnedVoice` and `OwnedPlayback` stop/detach before freeing buffers (`asnd-sys/src/lib.rs:641-669`; `aesnd/src/lib.rs:275-329`). Static source review does not establish that this is safe against all hardware/DMA/DSP in-flight reads.

## Explicit unchecked areas

- No Wii or GameCube hardware playback, callback timing, DMA cadence, cache coherency, DSP timing, or audible output was checked.
- No Rust build, test, target check, or regression test was run in this review.
- The generated DSP binaries/embedded-image outputs referenced by both CMake files are absent from the checked-in tree. The checked-in assembly files and C handlers were reviewed; binary build output was not.
- No ported application callers were audited for reliance on edge-case lifecycle, callback, or compatibility-macro behavior.

## 2026-10-02 AESND playback parity implementation

Compared again with current `libaesnd/aesndlib.c`, `gc/aesndlib.h`, and the checked-in DSP mixer source.

The confirmed silent-playback gap is addressed by a CPU mixer on the existing 48 kHz DMA cadence. It reads all eight AESND source formats, applies per-voice 16.16 frequency stepping, delay, loop/once/stream state, per-channel `0x100`-unity volume scaling, saturating multi-voice accumulation, and writes the 96-frame stereo big-endian output buffer before DMA. Voice callbacks are dispatched outside the interrupt-protected state guard; `VOICE_STATE_RUNNING` is emitted for each active voice on each mix cycle, and stream/stopped callbacks follow source-buffer transitions.

The DMA path now records mixer processing time instead of leaving the metric permanently zero. GameCube uses C's `54000000.0f / 1124.0f` default frequency, while Wii remains 48000 Hz. The output buffer is cache-flushed before DMA.

This intentionally uses a Rust CPU mixer rather than installing libogc's DSP program. Therefore exact DSP interpolation, cycle timing, callback deadlines, and bit-exact samples are not claimed. Those remain hardware/audio-probe validation items. The separate ASND findings in this document are unchanged.

## 2026-10-02 DSP phase/timing parity follow-up

The CPU mixer was compared directly with `libaesnd/dspcode/dspmixer.s`, not only with `aesndlib.c`.

The C DSP does fractional zero-order hold rather than linear interpolation: for each 48 kHz output frame it first mixes the previously latched left/right sample, advances the 16.16 source-rate accumulator, and fetches zero or more source frames only when that accumulator crosses an integer boundary. Rust now follows that same latch/phase order. Rates below 1x therefore repeat the previous sample for the same number of output frames as C, while rates above 1x consume multiple source frames and retain the final fetched sample for the next output frame. Delay conversion also uses C's wrapping `delay * 48` arithmetic.

Pure regression coverage pins 0.5x and 1.5x accumulator sequences in addition to all eight C sample formats.

The implementation remains a CPU mixer rather than the original DSP task. Exact DSP/ARAM scheduling, concurrent DMA-vs-DSP timing, and bit-exact hardware output are still hardware-validation items; this pass closes the identified fractional source-phase mismatch, not those architectural differences.


## 2026-10-02 ASND/AESND lifecycle parity cleanup

A fresh comparison against the current C sources closed the remaining contained lifecycle/status mismatches from the earlier audit.

- `ASND_End` now matches C by stopping DMA and unregistering the callback while retaining observable pause, time, callback, DSP metric, and voice state. The next first-time `ASND_Init` remains the single reset point.
- `ASND_StatusVoice` now gives `VOICE_PAUSE`/`SND_WAITING` precedence over the inactive/`SND_UNUSED` result, matching the C assignment order.
- Repeated `AESND_Init` no longer clears global pause or otherwise resets live state. It installs the AESND DMA callback on every call, as C does.
- `AESND_Reset` now unregisters the DMA callback with `NULL` instead of restoring an earlier callback. It marks AESND uninitialized and preserves command/PB contents until the next first-time init, while incrementing the Rust software-mixer session to reject in-flight stale work.
- The Rust-only `previous_dma_callback` field was removed because C does not have that ownership/restore behavior.

The previously documented AESND silent-playback, DSP-metric, GameCube-frequency, and ASND little-endian findings are historical: current source already contains the CPU mixer, process-time accounting, target-specific default frequency, and LE 16-bit decode branches. Exact DSP scheduling and bit-exact hardware audio remain validation items because Rust intentionally uses a CPU mixer rather than libogc's DSP task.

## 2026-10-03 Independent re-review: NEEDS_CHANGES

Reviewer: reviewer_d. Implementer: worker_c (latest lifecycle changes in `91aa0fb`; related mixer/parity implementation in `8acd74a` and earlier commits). Scope: `aesnd`, `aesnd-sys`, and `asnd-sys`. The source comparison included `gc/aesndlib.h`, `libaesnd/aesndlib.c`, `libaesnd/dspcode/dspmixer.s`, `gc/asndlib.h`, `libasnd/asndlib.c`, and `libasnd/dsp_mixer/dsp_mixer.s`, alongside the current Rust implementations.

The AESND path's current format decoding covers all eight header formats (`crates/media/aesnd-sys/src/lib.rs:352-357,445-478`; `gc/aesndlib.h:20-27`), including unsigned 16-bit PCM centered around zero. Its source phase adds the low 16-bit fraction, preserves the accumulator, emits the previously latched sample before fetching zero or more source frames, and thus follows the checked-in DSP's fractional zero-order-hold order (`aesnd-sys/src/lib.rs:486-491,535-573`; `libaesnd/dspcode/dspmixer.s:273-409`). The output path writes and flushes the selected 96-frame buffer before `AUDIO_InitDMA` (`aesnd-sys/src/lib.rs:610-649`); C selects its buffer, calls the audio callback, starts DMA, and schedules DSP work in its own DMA/request path (`aesndlib.c:371-407`). The target frequency constants and AESND repeated-init/reset callback behavior match the C declarations and lifecycle (`aesndlib.h:10-14`; Rust `aesnd-sys/src/lib.rs:18-21` and commit `91aa0fb`; C `aesndlib.c:427-497`). Callback cadence, process-time measurement, buffering, and mixer execution remain architectural CPU-vs-DSP differences, not evidence of bit-exact parity.

ASND currently decodes all eight C formats, including both little-endian 16-bit forms (`asnd-sys/src/lib.rs:191-199,217-269`; `gc/asndlib.h:72-81`; DSP selectors `dsp_mixer.s:774-913`). Its pitch phase advances in 16.16 units at 48 kHz (`:182-185,847-879`), and the output is cache-flushed before DMA (`:895-898,901-959`). The latest `91aa0fb` lifecycle behavior preserves state on repeated init/end as C does (`asnd-sys/src/lib.rs:961-989`; `asndlib.c:348-410`), and the status precedence follows C's pause-over-inactive assignment order (`asndlib.c:639-651`; Rust `asnd-sys/src/lib.rs:1202-1227`). The Rust CPU mixer calls global/voice callbacks on its DMA and buffer-boundary path (`:835-845,925-959`); C schedules callbacks through DSP request lookahead (`asndlib.c:129-250,257-346`). These callback timing and per-voice DSP scheduling differences remain implementation-architecture limitations. The Rust mixer sums in `i32` and clamps after mixing voices (`:813-893`); C saturates the DSP accumulation after each voice (`dsp_mixer.s:610-659`), so clipping with multiple opposing voices is also not bit-exact and remains an architectural output difference.

### Required change

ASND channel volume scaling still differs from C. Rust computes `(sample * volume) / 255` (`asnd-sys/src/lib.rs:869-872`). C masks channel volume to 0..255 (`libasnd/asndlib.c:428-429`) and the DSP multiplies the sample by that value and arithmetic-shifts the product right by 8 (`libasnd/dsp_mixer/dsp_mixer.s:917-931`), which is fixed-point division by 256 with signed arithmetic-shift rounding. For example, volume 255 leaves a Rust sample unchanged but scales the C sample by 255/256; for negative products, Rust's signed division truncates toward zero while the DSP arithmetic shift rounds toward negative infinity. The existing mixer tests use volume 255 or check nonzero output/callback progress without asserting this amplitude and rounding behavior (`asnd-sys/src/lib.rs:1767-1799,1811-1845`). This is an observable sample mismatch and is not documented as an intentional C bug fix. Keep the audio crates open for implementation and independent re-review.

### Other parity notes

- C `ASND_GetDSP_PercentUse` divides the stored raw `time_of_process` value by 21333 (`libasnd/asndlib.c:785-788`), while `ASND_GetDSP_ProcessTime` converts ticks to nanoseconds (`:790-799`). Rust stores converted nanoseconds for the time getter and calculates CPU-mixer percentage from elapsed microseconds over its output window (`asnd-sys/src/lib.rs:946-956`). This is not a like-for-like DSP metric; the Rust value measures CPU work and the C percent formula appears inconsistent with its own documented nanosecond unit. Keep this as an explicit metric difference, not a claim of identical results.
- Cache flush and buffer order were inspected statically. AESND flushes its output before DMA (`aesnd-sys/src/lib.rs:647-649`); ASND flushes its output before starting DMA (`asnd-sys/src/lib.rs:895-898,958-959`), and C flushes source buffers during voice setup (`asndlib.c:423`). No cache-coherency behavior was verified on hardware.
- Existing regression coverage includes format and fractional stepping cases, but it does not exercise the `/256` volume scaling result identified above. The ASND test target compilation failed on existing test callback signatures that are `unsafe fn` rather than the declared C ABI; the library itself checked before that test compilation error. Tests did not run. No hardware verification was performed.

Validation: `cargo check -p aesnd-sys -p aesnd -p asnd-sys --tests --target powerpc-unknown-linux-gnu` was attempted. It failed compiling ASND test callbacks due to callback ABI/type mismatches (Rust `unsafe fn` supplied where the callback type requires `unsafe extern "C" fn`). AESND and the ASND/AESND library code were reached by the target check, but this command did not complete successfully. No tests ran. Result: `NEEDS_CHANGES`; reviewer reviewer_d; implementer worker_c; implementation commits include `91aa0fb` and `8acd74a`.

## BUG FIX handoff: ASND volume scaling

Implementer: `worker_c`. Independent re-review is needed; the review thread handles reviewer assignment and review. Scope: `crates/media/asnd-sys/src/lib.rs` volume scaling and focused pure regression coverage, compared with `../../C-sources/libogc/libasnd/dsp_mixer/dsp_mixer.s:917–931` and volume masking in `../../C-sources/libogc/libasnd/asndlib.c:428–429`.

ASND now multiplies each signed sample by the clamped 8-bit volume in `i32` and shifts the signed product right by 8. The product cannot overflow `i32` for an `i16` sample and volume 0–255. Rust's signed right shift preserves the sign, matching the DSP arithmetic shift for negative products. Regression cases cover positive and negative samples at volumes 0, 1, 128, and 255, including `i16::MIN` and `i16::MAX` at 255.

`cargo check -p asnd-sys --target powerpc-unknown-linux-gnu --lib` passed. `cargo check -p asnd-sys --target powerpc-unknown-linux-gnu --tests` failed on four existing test callback ABI mismatches (`unsafe fn` where `unsafe extern "C" fn` is required), including helper callbacks in the pre-existing tests. The target test code did not compile, and tests were not run. No hardware verification was performed.

## Independent re-review: ASND channel volume scaling

**Result: PASS for commit `0b8e259`; the audio crate group remains open for other recorded differences.** Reviewer: `reviewer_d`. Implementer: `worker_c`.

C masks each channel volume to 0–255 (`../../C-sources/libogc/libasnd/asndlib.c:428–429`). The DSP multiplies the signed sample by that value and arithmetic-shifts the product right by 8 (`../../C-sources/libogc/libasnd/dsp_mixer/dsp_mixer.s:917–931`). Rust's `scale_sample` now uses the same signed `i32` product and `>> 8` operation (`crates/media/asnd-sys/src/lib.rs:901`), and the product fits in `i32` for an `i16` sample and volume 0–255. Its regression cases cover positive and negative samples, multiple volumes, and both `i16` limits. The focused change matches C.

The lifecycle changes were also checked against C `ASND_Init`, `ASND_End`, and status ordering. The CPU mixer remains an architectural difference from C's DSP task, including callback timing and multi-voice clipping. The PowerPC library check passed per the handoff. The target test build failed on existing callback ABI mismatches, so tests did not compile or run. No hardware verification was performed. Keep `aesnd`, `aesnd-sys`, and `asnd-sys` open for the other recorded differences; this review accepts only the volume-scaling fix.

## Independent current-source audit of remaining audio behavior

Reviewer: `/root/reviewer_d`. The ASND signed `/256` scaling change in `0b8e259` matches C: Rust multiplies the signed sample and volume then arithmetic-shifts right eight bits (`crates/media/asnd-sys/src/lib.rs:901–905`), matching C's byte mask and DSP multiply/shift (`libasnd/asndlib.c:428–429`; `dsp_mixer.s:917–931`).

Other confirmed differences remain. Rust mixes 1024 stereo frames on the CPU inside `audio_dma_callback` and dispatches voice callbacks at mixer/buffer boundaries (`asnd-sys/src/lib.rs:802–899, 908–965`). C divides the buffer work between the DMA callback and DSP request callback (`libasnd/asndlib.c:257–389`); voice callbacks can run during lookahead before the channel's DSP pass (`:229–250, 325–345`). Rust also sums all voices in `i32` and clamps once per output frame (`asnd-sys/src/lib.rs:813–814, 868–893`), while C clamps after each voice addition (`dsp_mixer.s:610–659`). For opposing voices after saturation, the output can differ.

AESND remains a CPU mixer where C uses a DSP task and stream-buffer pipeline. C's DMA handler queues DSP processing and its request handler refills/advances stream buffers (`libaesnd/aesndlib.c:242–303, 322–407`); DSP assembly mixes in 96-sample units (`dspcode/dspmixer.s:82, 273–409`). Rust reads and mixes an entire output frame from the DMA callback (`crates/media/aesnd-sys/src/lib.rs:480–503, 519–649`). Callback timing, refill deadlines, process metrics, buffering, and exact sample parity therefore differ.

Several historical findings are stale: AESND voice silence, zero DSP metric, and GameCube default frequency have been fixed; ASND LE16 decoding, end-state clearing, and inactive/paused precedence also now match the cited C paths (`aesnd-sys/src/lib.rs:18–21, 519–629`; `asnd-sys/src/lib.rs:217–269, 990–995, 1217–1228`; C `asndlib.c:396–410, 639–651`). `cargo check -p aesnd-sys -p aesnd -p asnd-sys --target powerpc-unknown-linux-gnu` passed. With `--tests`, compilation failed on existing ASND callback ABI mismatches (`asnd-sys/src/lib.rs:1470, 1530, 1781, 1815`); tests did not run. No hardware verification was performed.

## BUG FIX handoff: ASND voice callback buffer-pass timing

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed.

C checks each active voice once per output-buffer pass and invokes its callback when no second buffer is queued, before the DSP processes that voice (`libasnd/asndlib.c:229–250, 257–346`). Rust previously waited until the current sample buffer was empty. It could also call the callback again on later frames in the same output pass if the callback did not queue data. Rust now invokes each eligible voice callback on the first mixer visit in an output pass, skips it when a second buffer is pending, and limits each voice to one callback per pass. It still promotes a queued buffer only when the current buffer is empty.

Regression coverage checks one callback claim per pass, pending-buffer suppression, and a voice callback on a long source buffer that does not end during the current mixer output. The existing target test callbacks now use the declared C ABI, so the target test code compiles. `cargo check -p asnd-sys --target powerpc-unknown-linux-gnu --lib`, `cargo check -p asnd-sys --target powerpc-unknown-linux-gnu --tests`, and scoped `git diff --check` passed. Tests did not run. The CPU mixer still runs synchronously on DMA where C schedules DSP request callbacks; exact timing and refill deadlines remain hardware-validation items. No hardware verification was performed.

## Independent re-review: ASND callback pass timing (`e2feda4`)

**Result: PASS for this bounded fix.** Reviewer: `worker_a`. Implementer: `worker_c`.

Rust now visits each active voice once per mix-buffer pass, invokes its callback before mixing that voice when no second buffer is pending, and records that callback as claimed for the pass. This matches C's per-voice callback opportunity and avoids repeated callbacks when a long current buffer spans the mixer output (`../../C-sources/libogc/libasnd/asndlib.c:229–250,257–346`; Rust `crates/media/asnd-sys/src/lib.rs:816–905`). Pending second buffers suppress the callback, and promotion remains conditional on the current buffer being empty. The added regression covers one claim per pass, pending-buffer suppression, and a callback with a source buffer longer than one mixer output. The fix does not resolve the documented CPU-mixer versus DSP pipeline and sample-parity differences.

I inspected the full fix diff and test code. No checks or tests were run during this re-review; the implementer's note records successful PowerPC library and test compilation, with tests not executed. No hardware verification was performed.

## 2026-10-05 completion pass — ASND voice passes and AESND scheduling

`asnd-sys` now processes the software mix in complete voice passes instead of interleaving voices frame by frame. This matches the C DSP request model closely enough to preserve callback-before-voice-pass ordering. Each voice addition saturates the current signed 16-bit output before the next voice is applied, matching `dsp_mixer.s`; opposing voices after clipping therefore no longer differ because of one final aggregate clamp. The recorded ASND callback/saturation blockers are closed at source level, and `asnd-sys` moves to `needs_re_review`.

`aesnd-sys` now invokes the application audio callback before voice mixing, as C does before starting the DSP request. Its mixer saturates after each voice addition rather than after all voices. Stream notification uses a two-`DSP_STREAMBUFFER_SIZE` (2304-byte) lookahead and is emitted once per source buffer, matching the amount C keeps staged in its two stream blocks instead of waiting for playback to reach the final byte. Running callbacks still precede each voice pass and stopped callbacks follow the pass.

`aesnd-sys` remains `parity_blocked` because its public PB stream cursor still models direct software playback rather than C's separately staged stream-buffer cursor, and its process-time metric measures the Rust software mixer rather than the original DSP task. Those are observable implementation differences and require a further implementation pass rather than reclassification as mere hardware validation.

## 2026-10-07 AESND staged-stream implementation pass

**BUG FIX handoff: staged two-block stream/PB cursor.** The current `aesnd-sys` software mixer now emulates the C CPU/DSP staging boundary instead of reading caller MRAM directly while mixing. Each voice owns a two-block, 2304-byte staging ring. A fresh voice's first software DSP pass produces no voice samples and stages up to two `DSP_STREAMBUFFER_SIZE` blocks, then sets `VOICE_RUNNING`, matching `__aesndhandlerequest` on RVL. `mram_curr` is now the producer/staging cursor; playback advances `buf_curr` in accelerator sample-width units. Refill of block 0 occurs on the half-buffer crossing, and refill of block 1 occurs on cursor wrap, with `stream_last` updated after the request as in C (`libaesnd/aesndlib.c:242-303`).

The previous synthetic 2304-byte lookahead notification was removed. Stream/end handling now runs at the C request boundary after each 96-sample voice pass, so short-buffer notification/stop timing follows the staged producer cursor instead of direct MRAM playback. `VOICE_STATE_RUNNING` remains emitted before each selected voice pass; the implementation no longer sets `VOICE_RUNNING` merely because that callback is emitted. A focused cursor regression covers accelerator-unit advancement and circular wrap.

This closes the specific staged-stream/PB-cursor blocker at source level without embedding or executing the original DSP binary. `aesnd-sys` remains `parity_blocked` because the process-time metric still measures the CPU software mixer rather than the C DSP task interval, and exact callback mutation semantics during `__aesndhandlerequest` still need a separate comparison. No PowerPC build or hardware test was available in this environment.

## BUG FIX handoff: AESND callback-aware request state machine

`aesnd-sys` now services the staged-stream request path through one resumable state machine instead of deciding stream/stopped callbacks separately from refill handling. `next_voice_request_action` owns end-of-MRAM handling, stream callback dispatch points, callback-sensitive stop/loop decisions, initial two-block staging, midpoint refill, wrap refill, and `stream_last` updates. Callback actions release `AESND_STATE`; after `VOICE_STATE_STREAM` returns, the same state machine re-enters and re-reads the voice before making any stop, loop, or refill decision. This matches the ordering of C's `__aesndhandlerequest` while avoiding callback re-entry deadlocks.

Each request carries the current AESND session plus the voice-slot generation. The identity is checked after every user callback. A callback that frees and reallocates the same voice slot therefore invalidates the old request rather than allowing stale refill or stop logic to mutate the replacement voice. The running callback path uses the same identity guard, so mutations of the existing voice are observed while same-slot reallocation aborts that pass. `VoiceFrameCallbacks` and `invoke_voice_frame_callbacks` were removed for stream-state handling.

Focused regressions cover both callback-sensitive behaviors: a stream callback that replaces the MRAM buffer is re-read before the `VOICE_ONCE` stop decision, and a stream callback that frees/reallocates the same slot invalidates the original request before it can stage or stop the replacement. This closes the callback-mutation-ordering blocker at source level. The remaining `aesnd-sys` blocker is process-metric equivalence: Rust still measures synchronous CPU mixer work while C reports timing associated with its DSP request/task path. No PowerPC build or hardware test was available in this environment.


## 2026-10-07 AESND staged-request/process-metric handoff

**Implementation comparison result: no remaining confirmed source-level blocker; independent re-review required.** The current implementation was compared again with the bundled `libaesnd/aesndlib.c` request path and DSP-dispatch timing. The software implementation keeps the C two-block staging model: `mram_curr` advances when source bytes are staged, `buf_curr` advances through the staged ring, the initial request fills both 1152-byte halves, midpoint crossing refills the first half, and wrap refills the second half. End-of-source handling invokes the stream callback before re-reading the voice for once/loop/refill decisions.

The callback-aware request state machine releases `AESND_STATE` while user code runs and then re-enters the same state machine, so callback mutations are observed before stop, loop, or refill decisions. Session plus voice-generation validation prevents an old software request from mutating a replacement voice after callback re-entry. This is an intentional safety extension around the software implementation; it does not change normal valid-voice request behavior.

The process metric now starts at the same semantic boundary as C: after the first selected voice's `VOICE_STATE_RUNNING` callback and immediately before the first software-DSP voice pass. Subsequent running callbacks, request callbacks, staged refills, and output production remain inside the measured interval. The stored value is converted to microseconds and usage remains `time_us * 100 / 2000`, matching `AESND_GetDSPProcessTime` and `AESND_GetDSPProcessUsage`. The implementation uses CPU time for the software DSP substitute because no original DSP binary is executed, but the observable API timing boundary and units now match the C contract.

No confirmed source-level parity blocker remains in `aesnd-sys` from this implementation comparison. Because this pass changed the process-metric timing boundary, `REVIEW-WORKFLOW.md` requires a different reviewer before the crate can move from `needs_re_review` to `reviewed`. Runtime cycle counts and hardware scheduling are outside the static parity ledger. No PowerPC compiler or hardware test was available in this environment.

## 2026-10-07 high-level `aesnd` comparison and re-review handoff

**Comparison result: no remaining confirmed high-level blocker; independent group re-review required.** The high-level crate was rechecked after the low-level staged-stream and callback changes. `VoiceHandle` delegates lifecycle, stop, mute, loop, stream, format, frequency, volume, delay, buffer, play, and callback operations directly to `aesnd-sys`. The process-time and usage APIs are direct wrappers over the corrected low-level metrics.

`OwnedAudioBuffer` remains valid for the complete period in which AESND may stage from its MRAM source. `OwnedPlayback::replace_buffer` calls the low-level equivalent of `AESND_SetVoiceBuffer`, which changes the producer source without clearing already-staged blocks, matching C behavior. The returned old allocation is no longer referenced by subsequent source staging once the setter returns. Drop and `into_parts` stop the voice and detach the retained MRAM source before releasing the allocation; generation validation in `aesnd-sys` prevents stale callback/request work from applying to a freed and reallocated slot.

No old direct-MRAM workaround remains in the high-level crate, and no additional callback-cadence, loop/stop, ownership, or process-metric mismatch was found. `aesnd` therefore moves from `parity_blocked` to `needs_re_review`: it shares the recently modified AESND implementation scope and must be accepted by a different reviewer before parity is declared. No PowerPC compiler or hardware test was available in this environment.

## Independent re-review — staged AESND implementation (2026-10-07)

**Result: NEEDS_CHANGES.** Reviewer: `/root` in a new review thread. The current source does not compile: `cargo check -p aesnd-sys ... --target powerpc-unknown-linux-gnu --tests` reports E0502 at `crates/media/aesnd-sys/src/lib.rs:743`, where `state.voicepb` is mutably borrowed at the same time that `state.stream_buffer` is immutably borrowed. No AESND tests ran. Fix the borrow split and rerun the target check before the source-parity re-review can continue. The result applies to both `aesnd-sys` and `aesnd`, which shares this implementation. No hardware verification was performed.

## 2026-10-07 implementation handoff — disjoint field borrow

Implementer: `/root`. `mix_voice_frame` now destructures the locked `AesndState` to borrow `voicepb` mutably and `stream_buffer` immutably as separate fields. The mixer still updates the selected voice cursor while reading its staged sample bytes; it does not copy the full stream buffer or weaken synchronization.

Validation: `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn' cargo check -p aesnd-sys -p aesnd --target powerpc-unknown-linux-gnu --tests` passed and compiled target test code. Tests were not run. The previous E0502 blocker is fixed; independent source review of this fix remains required before either crate moves to `reviewed`. No hardware verification was performed.

## Independent current-source review (2026-10-15)

Reviewer: `/root/release_parity_review`. Result: **NEEDS_CHANGES** for `aesnd` and `aesnd-sys`. The current implementation uses `native_dsp.rs` and the embedded DSP image from `e97e001`. Thus, the old software-mixer and disjoint-borrow handoff is historical. The current PowerPC library and test code compiles.

Confirmed mismatch: C enters its end-of-source branch before the stream callback, then tests `VOICE_ONCE` and `VOICE_LOOP` after that callback without testing source exhaustion again (`../../C-sources/libogc/libaesnd/aesndlib.c:264-276`). Rust resumes by testing `mram_curr >= mram_end` again (`crates/media/aesnd-sys/src/lib.rs:583-613`). A stream callback that replaces the buffer while retaining `VOICE_ONCE` therefore continues in Rust, but C clears `buf_start` and stops. A callback that replaces the buffer while retaining `VOICE_LOOP` can also bypass C's reset of `mram_curr`. The test `stream_callback_mutation_is_re_read_before_stop_decision` at `:1335` requires the incorrect once behavior. Preserve request identity checks, but retain the entered end-of-source branch across callback return and apply C's post-callback flag decisions. Update regression expectations from the C contract.

The high-level crate delegates this behavior to the sys crate. The group remains blocked. No approval of the complete native DSP state machine or of physical playback is given by this bounded review. `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn' cargo check -p aesnd-sys -p aesnd -p ogc-usbstorage-sys --target powerpc-unknown-linux-gnu --tests` passed with existing cfg warnings. Target tests did not run. No hardware was used.

## BUG FIX handoff — entered end-of-source branch (2026-10-15)

Implementer: `/root/release_parity_review`. Required independent reviewer: `release-quality/root`. Fix commit: `b13dd76`; fix claim: `56b05dc`. The implementer was also the auditor of the confirmed finding and does not approve this fix.

The resumable request now records that it entered the end-of-source branch before dispatching `VOICE_STATE_STREAM`. After callback return it retains that branch, reads the updated flags, and applies ONCE stop or LOOP cursor reset without testing source exhaustion again. This follows `../../C-sources/libogc/libaesnd/aesndlib.c:264-276`. Session and voice-generation checks remain in place before resumed work, so callback slot replacement still invalidates the old request.

The former regression that required continuation with ONCE now requires stop and a cleared staged-buffer address after buffer replacement. New target regression cases require LOOP to reset the callback-modified source cursor and require a callback that clears ONCE to continue. The existing same-slot reallocation regression remains. These tests call the production request state machine; no host transport or mixer replacement was added.

`RUSTFLAGS='--check-cfg=cfg(gamecube) -D warnings' cargo check -p aesnd-sys -p aesnd --target powerpc-unknown-linux-gnu --tests` passed. Target tests did not run. Formatting and scoped whitespace checks passed. `cargo xtask audit-source` passed with existing keyboard audit debt. `cargo xtask audit-stubs` stopped on stale AESND evidence (`get_dsp_process_time` moved from line 686 to 688); the release coordinator owns the pending evidence and inventory refresh. No evidence or generated report was changed in this fix. No hardware was used.

Both AESND rows are handed to `release-quality/root` as `needs_re_review`. Acceptance of this bounded fix does not establish the complete native DSP or hardware playback scope.

## Independent re-review — bounded DSP fixes in `8824d70` (2026-10-15)

Reviewer: `/root/review_lwp_parity`. Implementer: `/root`. Result: **PASS for
the three bounded source contracts only**. I inspected the complete commit diff
for `aesnd-sys`, the target selection, the new ARAM regression, and the updated
`VOICE_ONCE` expectations. This does not clear the other AESND blockers.

1. **GameCube ARQ completion marks the canonical PB.** C's
   `__aesndarqcallback` uses `req->owner` to set `VOICE_RUNNING` on
   `__aesndvoicepb[owner]` (`../../C-sources/libogc/libaesnd/aesndlib.c:172-175`).
   Rust's `aram::staged` validates the saved session and generation, then calls
   `mark_aram_voice_running` with the owner index
   (`crates/media/aesnd-sys/src/native_dsp.rs:403-418`); the helper updates
   `state.voicepb[index]`, not the transient `state.command.0`
   (`crates/media/aesnd-sys/src/lib.rs:422-424`). The `gamecube`-gated test
   distinguishes those two PBs (`lib.rs:1347-1368`). The identity guard is a
   Rust lifetime safety addition and does not change the valid completion path.

2. **`voicesstopped` changes at the DMA callback boundary.** C reads its prior
   value to choose mute/audio output, queues that output, then changes the
   flag after it selects the next voice
   (`aesndlib.c:371-407`). Rust follows the same order in
   `native_dsp.rs:197-243`: it selects the DMA buffer from the prior state,
   invokes the audio callback, starts DMA, returns for pause/not-ready, then
   sets stopped when no voice is selected or clears it before dispatching a
   voice. The fix removes eager recomputation from voice allocation/free/stop,
   play, and request completion in `lib.rs`; the only live assignments are now
   in the DMA callback. This preserves C's one-callback boundary between voice
   state changes and the mute/audio buffer decision.

3. **`VOICE_ONCE` buffer clearing follows the target-specific C branch.** C
   clears `pb->buf_start` only in the `HW_RVL` request handler
   (`aesndlib.c:261-273`); the GameCube `HW_DOL` handler stops without clearing
   it (`aesndlib.c:197-209`). Rust uses `#[cfg(not(gamecube))]` for the clear
   and always sets STOPPED (`crates/media/aesnd-sys/src/lib.rs:600-610`). The
   target profile selects `cfg(gamecube)` for GameCube and leaves it unset for
   Wii (`.cargo/config.toml:28-34`, `targets/powerpc-nintendo-gamecube.json`).
   The updated regression checks the retained GameCube `buf_start` and the
   cleared RVL value (`lib.rs:1370-1411`).

Validation: `RUSTFLAGS='--check-cfg=cfg(gamecube) -D warnings' cargo check -p
aesnd-sys -p aesnd --target powerpc-unknown-linux-gnu --tests` passed for the
RVL branch. The same test sources compiled with `cfg(gamecube)` enabled on the
PowerPC Linux target. The production GameCube custom-target check passed with
nightly `-Z build-std=core,alloc -Z json-target-spec`, and `cargo xtask
build-dols` passed. A PowerPC test-binary link attempt failed because the
configured host linker rejects the cross-link flags; tests did not run. No
hardware behavior is claimed. `cargo xtask audit-source` passed with existing
keyboard audit debt. `cargo xtask audit-stubs` remains blocked by stale AESND
evidence for `aesnd-sys::get_dsp_process_time` at line 688; this is an evidence
inventory issue, not a new source mismatch in this fix.

The AESND crates remain `parity_blocked`. The separate callback return-type
parity, high-level `VoiceHandle` ownership safety, and complete native PowerPC
DSP behavior remain unresolved. This bounded pass does not validate callback
timing, DSP processing, DMA/ARQ behavior, cache coherency, audible output, or
Wii/GameCube hardware operation.

## Evidence refresh — 2026-10-15

The implementation evidence line references for AESND and USB storage now
match the current sources. `cargo xtask inventory` regenerated the inventory
and status files. `cargo xtask audit-source` passes with the existing keyboard
queue audit debt. `cargo xtask audit-stubs` now reaches the completeness check
but fails on 2,354 API entries that lack accepted implementation evidence.
This broad audit result does not change the bounded AESND re-review result.

## Independent review — callback branch fix (2026-10-15)

Reviewer: `release-quality/root`. Implementer: `/root/release_parity_review`. Result: **PASS for the bounded fix in `b13dd76`**. C enters the exhausted-source branch once, calls the stream callback, then reads ONCE and LOOP flags (`../../C-sources/libogc/libaesnd/aesndlib.c:264-276`). The Rust request progress now retains that branch across callback dispatch. It reads the updated flags, clears `buf_start` and sets STOPPED for ONCE, or resets the cursor to the updated source start for LOOP. The request still checks session and generation before resumed work, so freeing and reusing a slot cannot apply old work to the new voice.

I inspected the complete fix diff, the callback dispatch loop, and the regressions against these C decisions. The tests use independent cursor and flag expectations and the production state machine. PowerPC library and test compilation passed with warnings denied. The tests did not run. No playback or physical DSP validation was performed. Both rows now use `review_incomplete`: this bounded acceptance closes the confirmed callback blocker but does not complete the native DSP state-machine and full API comparison.

## Runnable DSP stream coverage (2026-10-15)

The native AESND DOL now runs four stream cases through the public API and real
DSP callbacks: replacement with ONCE, replacement with LOOP, restart as LOOP
from STREAM, and same-slot free/reallocate from STREAM. Each case must first
produce the initial PCM. Continuing cases must then produce the independent
negative PCM pattern. ONCE must stop dispatch and produce silence without
playing replacement PCM. Explicit stop and reset are checked separately.

Input buffers have permanent aligned storage. Callbacks do not edit private PB
fields or invoke the request engine directly. A failed initial DSP/callback
check leaves dependent behavior UNTESTED. The tests check observable loop
playback; they do not replace the target unit test that changes an internal
cursor before the post-callback LOOP decision. Build and source checks do not
establish a hardware pass. The hardware register retains `not-tested` until
logs from the new DOL are accepted. Full source parity review remains open.

## Independent full-source review — 2026-10-15

Reviewer: `/root/review_lwp_parity`. Result: **NEEDS CHANGES**. The current
native DSP port has confirmed differences beyond the stream callback fix:

- On GameCube, an ARQ completion can set `VOICE_RUNNING` on the transient
  command PB after that PB was copied to the canonical PB. The update is then
  lost when the command PB is cleared (`crates/media/aesnd-sys/src/native_dsp.rs`
  `aram::staged` and `task_request`; C `C-sources/libogc/libaesnd/aesndlib.c`
  `__aesndarqcallback`). Update the canonical PB as C does.
- Rust changes the `voicesstopped` flag before the next DMA selection. C changes
  it during voice scanning, after selecting the current DMA buffer. This can
  change one 2 ms output block. Match C's selection order.
- Rust clears `buf_start` for `VOICE_ONCE` on both targets. C clears it only in
  the `HW_RVL` path. Add target-specific behavior or record a deliberate
  GameCube difference with its restart effect.
- Switching between legacy and with-argument callback setters loses the prior
  callback return value. The C inline wrappers return the previous callback
  pointer across both forms (`gc/aesndlib.h`). Rust stores the forms separately.
  Keep this as an explicit ABI/type deviation if it cannot be represented
  safely.
- `VoiceHandle::new` is a safe public constructor for an owning raw slot. Safe
  code can create duplicate owners and free a live or reused voice through
  either handle. Make ownership creation unsafe or change the handle model.

The reviewer also found stale present-tense text describing a CPU mixer. The
current implementation uses the native DSP request path; mark the old mixer
description as historical. Accepted resource differences include rejecting
already-initialized GameCube ARAM and retaining the 32-block reservation after
reset. The reviewer made no changes and ran no tests or builds. No hardware
result was claimed. The bounded Wii stream callback fix passes its ordinary
valid-voice comparison, but both AESND rows remain blocked by the findings
above.

## Bounded parity fixes — 2026-10-15

**BUG FIX:** The GameCube ARQ completion callback now sets `VOICE_RUNNING` on
the canonical voice PB. It no longer writes the transient command PB
(`crates/media/aesnd-sys/src/native_dsp.rs`, `aram::staged`; C
`libaesnd/aesndlib.c`, `__aesndarqcallback`). A PowerPC test checks the helper
while a request is active. The GameCube test source could not be linked with
the host linker, so this test did not run.

**BUG FIX:** `voicesstopped` now changes only in the DMA callback, after it
selects and starts the current DMA buffer. Voice allocation, stop, and request
handling no longer change it early, matching C's callback order
(`native_dsp.rs::audio_dma_callback`; C `__audio_dma_callback`).

**BUG FIX:** `VOICE_ONCE` clears `buf_start` only on RVL. The GameCube path
keeps the staged address, matching the C `HW_DOL` branch
(`lib.rs::next_voice_request_action`; C `__aesndhandlerequest`).

PowerPC test compilation for the default RVL profile passed with warnings
denied. The DOL probe build passed. The GameCube test target could not link
because the configured host linker does not support the PowerPC link flags.
Tests did not run and no Wii or GameCube hardware result is claimed. These
bounded fixes await independent source re-review by `/root/review_lwp_parity`.
Callback return-type parity, `VoiceHandle` ownership safety, and full native
DSP parity remain open, so both crates stay `parity_blocked`.

## Wii probe evidence — 2026-10-15

Run `1792110541-513590000` on Wii `10.0.0.242` returned a complete WIR2
report: **46 passed, 7 failed**. The DOL build ID was
`12a70b798c9025e95c0928b32afeabf4f95c5d92ed36b28c8188fd119ccfaf79`.
The local evidence is in `target/wii-runs/1792110541-513590000-10.0.0.242/`.

- Two native passes found zero output buffers with the required left 2048 and
  right 1024 samples. Each pass required at least ten matching buffers.
- Four stream cases found zero buffers with the initial source samples.
  Replacement, loop, and explicit-stop sample outcomes remain untested because
  the initial-output prerequisite failed. Callback and DMA checks do not prove
  correct PCM output.
- The screenshot-transfer check failed with the first error `-110` during
  startup. Nine later page screenshots and the final screenshot arrived with
  readable text. Thus this run does not prove capture of every page.

The earlier run `1792110049-242303000` returned a final report but its XFB
snapshots contained corrupt green pixels. The probe now copies uncached XFBs
with volatile aligned word accesses. The later run above confirms readable
snapshots. This fixes probe evidence capture; it does not fix AESND output.
A separate run, `1792110368-028536000`, stopped at configuration startup and
returned no report. It is not a test pass.

The System Menu return is still open. A final report and an
`exit_path=system_menu return=requested` record do not prove that the menu
opened. Probe diagnostics now show network cleanup and reset stages on screen.
AESND parity and real-hardware acceptance remain blocked.

Run `1792113309-473420000` used DOL build
`b0ffd2fecec448cdb566cd893da7376631ee20c6a29ac87f9726c8bf66547239` and
returned **46 passed, 7 failed**. The two native DSP sample checks found zero
matching buffers, and four stream cases found zero initial-source buffers.
The page screenshot check failed: the first two capture attempts returned
`-110`, while nine later captures reached the host. The final WIR2 report did
arrive. Artifacts are in `target/wii-runs/1792113309-473420000-10.0.0.242/`.
This run is incomplete for page evidence and does not establish System Menu
return; the upload endpoint did not return. Its screen showed DSP shutdown
mailbox `0x8071` with the busy bit set while VI frames continued.

The agent probe now retries IOS `ETIMEDOUT` during report socket setup up to
three times. It keeps other errors and the final timeout visible. Standalone
tests cover retry success, exhaustion, and non-timeout errors. The rebuilt DOL
is `cd00daa6a6b04db63a36cc570d0b44701fa6353a0cb2e1b81392a1d31a15d77d`; a new
real-Wii run is still needed to confirm that every page arrives. The same build
also keeps the recovery watchdog armed while a requested system reset runs, so
a DSP shutdown wait does not cancel its fallback path.

## Framebuffer capture and DSP shutdown follow-up — 2026-10-15

Final and page reports now select the XFB reported as VI's current framebuffer.
The probe checks that this address is one of its two allocated XFBs and matches
the console target. A mismatch is retained in the report transcript with the
current, next, console, and allocated addresses, and the capture fails. The
seconds indicator now reads `IDLE s`; numbered `STEP` records remain in the
progress log. A host regression check covers both valid XFBs and mismatched
VI or console targets.

The earlier review compared the Rust DSP shutdown order with C `__dsp_shutdown`: stop DMA, wait
for both DMA status bits, clear mailboxes, wait while the DSP mailbox busy bit
is set, wait 44 timebase ticks, then assert reset. The observed mailbox value
`0x8071` is still busy at that wait. It does not identify which earlier AESND
operation left the DSP in that state, so no AESND shutdown behavior or wait
condition was changed without hardware evidence. The Wii cause, visible
failure-page retention during a stuck reset, and successful HBC return remain
open pending a new Wii run. This comparison missed the low-half mailbox read;
the 2026-10-16 finding below supersedes the claim that the wait matches C.
No device reset or firmware change was performed during that review.

Verification: `cargo test -p xtask probe_` passed (22 tests), and
`cargo xtask build-agent-probes` passed. `cargo xtask audit-source` passed
with existing keyboard audit debt. `cargo xtask audit-stubs` reported 2,355
existing unverified or incomplete API entries, including AESND APIs.

The Dolphin validator now copies the configured Wii NAND into each isolated
probe profile and removes that copy when the run ends. It rejects a source
without the installed System Menu. The default source is Dolphin's standard
macOS NAND path; `--nand-dir` and `DOLPHIN_WII_NAND` can select another source.

The AESND run used Dolphin 2603-415 and this copied NAND. It timed out after
90 seconds with zero frames, pages, or probe transport bytes. The boot
breadcrumb reached stage 3, but Dolphin did not produce guest output. Its GDB
stub returned a stop packet where the runner expected a PC register reply, so
the PC remains unknown. The report is in
`/private/tmp/libogc-dolphin-nand-final/suite.json`. The run did not reach the
DSP shutdown or System Menu launch path, so it gives no evidence for either.
The temporary NAND copy was removed after the run. The report-smoke script
targets a real Wii and was not run. Hardware verification is not claimed.

## DSP shutdown mailbox acknowledgement — 2026-10-16

Run `1792118749-209307000` used DOL SHA-256
`9715d7a9a5b004790bf6a23d5fb1f5ed513946e63128b589462ea7adfcfce479`.
It returned 47 passes and 6 failures, 11 readable checkpoint screenshots,
and a final framebuffer image. The six DSP sample checks remain failed.
Artifacts are in `target/wii-runs/1792118749-209307000-10.0.0.242/`.
The user's photo shows completed network cleanup and entry into System Menu
reset. The user subsequently recovered the Wii to HBC manually. This is not
evidence that the probe's reset completed.

The earlier shutdown source review missed a required side effect. C
`__dsp_shutdown` reads `_dspReg[2]` and `_dspReg[3]` on every mailbox poll.
Rust read only register 2. Reading register 3 acknowledges the message and
clears the busy bit; a high-only poll can leave the observed `0x8071` busy
forever. Dolphin's `SDSP::ReadMailboxLow` also explicitly clears that bit:
https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/DSP/DSPHWInterface.cpp

The production shutdown loop now reads both halves through the same mailbox
reader as `read_mail_from`. It still waits for the mailbox to become empty
before asserting reset. No timeout bypass or title-launch shortcut was added.
Two host regression tests check read order, full message value, and a pending
message that remains busy until its low half is read. Both pass through
`cargo test -p xtask dsp_mailbox_tests`.

Both AESND probes now record the first actual nonzero stereo frame, total
nonzero frames, and silent callback buffers. This uses one shared observer.
The sample expectations and acceptance checks are unchanged. Its host test
passes through `cargo test -p xtask evidence_tests`.

`cargo xtask build-agent-probes` passed. The prepared agent DOL SHA-256 is
`3339a669675bc03d34a355e0bac6e3ee7726c2f21f3f2b84decd092ad7487f6b`.
It has not been uploaded. `cargo xtask audit-source` passed with the existing
keyboard audit debt. The implementation evidence line and generated inventory
were refreshed. `cargo xtask audit-stubs` still reports 2,355 disallowed API
entries; these findings were not suppressed.

The Dolphin runner built and launched the shutdown fix with the copied Wii
NAND. It timed out after 90 seconds, at boot stage 3, with no frames or probe
output. Its PC was in `compiler_builtins::int::specialized_div_rem::u64_div_rem`.
Report: `/tmp/libogc-dolphin-mailbox-fix/suite.json`. This run predates the PCM
observer addition. It did not reach AESND checks or shutdown and is not a pass.
Real-Wii reset completion and all six sample failures remain open.

## Executed DSP sample comparison — 2026-10-16

The timeout stack in `/tmp/libogc-dolphin-stack-trace/suite.json` showed
`aesnd_stream_probe::run` calling the Gecko report transport. Boot stage 3
was only the last startup marker; it did not mean execution remained there.
The Gecko double-completion defect is recorded in `libusbgecko.md`.

Once that transport worked, the screenshot reader rejected valid white-pixel
data beginning `EB80` as a GDB error. The runner now recognizes only the
three-character `Ehh` error form. A regression distinguishes valid pixel data
from actual error packets. Timeout reports now retain stack frames.

Actual native PCM was `04000800`, not silence. The DSP mixer assembly writes
the right channel before the left channel in `dspmixer_loop_end`. With input
+4096 and left/right volumes 0x80/0x40, the native frame is therefore
`04 00 08 00`. The probes had checked left-first bytes. Both now share fixed
right-first positive and negative expected frames. They still require every
sample in each matched buffer to equal the expected bytes. A regression
rejects reversed channels, silence, a wrong later sample, and partial frames.
No production mixer arithmetic was changed.

`cargo xtask dolphin-validate --no-build --probe aesnd_native_probe
--artifact-dir /tmp/libogc-dolphin-pcm-corrected` passed: **72 checks, zero
failures, 10 valid page screenshots**. DOL SHA-256:
`c2214bcf9c688ad0532649f4d926a776a43bd4885a0131749f6899b9e869d4cf`.
The report and pages are in that artifact directory. This closes the six
sample failures in Dolphin. Hardware sample and System Menu return acceptance
still require a completed Wii run.

The corrected agent DOL SHA-256 is
`adeabca4319643ff38a1caf2b095ccc9fd5bfc0a68c5d88b80331e54efd9154b`.
The required Terminal script uploaded it in run `1792125100-658450000`.
The upload completed, but the runner detected the HBC port returning after
974 ms and stopped without accepting a report connection. No screenshots
were received. Artifacts:
`target/wii-runs/1792125100-658450000-10.0.0.242/result.json`.
This is an unsuccessful hardware run and does not prove the DOL executed or
that its reset path completed.

The user first observed HBC, then a green screen and later a red DSI exception
screen. The photo shows PC `0x81341700`, DAR zero, and stack addresses in the
`0x8133xxxx`/`0x8134xxxx` range. These addresses are outside this probe's loaded
text (`0x80004000` through `0x8004d1bc`) and overlap the local C HBC link range.
The local HBC ELF's code bytes at that PC do not match the photo; an exact HBC
function cannot be assigned from those files. The exception cannot be counted
as a probe result or successful reset. The user pressed the screen's reload
button and confirmed a return to HBC.

One retry was started through the same script, run `1792125168-945272000`.
It timed out after 60 seconds at readiness with connection refused. It sent no
DOL. No further upload is in progress. The first run's early HBC-port result
did not establish a stable return and stopped its report listener too early
to collect later evidence. The Wiiload report-listener early-stop condition
requires a separate correction before another hardware run.

All 59 xtask host tests passed before that upload. Source audits and inventory
are refreshed; the completeness audit retains its 2,355 existing open entries.
