# DVD mount and direct DI on Dolphin

Dolphin 2609a ran `dvd_probe` with the synthetic Wii disc fixture. The [media
log](media.log) has 52 passes and zero failures. DVD mount and direct DI mount,
disc ID, and Rust and C ABI synchronous and asynchronous reads passed. The
async checks verify the IOS completion value, callback buffer, and bytes. The
[no-media log](no-media.log) has 183 passes and zero failures. Suite reports
record the DOL hashes, launch arguments, and results: [media](media-suite.json)
and [no media](no-media-suite.json).

The earlier [baseline](../2026-11-04-dvd-c-read-check/README.md) had five
failures. The direct DI probe did not call mount after init. With mount added,
[temporary diagnostic instrumentation](diagnostic.log) showed the reset stage
(`2`) returned `1` and the mount callback reported `-5`. The C reference
expects `4` for this stage. Dolphin's IOS implementation returns its normal
success result `1` for reset. The mount state machine now accepts either reply
for reset and continues to reject other values. Other stages still require
`1`. The diagnostic instrumentation was removed after the measurement.

The probe now waits for the mount callback and requires `DVD_READY`. Its async
read callback expects the raw IOS success value `1`, not the synchronous
wrapper's normalized value `0`. The C ABI checks call the Rust-owned C ABI;
they are not a separate C implementation control.

Reproduce with `cargo xtask build-dolphin-probes`, then run
`cargo xtask dolphin-validate --no-build --probe dvd_probe --disc-image
/path/to/fixture.iso --dolphin /path/to/Dolphin --artifact-dir /path/to/new-run`.
Run the same command without `--disc-image` for the no-disc case. The fixture
SHA-256 was `eeabeac80855703cd5909fdad7ccaebcc97663bdede38a7afe8d241788c37335`.
The runner uses a private Dolphin profile. The fixture has no signed Wii
partition or filesystem. The drive mount result does not establish partition
opening, filesystem access, or physical Wii behavior.
