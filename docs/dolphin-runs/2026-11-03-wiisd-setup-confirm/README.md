# Wii SD virtual setup confirmation

The existing `cargo xtask dolphin-validate` runner ran `wiisd_probe` on
Dolphin 2609a with a private profile and virtual Wii SD enabled. It passed 15
checks with no failures. The DOL SHA-256 was
`faf5e9e542cf5408623308903524bf5ceeb3807fd73684f77c3233bf82102f25`.

The [probe log](probe.log) records card insertion, sector-zero read, one-sector
and nine-sector write/read/restore cycles, shutdown, and restart. The runner
reported valid page captures. The [suite report](suite.json) records the
launch arguments and artifact paths. The earlier [FAT SD log](../2026-11-03-storage-unrestricted/fat_sd_probe/probe.log)
records a successful FAT mount and file operations on the virtual card.

The HBC test enables Dolphin's Wii SD folder sync; this probe runner enables
the virtual Wii SD card and writes in a private profile. These results verify
the probe runner's virtual SD path. They do not verify a USB mass-storage
device, an EXI SD Gecko adapter, or a disc image.

```sh
cargo xtask dolphin-validate --no-build --probe wiisd_probe \
  --dolphin ${DOLPHIN_BIN} \
  --artifact-dir ${WIIDEV_ROOT}/tmp/wiisd-confirm-2026-11-03
```
