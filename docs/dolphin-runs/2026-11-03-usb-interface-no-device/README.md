# USB storage interface checks without a device

## Result

Dolphin 2609a passed `storage_probe` with 229 checks and no failures. The
probe found no USB storage device. Its DOL SHA-256 is
`42a14a2fc2fbb785fb53757c5a96adfa7341f974b0e72507a56a5e6aeeba1ef2`.

The probe called both public USB storage disc interfaces. For slots A and B,
one grouped assertion checked that startup succeeds, insertion reports false,
one-sector reads and writes fail, the read buffer remains unchanged, status
clear succeeds, shutdown succeeds, restart still reports no device, and final
shutdown succeeds. The log names each slot's result.

The same run passed USB initialization, no-device enumeration, null-handle
rejection, FAT USB mount's `DeviceUnavailable` result, and deinitialization.
It does not test successful USB reads, writes, cache flush, or FAT file
operations on USB media.

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified
external fixtures, then run:

```sh
cargo xtask dolphin-validate \
  --probe storage_probe \
  --dolphin <path-to-dolphin-executable> \
  --artifact-dir docs/dolphin-runs/2026-11-03-usb-interface-no-device
```

## API evidence map

| Interface entry | Assertion in `probe.log` | Result scope |
| --- | --- | --- |
| `startup` | Slot A and B callback checks | Interface startup succeeds without a mass-storage device. |
| `is_inserted` | Slot A and B callback checks | Both discovered drive slots report absent before and after restart. |
| `read_sectors` | Slot A and B callback checks | One-sector read fails and preserves destination bytes. |
| `write_sectors` | Slot A and B callback checks | One-sector write fails without a device. |
| `clear_status` | Slot A and B callback checks | Status clear returns success in Dolphin's Wii mode. |
| `shutdown` | Slot A and B callback checks | Shutdown and restart cleanup return success. |

Each row shares a grouped assertion for its slot. The run does not prove
successful behavior for any callback on attached USB media, or timing and
hotplug behavior on physical hardware.

## Evidence files

- [`storage_probe` log](storage_probe/probe.log)
- [Final summary capture](storage_probe/pages/page-019.png)
- [Suite report](suite.json)
