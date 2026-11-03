# Media-aware storage probe

## Result

`storage_probe` passed in Dolphin 2609a: 97 checks passed and 0 failed. The
suite captured 10 pages and 0 video frames. The final page shows the expected
`CARD_ERROR_NOCARD` result, skipped card save and format checks, storage
shutdown, and `PASS summary passes=97 fails=0`.

The probe ran with a private Dolphin profile under this directory. It had no
USB storage device and no GameCube memory-card image. The probe checked the
USB no-device result, skipped USB reads and FAT file operations, checked the
asynchronous card-mount completion with result `-3`, and skipped card save,
format, and hotplug operations. Dolphin's virtual Wii SD path completed
startup, insertion, sector read, zero-sector write, and shutdown checks. The
probe also skipped GameCube SD Gecko operations because no EXI adapter was
configured. It skipped argv handoff checks because direct launch supplied no
arguments.

This run does not verify USB transfer behavior with a device, card saves or
format, readable-disc paths, GameCube SD Gecko I/O, physical media, or physical
Wii/GameCube behavior. It does not turn source inventory coverage into runtime
proof for every storage API.

## Reproduction

```sh
LIBOGC_MAD_PROBE_MP3=[libogc-rs fixtures]/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[libogc-rs fixtures]/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-storage-probe-media-aware
```

Fixture SHA-256 values:

- `layer3-stereo.mp3`: `81b37c353f469a792480ed477139b662a3970a4cee24337b86507aca1f2bb38a`
- `libmad-stereo-mpeg1.mp3`: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`

## Build evidence

- Dolphin: `Dolphin 2609a`
- DOL SHA-256: `a903bdc7f439564c824ae4e9a2ef976a7910389990933098f19592dbdeedf22a`
- Repository revision: `d4bd203` with the uncommitted media-aware probe change
- Run status: `passed=true`; suite `status=pass`; 97 passed; 0 failed
- Captures: 10 pages; 0 video frames
- Final page: `storage_probe/pages/page-010.png`
- Machine report: [`suite.json`](suite.json)
- Probe log: [`storage_probe/probe.log`](storage_probe/probe.log)

An earlier run before the no-media handling changes remains in
[`2026-11-03-storage-probe-qt-sandbox-bypass`](../2026-11-03-storage-probe-qt-sandbox-bypass/).
