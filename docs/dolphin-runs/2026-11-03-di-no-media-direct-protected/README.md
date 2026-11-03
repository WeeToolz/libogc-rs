# Direct DI no-disc checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `dvd_probe`
- Repository revision: `a20245d4da253318d5ed6a70149800fbb4e6a725`
- The suite report marks the worktree dirty because this run contains the DI
  probe changes recorded with the run.
- Probe DOL SHA-256: `83f20b60619ab04d9da69d5a16c2ae116a81bdad7af7754abe521c777f832ff3`
- Result: pass, 110 checks, 0 failures
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[external tools]/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[external tools]/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe dvd_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-di-no-media-direct-protected
  ```

The probe calls the Rust DI API and C `DI_*` entry points directly. It checks
no-disc mount state and callback delivery, drive identification and support,
disc-ID output preservation, read argument errors, protected-read return
values, drive controls, and close, reopen, and final cleanup.

In this Dolphin run, DVD key, physical-data, and report-key calls returned
`-5`; BCA calls returned `0`. The BCA result does not validate data contents
because Dolphin had no disc. The direct sector read returned `1` through the
AHBPROT path. This does not prove behavior on a physical drive or through a
different IOS privilege path. A no-disc partition-open request did not return
in an early attempt. The final probe omits both partition calls, which remain
unverified. The init callback, DVDX load, and cache controls also remain
unverified.

The suite report, logs, and page captures are in this directory. The private
Dolphin profile is ignored by Git.
