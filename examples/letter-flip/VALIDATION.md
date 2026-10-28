# Letter Flip verification — 2026-10-13

## Completed checks

- Shared Rust code: 36 tests passed; no failures or skips.
- Python voice generator: 5 tests passed; no failures or skips. These include actual failing processes at speech and encoding boundaries and a real silent MP3 decode check.
- Jamie (Premium) pack: all 137 MP3s passed hash, source, codec, channel, sample-rate, duration, full decode, and non-silence checks. The files total 1,998,176 bytes. The longest clip is about 18.33 seconds.
- Git ignore check: all 137 MP3s are ignored. No audio file in the generated folder is tracked. The folder-level rule also ignores generation records and temporary files.
- Wii game DOL: built with the embedded voice pack.
- Real-Wii probe DOL: built. It uses production storage and audio APIs and separate test files.
- application DOL: built after removal of the unused looping extension from its shared PCM helper. Finite playback is preserved.
- Source-structure audit: passed. No audit allowance or expected result was weakened.
- Screen layouts: rendered through the production renderer on a host raster canvas. All screens, settings rows, persistence labels, and number cards passed bounds checks. The Game settings and correction screens were inspected visually. This does not prove television or Wii framebuffer behavior.

## Checks that remain open

The real-Wii probe was not run. Real controller input, audible playback, memory-card writes and remounts, restart persistence, supported video modes, and television margins need hardware checks. A build or host test is not a hardware pass. See the README for the probe build and manual check instructions.

The repository-wide runtime-completeness audit is blocked by existing stale implementation evidence:

```text
stale implementation evidence: src/lib.rs:809 mp3player-sys::init
```

The `mp3player-sys` source and the evidence entry were already different before this work. This change does not edit or suppress that evidence. The source-structure part of the same audit passes.
