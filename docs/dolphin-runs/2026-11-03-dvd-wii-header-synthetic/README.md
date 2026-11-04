# Synthetic Wii-format DVD and DI check

Dolphin 2609a ran `dvd_probe` in Wii mode with a sparse, unencrypted synthetic
Wii image. Its header has `TST1TS` as the disc ID, the Wii magic value at
offset `0x18`, and a minimal partition table. `file` recognized the image as a
Wii disc. The image is a test fixture, not a valid game disc: it has no valid
signed partition ticket, TMD, or filesystem. The header and partition offsets
follow the [Wii disc format notes](https://wiibrew.org/wiki/Wii_Disc).

The DVD interface reported a disc and passed its inquiry, disc ID, synchronous
sector-zero read, asynchronous sector-zero read, and synchronous read at byte
offset `0x2000`. `dvd::mount()` returned `-1`. The direct DI interface opened,
but still reported no disc; its disc-ID read returned `-5`, and its Rust
sector read returned `1`. The C sector read matched the Rust result, which
confirms parity for that failure only. The [probe log](probe.log) reports 23
passes and 4 failures.

The sparse image was 4,699,979,776 bytes. Its SHA-256 was
`c934b392d3348e62fce22caa8b4826196b89a15a6c9ca709be9bfc4a1c97533f`. Its
known data at offset `0x2000` used byte pattern `(11 + 37 * index) mod 256`.
The probe DOL SHA-256 was
`a17a944a3edd8974a1b79ff7ffdeadd83554f5c5d836fd343c5e1ef02479ece8`.

The current probe source also checks an asynchronous read of the full sector at
offset `0x2000`. It checks the completion callback, transferred byte count, and
all 2,048 bytes against the pattern above. This source change was not part of
the saved run above. It needs a new Dolphin run before it counts as runtime
evidence.

The source now also reads two sectors from `0x2000` in one asynchronous call.
It checks the full payload sector, the following sparse zero-filled sector,
and the reported 4,096-byte transfer length. This is also pending a Dolphin
run.
The current Wii `dvd_probe` DOL built by `cargo xtask build-dolphin-probes`
has SHA-256
`c95d7207a63245bddf7f27d4abbbebe966344781b75af2f0434e663773931fb2`.

The command used the existing runner:

```sh
cargo xtask dolphin-validate \
  --probe dvd_probe \
  --disc-image /path/to/synthetic-wii-test.iso \
  --dolphin /path/to/Dolphin \
  --artifact-dir target/dolphin-wii-synthetic-media
```

This result adds Wii-format DVD read evidence. It does not prove successful
DVD mounting, a valid partition, direct DI reads, partition operations, or
physical drive behavior. The DI result may need a valid partition image or a
different Dolphin setup; this run does not identify which condition caused it.
