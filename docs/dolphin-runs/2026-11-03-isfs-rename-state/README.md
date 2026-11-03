# ISFS rename result and cleanup probe

Dolphin 2609a ran `runtime_probe` in its private NAND profile. The DOL
SHA-256 is
`3c6eca45100e4d2b843270eeccbd60f6ba50b8582ad3b061c9bf0c0c68ad3568`.
The source revision was `a875de8` with local probe changes. The probe reported
87 passes and 1 failure. Its full check log is in [`probe.log`](probe.log).

The sync and async rename calls returned `ISFS_EINVAL` (`-101`). Each check
confirmed that the source file remained and the destination stayed absent.
Both rename paths checked file closure. The probe removed the source and
destination paths, checked that both were absent, and removed the test
directory. These checks passed.

The full probe failed its existing `format_async` completion check. The
submission returned zero and Dolphin removed the marker file and directory,
but the callback did not complete before the probe timeout. The IOS result
remains unknown. This run does not prove successful rename, callback delivery
for async format, or physical Wii NAND behavior.

Reproduce with the current source and a configured Dolphin executable:

```sh
LIBOGC_MAD_PROBE_MP3=<path-to-probe-mp3> \
LIBOGC_MAD_REFERENCE_MP3=<path-to-reference-mp3> \
cargo xtask build-dolphin-probes

cargo xtask dolphin-validate --no-build --probe runtime_probe \
  --dolphin <path-to-dolphin-executable> \
  --artifact-dir target/dolphin-isfs-rename-state \
  --timeout-seconds 180
```
