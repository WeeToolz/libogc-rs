# exFAT through Newlib in Dolphin

The `ogc-blockfs` coordinator selected exFAT, registered `sd:`, and served
Newlib file calls on sector-zero and MBR images in Dolphin 2609a.

| Image | Result | Checks | Image hash unchanged |
| --- | --- | ---: | --- |
| 64 MiB, exFAT at sector zero | Pass | 40 / 40 | Yes |
| 64 MiB, exFAT in primary MBR partition | Pass | 40 / 40 | Yes |

The probe opens and reads the fixture file, rejects a write-only open, verifies
that an open handle blocks unmount, closes the handle, and unmounts. The full
runner JSON, logs, and screen captures are stored in each image directory.

This evidence covers Dolphin's virtual Wii SD. It does not prove physical SD
controller behavior or card removal on a console.
