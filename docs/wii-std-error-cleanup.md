# Wii std error mapping and cleanup

Updated: 2026-10-14. Production changes are implemented. The controlled fault
probe passed 24 checks with zero failures in Dolphin. Real media, network
traffic, removal, and hardware failure acceptance remain separate.

## Error domains

Newlib errno, IOS socket replies, IPC transport failures, and typed FAT/LWP
errors have separate mappings. Unknown errors are not guessed or reported as
success. Socket errors retain their original domain and code in the std error
message. IPC -4 maps to EINVAL, -5/-22 to ENOMEM, -6 to ENOENT, and -8 to EAGAIN.
The same numeric IOS socket result can have a different meaning; IOS -6 is
EAGAIN. IPC errors are not retried as IOS socket readiness notifications.

FAT invalid boot data maps to EBADMSG/InvalidData. An unsupported FAT variant
maps to ENOTSUP. Other FAT mappings preserve missing paths, permissions, device
removal, busy handles, storage exhaustion, and size limits. LWP cancellation
maps to ECANCELED instead of EIO; its message keeps cancellation distinct from
TLS cleanup failure. The std Newlib classifier recognizes malformed protocol
or device data and network buffer exhaustion.

## Device callbacks

`DeviceFile::open_checked` uses the exact supplied flags and returns negative
Newlib errno. The older Option-returning open helpers remain compatibility
interfaces. Append helpers retain O_APPEND on every attempt.

Read, write, seek, sync, and close capture callback errno before any subsequent
operation. Stale errno is cleared before each callback. A failed callback that
does not set errno becomes EIO. Oversized read/write counts, invalid status
results, and zero-progress writes are rejected. Partial writes use the actual
completed byte count and stop at the first failure.

`DeviceFile::close` consumes the handle, attempts close once, releases local
storage, and returns the real error. Drop also attempts close once. Both record
cleanup failure through `ogc_sysbase::device_close_failures()` and
`last_device_close_errno()`. A failed backend close can leave backend state
uncertain; local release does not claim successful device cleanup or persistence.
The handle is not retried after it has been consumed.

FAT devoptab close now records failures as well as returning them. Typed FAT
close/Drop diagnostics remain available through `file_close_failures()` and
`last_file_close_errno()`. Explicit sync reports persistence failures directly.

Network initialization, cancellation, and teardown no longer discard IOS
device close failures. If the primary operation already failed, its error is
returned and a later close failure is recorded. If the operation succeeded,
failed close becomes the returned failure. The std runtime's network snapshot
includes `initialization_close_failures` and `last_initialization_close_error`
in addition to the existing socket close diagnostics. The last initialization
close code retains the original IPC value.

## Verification

Run the shared production engine and mapping tests:

```sh
rustc --edition=2024 --test tests/wii-std-errors/mappings.rs \
  -o target/wii-std-errors-tests
./target/wii-std-errors-tests
```

The run passed 16 tests. These use actual production mappings, the LWP enums,
and the devoptab/initialization cleanup engines with explicit failure injection.
They check independent expected errno values, domain separation, byte results,
error preservation, local release, and diagnostic counts.

Build the target probe:

```sh
cargo xtask build-wii-std-app lwp-probe --bin wii_std_error_cleanup_probe \
  --features wii-std-sync \
  --output target/xtask/probes/wii_std_error_cleanup_probe.dol
```

`cargo xtask build-wii-std-probe` also includes this probe. It installs an
explicit controlled fault endpoint in the real devoptab registry and calls the
production DeviceFile methods. It checks actual std errno classification,
failed-open allocation release, exact append flags, partial writes, callback
errors, malformed counts, explicit close, Drop diagnostics, and endpoint removal.
The endpoint represents callback failures; it is not a model of FAT, SD, USB,
or network hardware. Failed checks are retained on the final screen.

The DOL passed 24/0 in Dolphin. SHA-256:
`968bb26c516ae9179a42d6c5d11a6207956e6533ce96000542a758dc1344ed17`.
See the [final Dolphin screen](hardware-evidence/wii-std-error-cleanup-dolphin-20261014-final.png).

The earlier service and application DOL files were not replaced by this build.
