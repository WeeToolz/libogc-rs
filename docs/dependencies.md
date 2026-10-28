# External development dependencies

The `libmad-rs` decoder is included under `vendor/libmad-rs`. It was copied from local source commit
`4d13614e86d834590bcd895740a5d85b46ac8d9d`. The decoder is a separate Cargo
package, excluded from the root workspace; `cargo xtask test-host` runs its
host tests. Its license is included alongside the source. Do not substitute a
different decoder or fixture without verifying its behavior and license.
The `mad-sys` ABI implementation files from the same source commit are
included in `crates/media/mad-sys/src/imp`.

MP3 fixtures are not distributed in this repository. Before running the full
host test suite or building the MP3 decoder probes, provide files outside the
repository and set these environment variables to their absolute paths:

- `LIBOGC_MAD_REFERENCE_MP3`: the exact reference fixture used for the C
  comparison and `libmad_rs_decode_probe`, SHA-256
  `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`.
  Obtain a copy you are permitted to use; the fixture is not available in
  this repository. A different file does not match the fixed expected values.
- `LIBOGC_MAD_PROBE_MP3`: test audio for `mad_decode_probe`, in MPEG-1 Layer
  III, 44.1 kHz stereo, with at least 20 frames. The generation command in
  [the probe fixture guide](../probes/ogc-system-probe/test-data/README.md)
  can create a suitable file outside the repository.

Export both variables before `cargo xtask test-host` and probe builds. A missing
reference fixture fails the relevant host test; a missing probe fixture fails
the relevant target build. Neither condition counts as a pass. These variables
are not needed for the ordinary runtime compilation check.

Physical hardware workflows also depend on a local hardware procedure and
Wiiload-rs, which are not included here. See [the hardware validation guide](wii-hardware-validation.md)
and [agent instructions](../AGENTS.md). Some recorded paths and network
addresses describe the original development setup and must be checked for a
new setup.

The Wii standard library build also requires a nightly compiler and `rust-src`
that match [`rust-std/source-lock.json`](../rust-std/source-lock.json). The
preparation script checks both the compiler commit and the library source
hash. Installing the current nightly alone does not guarantee a match.
