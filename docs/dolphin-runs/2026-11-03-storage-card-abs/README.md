# Storage probe with a private memory card profile

The Dolphin runner used a private user profile for `storage_probe`. It set
`Dolphin.Core.SlotA=1` and an absolute `Dolphin.Core.MemcardAPath` under that
profile. The run did not create the raw card image. The card probe reported no
card and skipped card save and format operations. No card was formatted.

## Run details

- Dolphin: 2609a
- Probe DOL SHA-256: `a0f03acb9133b112ed0f6bbb6823cf7bd8ce19f855713b00fc119997f33486b4`
- Repository revision: `e4ea356a0579062dcbb7cc1aba218356118d4a09`
- The runner recorded a dirty worktree because the Dolphin arguments were
  under development. The probe source did not change in that worktree.
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe storage_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-storage-card-abs
  ```

## Result

The probe returned `probe_fail`: 62 checks passed and 3 failed. The failures
were `argv handoff`, `usb raw inserted`, and `usb fat mount`. The report has
9 page captures and 0 video frames. The final check did not pass.

The card section logged `card tests skipped: insert a memory card in slot A`.
This run gives no runtime evidence for card save operations. It does not prove
that Dolphin accepts the configured raw-card path for this Wii DOL.

The full report, logs, private Dolphin profile, and page captures are in this
directory. `suite.json` records the arguments and all check results.
