# Mandatory Wii hardware procedure

Read and follow the workspace `../../DEVELOPER.md` before Wii capture,
Wiiload, recovery, or physical diagnostic work. Its pre-run requirements are
mandatory. Follow the parent `AGENTS.md` files. Component examples and old
launchers do not waive the guide's requirements. The AESND command and addresses
below are historical; verify its settings against the guide before use.
The workspace guide is not part of this repository. If it is unavailable,
do not perform these physical diagnostic operations until you have obtained
the applicable procedure.

# Real Implementation Policy

## Run DOL files in Dolphin

This repository has a Dolphin runner for testing Wii DOL probes. Use it when a task needs DOL runtime validation; do not stop after building a DOL if Dolphin is available.

PowerPC runtime tests use Wii DOL probes in Dolphin. When you review or
verify PowerPC tests, inspect the relevant probe and its checks before you
conclude that a test is not executed. Host test selection does not describe
Dolphin probe coverage. Match the probe checks to the behavior under review;
do not assume that a probe covers every target-only unit test. A target
`cargo check --tests` only compiles test code and does not run a probe.

- Run the registered probe suite with `cargo xtask dolphin-validate`. It builds the probe DOLs, starts Dolphin, checks the probe results, and writes a report under `target/dolphin-validation/`.
- Run one probe with `cargo xtask dolphin-validate --probe <probe_name>`. Add `--no-build` only when the current DOL is already built. Use `--dolphin PATH` if Dolphin is not at its default application path, and `--artifact-dir PATH` to choose where reports go.
- Build or refresh the probe DOLs without running them with `cargo xtask build-dolphin-probes`.
- For the Wii std thread probe, `scripts/run-wii-std-thread-dolphin.command` builds and launches it in Dolphin. Read the final PASS/FAIL summary; a clean Dolphin exit alone does not mean that the probe passed.

Report whether the DOL ran, its probe result, and the artifact/report path. Dolphin validation does not replace real-Wii hardware validation when the behavior depends on physical hardware.

## Build probes for Wiiload-rs

For real-Wii probe runs, this repository also uses `Wiiload-rs` (`Wiiload-rs/Cargo.toml`) to upload and run DOL files. Read its CLI help and current probe/report implementation before changing the workflow. A representative invocation is `cargo run --release --manifest-path Wiiload-rs/Cargo.toml -- run <probe.dol> --wii-ip <WII_IP> --host-ip <HOST_IP> --output target/wii-runs`; follow the runner's options for timeout, restart, and recovery behavior.

Build report-enabled probes with `cargo xtask build-agent-probes` (outputs are under `target/xtask/probes-agent/`). For a new probe, start from the existing report-enabled probe patterns in `probes/ogc-system-probe/src/probe_support.rs` and the focused probe modules. Reuse their structured Wiiload diagnostics and report flow, including run/build identity and check results. Keep probe checks tied to production APIs and observable results; do not replace them with a success-only report or a simulated hardware result. Record the Wiiload run ID, DOL SHA-256, saved report path, failed checks, and any untested hardware limits. See `docs/wii-hardware-validation.md` and `probes/README.md` for the evidence and hardware-run workflow.

### Run agent upload commands through Terminal

The agent environment blocks the network traffic used by Wiiload. When an existing agent `.command` file is provided for a Wii run, launch it through the signed-in macOS Terminal app using `/usr/bin/open -a /System/Applications/Utilities/Terminal.app "$PWD/<path-to-command-file>"` from the repository root. Follow the Terminal output and wait for the command's final report. Do not run Wiiload directly from the agent environment or replace the command file with a direct upload command; the command file contains the required run settings and recovery behavior.

For the AESND DOL, use this exact command from the repository root:

```sh
/usr/bin/open -a /System/Applications/Utilities/Terminal.app "$PWD/target/wii-runs/send-aesnd-agent.command"
```

This sends `target/xtask/probes-agent/aesnd_native_probe.dol` to the configured Wii (`10.0.0.242`) and waits up to 600 seconds for the report. The command requests a return to the System Menu after the test. That reset path has stalled during DSP shutdown on earlier runs, and the frame counter can keep moving while it is stuck. Treat the probe report as evidence for the checks it contains; a moving frame counter does not prove that System Menu return completed. Record this limitation in the run result.

This repository does not accept code that only appears to work. Implement the required behavior in production code. Do not use a stub, placeholder, fake implementation, simulated success path, or hidden no-op as a substitute for the real work. Always write DRY code.

## Required behavior

