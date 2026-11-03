# CARD accessor probe update

The GameCube CARD probe ran in Dolphin 2609a with a private copy of a verified
16 MiB memory card image. The final run passed 98 checks with no failures. The
DOL SHA-256 was
`2ef5fcb2b540b43c93b56f0dc9e62c30ebe72879d92e03ff47c11010cf7be587`.

The first run used an old persistence expectation. It passed the raw sys and
wrapper round trips, then wrote `0x96, 0x69` through the wrapper before closing
and remounting the card. The persistence check still expected the earlier
`0x3c, 0xc3` data. The probe reported 97 passes and 1 failure. The check now
expects the last written bytes. The first DOL SHA-256 was
`c065aac180ba99524bd9cc5e50c55d379fffa6668b1c659ed7efff3a8ca75e0d`. The
second run passed all 98 checks. The two logs preserve both results.

## Coverage

The probe checks invalid-channel results and preserved output values through
the public CARD wrapper and `ogc-card-sys` APIs. It also checks owned async
request submission and completion, synchronous wrapper and sys read/write
round trips, save creation, status and directory data after remount, deletion
after remount, free-block recovery, and unmount and shutdown results.

The Dolphin runner attached a private copy of the verified card image. Its
SHA-256 was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`; the
source image had the same hash after the run. The probe captured seven text
pages. I inspected them. The first page shows the API checks, and the summary
page shows `PASS summary passes=98 fails=0`.

This is Dolphin evidence only. It does not verify physical GameCube card
behavior, card formatting, or physical removal handling.

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3`, `LIBOGC_MAD_REFERENCE_MP3`, and
`LIBOGC_DOLPHIN_CARD_IMAGE` to the verified external fixtures. Then run:

```sh
cargo xtask dolphin-validate \
  --probe card_probe \
  --dolphin <Dolphin executable> \
  --artifact-dir <temporary report directory>
```

The reference MP3 fixture SHA-256 matched the value required by
`docs/dependencies.md`. The run used Dolphin's private profile and copied the
card image into that profile. The runner reported seven valid page captures,
zero failed checks, and a successful cleanup.

## Evidence files

- [Final probe log](probe.log)
- [First run failure log](first-attempt-probe.log)
- [First check page](first-page.png)
- [Passing summary page](summary-page.png)
