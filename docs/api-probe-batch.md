# Dolphin API evidence batches

Use the batch helper from the repository root with Python 3.11 or later.
It uses the existing Dolphin runner and inventory checks.

1. Run `python3 scripts/api-probe-batch.py plan` to rank the unverified
   groups. Use `plan --crate ogc-network` to see exact entries.
2. Select related entries. Add a direct call and a check with an independent
   expected result for each API. Write a JSON manifest and include the
   exact text after `PASS` for every required check. Use a crate-relative
   API path to the source file and a repository-relative evidence path.
   Add the inventory's `line` when symbols share the same file and name.
3. Set the MP3 fixture variables in [dependencies.md](dependencies.md).
   Run the helper with a fresh artifact directory. The runner builds and
   runs the probe in a private Dolphin profile. Review the log and
   screenshots. Add a short README to the artifact directory that records
   the expected behavior, earlier failures, and evidence limits.
4. Run `finish` to match the selected entries to the recorded pass lines,
   add the evidence rows, rebuild the inventory, and run formatting,
   Clippy, host tests, and the source and completeness audits. The helper
   reports outstanding completeness entries and saves the audit output.
   Review the resulting diff and commit the batch.

Run one batch at a time in a worktree. Edits outside the artifact directory
during or after a run invalidate the source fingerprint. Use a new run for
source changes. The helper records a failed run when the runner produces
a suite report. Each batch still needs a judgment about what the checks
prove. A passing label alone cannot establish physical Wii behavior or
full coverage of an API.

Example manifest:

```json
{
  "probe": "runtime_probe",
  "cases": [
    {
      "crate": "ogc-network",
      "path": "src/lib.rs",
      "symbol": "gai_strerror",
      "evidence_path": "probes/ogc-system-probe/src/runtime_probe.rs",
      "function": "probe_ipv4_values",
      "api_path": "net::gai_strerror",
      "checks": ["Network resolver error text matches known and unknown codes"]
    }
  ]
}
```

```sh
python3 scripts/api-probe-batch.py run /path/to/batch.json docs/dolphin-runs/new-batch --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin
python3 scripts/api-probe-batch.py finish docs/dolphin-runs/new-batch
```