- The C tree is reference-only. No Rust crate should depend on unresolved libogc C symbols at link time.
- Perform the operation that the API contract describes.
- Use the real production backend, transport, state, validation, and persistence path.
- Return success only after the required work completes successfully.
- Propagate errors to the caller. Preserve useful error details when the public API permits it.
- Validate all required inputs, state, bounds, permissions, formats, and hardware responses.
- Use function arguments for their documented purpose. Do not ignore an argument only to satisfy a signature.
- Consume and check results from operations that can fail. Do not discard them with wildcard bindings or empty error branches.
- Keep in-memory state consistent with committed external state after success and failure.
- Make asynchronous completion report the final operation result. Submission success is not operation success.

## Prohibited code

Do not add code that:

- always returns `true`, `0`, `Ok`, `Some`, or another success value without doing the required work;
- catches, discards, logs, or converts an error and then reports success;
- returns a hard-coded result instead of reading or calculating the real result;
- bypasses validation, authorization, integrity checks, persistence, synchronization, or hardware communication;
- updates only local state when the contract requires external or persistent work;
- invokes a backend but ignores its result;
- calls a callback with success before the operation completes;
- uses empty bodies, `todo!()`, `unimplemented!()`, placeholder constants, dummy data, fixed probe values, or fake counters as production behavior;
- selects a host fallback, test path, or feature flag that hides an incomplete production implementation;
- weakens a test, audit, probe, allowlist, or expected result only to make a failing workflow pass;
- documents missing behavior as implemented.

If the real operation cannot be implemented, return a clear supported error and record the missing capability as open work. Do not report success.

## Tests and probes

Tests must prove externally observable behavior, not only that a function was called.

- Test the production implementation or a shared engine that production uses directly.
- Verify returned values, errors, persisted bytes, state transitions, callbacks, and remount or restart behavior as applicable.
- Add failure injection at real operation boundaries. Verify that each failure is returned and does not create invalid state.
- Use fixed independent expected values for checksums, encodings, and protocol data. Do not calculate the expected value with the function under test.
- Recreate state from serialized or persisted data when the contract requires persistence.
- A test double can represent a transport failure or controlled medium only when it is explicit, deterministic, and exercises the same production logic. It cannot replace the production implementation or be the only acceptance evidence for hardware behavior.
- Hardware-facing behavior requires a real-hardware probe. A probe must call production APIs and validate real results. It must not contain a replacement hardware model.
- Cleanup failures must be visible when they affect test validity.
- A skipped test is not a pass. Report the skip and its reason.

## Review checklist

Before completion, search every changed function for discarded results, unconditional success, unused arguments, empty error branches, hard-coded outputs, and bypassed validation. Confirm that:

1. Every success path performs all required work.
2. Every failure path returns or records the real failure.
3. Tests fail if the production operation becomes a no-op.
4. Persistence tests reload from the persisted representation.
5. Asynchronous tests distinguish request acceptance from final completion.
6. Source and completeness audits are run, and new findings are fixed rather than suppressed.
# Firmware Safety Policy

This repository may include embedded-device firmware or other operations that can make persistent changes to hardware. These operations require stricter safety rules than normal application code because a software failure can make a device unusable or destroy unique device data.

These rules apply in addition to the Real Implementation Policy above. If two rules overlap, follow the stricter requirement.

## Protect users and hardware

- Treat every target device as valuable and difficult to replace.
- Do not assume that the user has spare hardware, recovery tools, a hardware flasher, or advanced technical skills.
- Prefer designs that reduce the chance of an unrecoverable failure.
- Do not expose destructive operations without safeguards.
- If a dangerous sequence can be automated safely, automate it instead of requiring risky manual steps.
- If an expert-only destructive feature is necessary, require explicit warnings and deliberate actions before execution.
- Do not place irreversible behavior behind unsafe defaults.

## Understand the system before modifying it

Before making a persistent firmware change:

- understand the boot process;
- identify firmware components and their dependencies;
- identify components required for boot;
- identify available recovery modes;
- understand the relevant storage and flash layout;
- review available hardware and firmware documentation;
- verify assumptions with knowledgeable maintainers or device researchers when possible;
- use less intrusive experiments until the system is understood well enough for permanent changes.

Do not perform a permanent modification when critical system behavior is unknown.

## Prepare first, commit last

Perform as much work as possible before changing persistent device state.

Use this sequence when applicable:

1. Identify the device and hardware revision.
2. Inspect the current firmware and state.
3. Validate compatibility.
4. Read all required data.
5. Prepare the complete modification.
6. Validate the prepared result.
7. Create and verify required backups.
8. Commit the final change only after all earlier steps succeed.
9. Verify the written result.

