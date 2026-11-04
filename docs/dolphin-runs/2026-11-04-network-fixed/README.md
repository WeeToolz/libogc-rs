# Network option and error fixes

The final standard and development DOLs each passed **52 checks, zero
failures**, using the patched macOS Dolphin build. See the
[standard log](standard.log), [development log](development.log), and
[results and DOL hashes](results.json). Local IP addresses are redacted.
These results supersede the earlier network baseline and diagnostic failures
for the checks present here. They do not establish all network API behavior,
physical Wii behavior, UDP development reporting, or recovery.

## Causes and changes

The SDK `getsockopt` backend now issues a real IOS request instead of always
returning an error. It validates reply length before copying data to the
caller. The probe checks that a failed query preserves caller output; host
checks cover fixed reply bytes and invalid lengths.

Dolphin on macOS previously forwarded several big-endian IOS option payloads
directly to native socket calls, used incompatible native option numbers, and
returned success without applying TCP_NODELAY. The saved
[patch](../../../scripts/patches/dolphin-macos-network-options.patch) converts
known integer and timeval options and calls real macOS setsockopt/getsockopt.
It does not cache requested values or synthesize success. The unmodified
Dolphin application still has these limitations.

A controlled test used the same DOL SHA-256 with the same Dolphin source
revision before and after the emulator patch: the
[unpatched log](control-unpatched.log) had 44 passes and seven failures; the
[patched log](control-patched.log) had 47 passes and four failures. Timeout,
TTL, and TCP keepalive changed to passes. This isolates those failures to
Dolphin, independently of the later SDK and probe corrections.

The Boolean round-trip assertions now accept zero/nonzero, as the BSD contract
requires. Numeric options still require the exact requested value. The
connect check verifies that the eight-byte Wii address is accepted and the
port-zero request fails through the public SOCKET_ERROR contract; the earlier
probe incorrectly expected a local -EINVAL validation that the implementation
and C reference do not perform for port zero. The bind wrapper now translates
IOS SO_EADDRINUSE (-3) to the installed PowerPC newlib EADDRINUSE (-112). This
fix is specific to that error path; other error conversion paths need their
own coverage. The debugger bind fixture still uses its own handles.

## Reproduce

Use the source revision in `results.json`. Apply the network patch with
`git apply /path/to/dolphin-macos-network-options.patch` from that Dolphin
checkout, then configure and build Dolphin with CMake. The tested build used
macOS, Release, and `-DENABLE_VULKAN=OFF`. The unrelated opt-in USB capture
patch was present in both control builds, but no USB device was selected.

Build the standard DOL with `cargo xtask build-dolphin-probes` and run
`cargo xtask dolphin-validate --no-build --probe network_probe --dolphin
/path/to/patched/Dolphin --artifact-dir /path/to/new-results`.

Build the development DOL with `cargo xtask build-dol ogc-system-probe --bin
network_probe --dev --probe-features dolphin-automation`. Set
`LIBOGC_DOLPHIN_DEV_DOL` and `LIBOGC_DOLPHIN_DEV_ELF` to the matching files from
its manifest and run the same validation command. Required external MP3
fixtures must be supplied through the build environment, as documented in
`docs/dependencies.md`.

Run the shared reply decoder host checks with `rustc --edition=2024 --test
crates/services/lwip-sys/src/socket_option_reply.rs -o /path/to/test-binary`,
then execute that binary.
