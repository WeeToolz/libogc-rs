# Parity completion plan — 2026-11-28

This is the active work order for finishing parity with the pinned C libogc
reference. The source of truth remains the generated API inventory, the static
review ledger, the hardware register, and each linked component report. Counts
below are a checked snapshot, not completion criteria.

## Checked starting state

- `cargo xtask check-inventory`: 3,426 inventory entries; current.
- `cargo xtask audit-stubs`: 126 unverified API entries; audit fails as designed.
- `cargo xtask review-status`: 92 crates; 63 reviewed, 4 parity blocked,
  23 awaiting independent re-review, and 2 reviews incomplete.
- `cargo xtask audit-hardware-register`: 55 rows; 14 passed and 41 not tested.
- `git status`: one verified cache-probe commit ahead of `origin/main`;
  network DNS prevented the push.

## Work order

1. **Finish the source-review sweep.** Complete the `ogc-fat` and `gc-wad`
   comparisons with their exact pinned references and required fixtures. If a
   reference or signing fixture is unavailable, record the exact gap and keep
   the review incomplete. Use the [review workflow](REVIEW-WORKFLOW.md) to
   coordinate each review batch and its fixes.
2. **Resolve the source-level blockers.** Fix confirmed mismatches in
   `lwbt-sys`, AESND, and `libdb-sys` after their review batch is complete and
   its findings are recorded. Missing fixtures in a separate review batch can
   stay open while this work proceeds. Add focused regression checks and
   target evidence. Assign each fix to an independent reviewer for approval.
3. **Hand off fixed code for independent review.** Assign every
   `needs_re_review` row to an independent reviewer. Clear that state only
   after the reviewer records a complete-diff comparison and its result.
4. **Resolve the remaining API evidence.** Work from the 126 exact API rows,
   grouped by crate and existing probe. Reuse existing test inputs, fixtures,
   and Dolphin runners. Add missing success and failure assertions to the
   smallest relevant probe. Record the exact API, named check, run identity,
   output, and limits. Evidence references alone do not prove behavior.
5. **Run the requested runtime checks.** With required external fixtures
   available, run host tests, strict Clippy, inventory checks, source audit,
   and the affected Dolphin probes. Keep the `audit-stubs` failure visible
   until every API has exact evidence.
6. **Complete physical acceptance.** Use the documented Wiiload runner only
   when a Wii is connected and a qualified recovery path is confirmed. Verify
   each applicable physical cache, DMA, interrupt, peripheral, and storage
   contract. Keep the 41 untested hardware rows open until actual results are
   recorded. Never substitute Dolphin or compilation for a physical result.
7. **Reconcile and publish.** Regenerate the inventory, validate each ledger,
   inspect the final diff, remove only task-owned temporary files, commit
   verified batches, then push when network access is available. Do not merge
   branches; the user controls merges.

## Guardrails from earlier mistakes

- **Do not infer parity from counts or labels.** Verify exact named behavior
  and record pass, fail, or untested. A changed classification can change the
  audit count without adding evidence.
- **Validate fixtures before use.** Check size and cryptographic hash against
  signed metadata before selecting a NAND or title fixture. If that fixture is
  unavailable, use an authorized known-good test title and identify the result
  as a test-title handoff. Do not imply that it proves System Menu behavior.
- **Make each probe expectation match its fixture.** Check the C behavior and
  the fixture's observed contents before adding an assertion. A missing device,
  ticket, network response, or directory entry is not proof of a Rust defect.
- **Separate build, compile, emulator, and hardware evidence.** A target test
  compile is not a test run. A successful DOL exit is not a passing probe. A
  Dolphin result is not physical Wii evidence. Preserve untested cases as open.
- **Check prerequisites before long commands.** Record required environment
  variables and fixtures first. A missing MP3 fixture can stop Clippy before
  linting; a sandboxed loopback bind can stop host tests. Correct the environment
  and rerun the exact command before recording a pass.
- **Use existing runners and keep artifacts bounded.** Reuse `cargo xtask`,
  `dolphin-validate`, probe support, and the hardware run scripts. Save durable
  run records under the established evidence directories. Remove temporary
  logs, scratch files, and private emulator profiles after confirming the
  durable report is complete.
- **Use narrow, reviewable commits.** Check status first; stage only the task
  files; verify the diff and relevant checks; then commit. Preserve unrelated
  work. Report any failed command and do not label a partial result as complete.

## Current blockers

- No connected physical Wii was established in this work session; physical
  cache and DMA acceptance remains open.
- Independent reviewers are required for the 22 current pending re-reviews.
- The HBC Reborn source checkout is available at commit
  `90025e8fe503795f5246392e044e010527879cc6`, but its working tree has
  unrelated changes. The WAD packer reference files are clean. The WAD
  differential template, content files, and common key are not available.
- Open API and source-review work remains as listed in the generated inventory
  and `config/static-review-scope.csv`.

## Progress — 2026-11-28

- Recorded the locked-cache burst run. It passed 78 Dolphin checks, but
  `lc_queue_length` pending-work behavior remains untested because Dolphin
  drained the queue before sampling. The run does not verify physical Wii DMA.
- Added evidence and cleanup requirements to this repository's `AGENTS.md`.
- Independently reviewed the `gc-elf` channel entry vector. The reference
  assembly and checked-in binary match byte for byte; 11 host tests and the WAD
  ledger check pass. Physical IOS entry remains open. The source review ledger
  now has 64 reviewed crates and 22 pending re-reviews.
- Corrected the `gc-wad` note: the reference source exists, but differential
  inputs are missing. Eight host tests pass; the package remains incomplete.
- `rust-anything` CLI and doctor run. The CLI is available, but no Codex MCP
  server is configured. No MCP setup change was needed for this work.
- The BlockFS ledger claim was released after the available records failed
  to establish reviewer independence. The source and tests were inspected.
  The review remains open for a reviewer who can establish independence.
- Pushed the verified commits through `d63d3f65` to `origin/main`.