When practical, stage writes in memory, a temporary file, or another non-destructive buffer and perform the final persistent write only after validation.

If preparation fails, abort without modifying persistent device state.

## Fail safely

Do not turn a recoverable error into a permanent failure.

If a critical operation fails:

- preserve device power and state when this improves recovery chances;
- do not continue with additional destructive writes;
- enter a safe recovery path when one exists;
- preserve useful logs and diagnostic information;
- return or expose the real failure;
- provide clear recovery instructions when user action is required;
- avoid automatic reboot, shutdown, reset, or power-cycle requests when they can reduce the chance of recovery.

A failure state must maximize the chance that the device remains recoverable.

## Perform compatibility and sanity checks

Do not assume the target environment matches expectations.

Before a permanent write, validate all applicable items:

- device model;
- hardware revision;
- firmware version;
- firmware layout;
- partition layout;
- storage device or flash type;
- existing modifications that can affect compatibility;
- required files and auxiliary data;
- file sizes, offsets, headers, bounds, and formats;
- expected hashes, signatures, or other integrity data;
- integrity of the tool and required support files.

If the environment is unknown, unsupported, malformed, or ambiguous, stop instead of applying a blind patch.

## Verify integrity

Use cryptographic hashes, signatures, or equivalent integrity checks when practical.

Maintain known-good identifiers for applicable inputs and outputs, including:

- supported input firmware;
- expected modified output;
- critical support files;
- tool binaries;
- device-specific data used during the operation.

Validate the prepared output before writing it to the device.

Do not treat an unrecognized firmware image as compatible only because it appears similar to a supported image.

## Protect users from accidental destruction

The software must make accidental permanent damage difficult.

- Disable destructive actions when required conditions are not met.
- Require deliberate confirmation for irreversible operations.
- Avoid one-step actions that erase critical system firmware.
- Revalidate destructive requests immediately before execution.
- Make accidental keyboard, touch, controller, or repeated-input events insufficient to trigger permanent damage.
- Do not rely on warning text as the only safeguard when software can enforce the condition directly.

The safe path must be the easiest normal path.

## Back up critical data

Before dangerous operations, back up data that may be needed for recovery.

This includes, when applicable:

- firmware;
- bootloader data;
- calibration data;
- device-specific configuration;
- unique identifiers;
- cryptographic keys;
- partition tables;
- other per-device information that cannot be reconstructed safely.

Verify that the backup contains the expected data.

Do not store the only recovery copy on the target device when failure of that device could destroy the backup.

A device can be flash-recoverable and still lose irreplaceable per-device data.

## Test before release

Do not use public users as the first test group for dangerous firmware changes.

Before release:

- test with a small trusted group when real-device risk exists;
- prefer testers who understand recovery procedures;
- ensure recovery hardware or procedures are available when practical;
- test failure paths, not only successful paths;
- confirm that errors stop safely without corrupting persistent state;
- test every known hardware revision that can affect firmware behavior;
- test different storage or flash variants when those differences matter;
- verify compatibility assumptions on real hardware.

A public beta is not a substitute for controlled safety testing.

## Support hardware revisions explicitly

Do not assume that all units with the same product name are internally identical.

For every supported revision:

- identify relevant hardware differences;
- confirm flash, storage, memory-layout, or boot differences;
- test the modification on that revision;
- mark unsupported or untested revisions clearly.

If the revision cannot be identified safely, do not perform the permanent change.

## Firmware pre-write checklist

Before any persistent firmware write, confirm all of the following:

1. The exact device and relevant hardware revision are known.
2. The relevant boot and recovery paths are understood.
3. The current firmware is known and supported.
4. All required inputs and bounds are validated.
5. The complete modification is prepared before commit.
6. Critical device-specific data is backed up and the backup is verified.
7. The expected result can be validated before and after the write.
8. A defined failure and recovery path exists.
9. The operation has been tested on equivalent hardware when real hardware behavior is required.

If any required item cannot be confirmed, do not perform the write.

## Firmware release standard

A firmware tool or persistent device modification is ready for release only when:

- unsupported environments are rejected safely;
- inputs are validated before writing;
- critical data is preserved or backed up;
- the write path uses a controlled commit;
- the result is verified;
- a recovery strategy exists;
- relevant hardware revisions have been tested;
- failure paths have been tested;
- user-facing warnings are clear and specific;
- normal user actions cannot easily cause permanent damage by accident.

For embedded firmware, functional correctness alone is not sufficient. Every operation must be designed around prevention, validation, recoverability, and protection of the user's hardware.
