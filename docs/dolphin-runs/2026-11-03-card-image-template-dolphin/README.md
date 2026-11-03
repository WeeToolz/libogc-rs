# CARD image attachment diagnostic

## Result

`storage_probe` passed in Dolphin 2609a with 111 checks and no failures. The
runner copied the card image into its private Dolphin profile. The source
image and the copy had the same SHA-256 hash after the run:
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.

The CARD API still reported `CARD_ERROR_NOCARD` when it tried to mount slot A.
The probe skipped card save operations. This run does not verify card mount,
geometry, save persistence, detach handling, or physical Wii behavior. The
111 passing checks cover the other storage probe checks and no-card errors.

The DOL SHA-256 hash was
`c262af6b0dd02d9b76a68f858addc7cb73177735c03be1493bfecfabbbd50b55`.

## Reproduction

Set `LIBOGC_DOLPHIN_CARD_IMAGE` to a raw GameCube memory-card image. The runner
copies it to its private profile. It does not write to the source image.

```sh
LIBOGC_DOLPHIN_CARD_IMAGE=<path-to-raw-card-image> \
cargo xtask dolphin-validate --no-build --probe storage_probe \
  --artifact-dir target/dolphin-card-image-template
```

The source file must have a size of at least six 8192-byte blocks, and its size
must be a multiple of 8192 bytes. The successful no-card assertions do not
count as successful card-operation evidence.
