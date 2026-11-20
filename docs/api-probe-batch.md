# Dolphin API evidence batches

Use the batch helper from the repository root with Python 3.11 or later.
It uses the existing Dolphin runner and inventory checks.

1. Run `python3 scripts/api-probe-batch.py plan` to rank the unverified
   groups. Use `plan --crate ogc-network` to see exact entries.
2. Select related entries and use `manifest` to fill in the inventory
   identifiers. Add direct calls with independent expected results. Fill
   each `checks` list with the exact text after `PASS`. If a symbol has
   more than one inventory entry, give `SYMBOL@LINE` or use `--source` to
   select the repository-relative API source file.
3. Set the MP3 fixture variables in [dependencies.md](dependencies.md).
   Run the helper with a fresh artifact directory. The runner builds and
   runs the probe in a private Dolphin profile. Open `batch-report.md` to
   review the checks, log lines, captured pages, and missing or failed
   checks. To compare observed outcomes with a previous run, pass
   `--compare PREVIOUS_ARTIFACT_DIR`. Record your coverage limits in the
   manifest's optional `limits` list or in the batch README. Inspect the
   screenshots before accepting the evidence.
4. Run `finish` to match the selected entries to the recorded pass lines,
   add the evidence rows, rebuild the inventory, and run formatting,
   Clippy, host tests, and the source and completeness audits. The helper
   uses one Rust inventory scan to refresh the inventory and report
   outstanding entries. It saves the audit log and `completeness-audit.json`.
   The JSON gives the inventory and remaining counts, plus the location,
   classification, and reasons for each remaining API. Review the diff
   and commit the batch.

Run one batch at a time in a worktree. Edits outside the artifact directory
during or after a run invalidate the source fingerprint. Use a new run for
source changes. The helper records a failed run when the runner produces
a suite report. Each batch still needs a judgment about what the checks
prove. A passing label alone cannot establish physical Wii behavior or
full coverage of an API.

For example, draft a manifest for an unverified network API. Replace the
probe function and symbol with your selection.

```sh
python3 scripts/api-probe-batch.py manifest --crate ogc-network --probe runtime_probe --evidence-path probes/ogc-system-probe/src/runtime_probe.rs --function probe_socket_select --api-prefix net select > /tmp/libogc-api-batch.json
```

After you fill in the check labels, run the batch and review the report.

```sh
python3 scripts/api-probe-batch.py run /tmp/libogc-api-batch.json docs/dolphin-runs/new-batch --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin
python3 scripts/api-probe-batch.py finish docs/dolphin-runs/new-batch
```

On macOS, the shell sandbox can prevent Qt from using desktop services. If
Dolphin aborts during `--version` with a Qt NEON or pasteboard error, use the
normal approval path to run that check with desktop access. If it passes,
run the batch helper with the same access and a new artifact directory. The
helper uses a private Dolphin profile.

Use `report` to regenerate a report from a saved run. It prints Markdown.
For an older run without `batch.json`, add `--manifest PATH`.

```sh
python3 scripts/api-probe-batch.py report docs/dolphin-runs/new-batch --compare docs/dolphin-runs/previous-batch
```

To audit an existing inventory without changing it, run
`cargo xtask audit-stubs --json /tmp/libogc-completeness.json`. Add
`--refresh-inventory` to regenerate the inventory and audit the same
snapshot. Both modes exit with status 1 if completeness entries remain.
The JSON is written after the source audit and inventory step succeed.
