# Storage API audit: Dolphin launch attempt

## Target and expected result

The target was the registered `storage_probe` DOL. The probe should check the
CARD mount, file, persistence, delete, and cleanup paths in a private Dolphin
profile. The run did not request the destructive format path.

## Build

The first build stopped because `LIBOGC_MAD_PROBE_MP3` was not set. The retry
used the local external fixture for both MP3 variables:

```sh
LIBOGC_MAD_PROBE_MP3=[external tools]/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[external tools]/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-storage-api-audit
```

The DOL built. Its SHA-256 is
`6d8c751d6bbb4a0142c149b62bef140c666be3ab6d6bc5647619d6eb33d700f4`.
The fixture SHA-256 is
`c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`.

## Dolphin result

The runner stopped before it launched the DOL. Dolphin aborted while handling
`--version` with `SIGABRT`. A direct invocation reported that the Qt build
requires the `neon` processor feature. Therefore, this attempt provides no
runtime evidence for the probe or its APIs.

The next Dolphin run requires a Dolphin build that starts successfully on this
host. The storage group check remains separate: `cargo xtask check-group
storage` completed for 15 Wii runtime packages.

## Repeat after CARD owner-state checks

The probe now also checks the owned mount request slot, mounted owner slot,
attached state, and pre-detach result. The target DOL built with the external
fixtures from `[external tools]/libogc-rs-fixtures`; its SHA-256 is
`64733665fb6b67b5fa837aad73e093c0de0913bfdd25df754c121046639fa179`.

The Dolphin runner again stopped before it launched the DOL because Dolphin
aborted on `--version` with `SIGABRT`. Direct invocation reported the missing
`neon` processor feature. This repeat adds no runtime evidence. The new owner
checks remain unverified until a compatible Dolphin build or Wii run executes
the DOL.

## Repeat after CARD invalid-channel checks

The probe now checks invalid-channel results for CARD status, metadata,
directory search, mount, create, delete, and format calls. It checks output
preservation and verifies that rejected asynchronous requests do not call
their callbacks. The target DOL built with the same external fixtures; its
SHA-256 is
`a795ee6da078dcf369556940eb07c4d1e7738facc9c99a883e5fcbf7b25edd67`.

Dolphin again aborted on `--version` with `SIGABRT` before it launched the
DOL. The new checks compiled, but they remain unverified until a Dolphin or Wii
run executes them.

## Build after fixed-path cleanup protection

The probe now uses exclusive create for fixed FAT paths and skips CARD save
checks when `SAVE` already exists. It only removes probe paths created by the
current run. The target clippy command passed, and `cargo xtask
build-agent-probes` built the updated DOL. Its SHA-256 is
`0d1d07be662ed7115529caed586f4c1962c6f41f770b4d6c7727f19f9e4e2ab4`.

This DOL has not run. The available Dolphin build still aborts before launch
while handling `--version`, as recorded above. The build result adds no runtime
evidence.

## Build with DI configuration and partition error checks

The probe now checks DI configuration setter effects on no-disc status, the
no-disc init-callback path through Rust and C setters, and closed-descriptor
errors for Rust and C partition APIs. The updated DOL SHA-256 is
`37547912e2242cde337056fcfa16312ff8d23d62d0fafcf7699cbf7004465350`.

The probe compiled, but Dolphin did not run it. These checks add no runtime
evidence. A successful partition operation and init-callback delivery after a
successful mount still need a disc test.

## Build with CARD API source coverage

The probe now checks the remaining CARD facade and system APIs, including
output preservation, invalid-channel results, owned async request state,
completion status, and game/company identifier selection. It skips an existing
`SAVE` file and deletes only probe files created by the current run. The DOL
SHA-256 is
`200cacf4b0c2b7df3f255f97fdae99c82c09f92b217487abff1e9105719f6731`.

The DOL built, but it did not run. Dolphin still aborts before launch, and
physical CARD behavior is untested. The zero open count in the generated API
inventory records mapped source checks; it is not a runtime pass.

## Build after CARD metadata error checks

The host metadata suite now has 18 passing tests. A new test checks malformed
checksum input, short metadata blocks, and invalid FAT block counts. It found
that `prepare_fat` changed its input before it returned an invalid-count error.
The implementation now checks the count before it changes the block. Strict
Clippy passes for `ogc-card-metadata`.

The updated Wii `storage_probe` DOL has SHA-256
`973d80e1c513f571263c6142022e1ac533ffae0cd8784ef1342fe8769cc2c2a5`. The
updated GameCube `card_probe` DOL has SHA-256
`146a507d5defb0eaac284996d18569a6fb8ebe505153c96c6b21e11b9c07213e`. Both
DOLs build. The sandboxed launch attempt stopped on Dolphin's `--version`
check, so it produced no runtime result. A later host-authorized CARD run is
recorded below; it does not cover these Wii and CARD DOL versions.

## Build after CARD cleanup-result checks

The probe now records the result of unmount after an incomplete create, write,
or read. If create completes with an inconsistent submission result, the
probe closes its returned file before it unmounts. Previously these unmount
results were discarded. The updated Wii `storage_probe` DOL has SHA-256
`dcd485e43ff2bbb4b25f09436ffc7ad30de917d19711deaf1e4b94f77878127a`. The
updated GameCube `card_probe` DOL has SHA-256
`a91bbbfe11e54fc098ff5284b7dbb2d72baebec67ac3735acff4595eb7bd1564`.
Both DOLs build without compiler warnings. Those sandboxed attempts did not
produce a Dolphin result. See the later CARD run below for a subsequent run
with the same Dolphin executable.

## Later CARD run

The launch failures above came from sandboxed runner attempts. A later
host-authorized run started Dolphin 2609a and ran the current GameCube CARD
probe. It passed 98 checks with no failures after one stale persistence
expectation was corrected. See the [CARD accessor report](../2026-11-03-card-accessor-update/README.md)
for the DOL identity, both probe logs, and the inspected output pages. This
later run does not add runtime results for the other DOLs listed above.
