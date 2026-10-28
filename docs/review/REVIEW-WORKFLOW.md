# Static C parity review workflow

Use this workflow for the components listed in `config/static-review-scope.csv`.
The source trees used for the recorded reviews are:

- C: `../../C-sources/libogc` in the original development workspace;
  [upstream libogc](https://github.com/devkitPro/libogc) is not included in
  this repository. The recorded reference commit is
  `ff53c4a7b7ba5587d82305e710ca65bce1092e1c`.
- Rust: this repository

The C path is an example of the original local layout. A reviewer may use a
different checkout path, but must record the exact source revision and any
local changes. Some components use other references; check their review notes
and [source notices](../source-licenses.md). The goal is to determine, from
source evidence, whether Rust matches C behavior where possible and whether the
reviewed Rust code has confirmed bugs. Hardware verification is outside this
workflow.

## Review pass

The review pass is read only. Reviewers must not edit implementation code or commit fixes.

1. Check `config/static-review-scope.csv` before starting. Claim each related crate together, for example `ogc-si` and `ogc-si-sys`, by setting `work_status` to `in_review`, entering an owner, and updating the date. Commit the claim before reviewing. Do not start a crate that another owner has claimed.
2. Compare the Rust implementation with the relevant C source and header. Follow each operation far enough to identify its behavior and root cause. Record exact paths and line numbers.
3. Write findings in the linked Markdown review note. Label confirmed bugs `BUG FIX` only in the later fix pass. For review findings, state the incorrect behavior and C behavior. Keep confirmed mismatches separate from uncertain differences and intentional Rust additions.
4. Update the ledger. `review_status` records the parity result, while `work_status` records the current activity. Use `parity_blocked` with `work_status=finding` when a confirmed non-safety mismatch needs a fix. Use `review_incomplete` with `work_status=in_review` when source comparison is not complete and no blocker has been established for the unfinished scope. Use `reviewed` only when parity is established for the required scope; deliberate safety-only deviations are allowed when they are documented explicitly.

## Fix pass

Start the fix pass after the planned crate review sweep is complete and its findings are recorded. Do not start source fixes while review claims remain active unless the user changes this sequence. Assign each fixer in the ledger with `work_status=fix_in_progress`.

1. Fix only the confirmed findings assigned to that crate. Keep unrelated changes out.
2. Add or run regression checks for each fix where the target allows it. Do not claim hardware verification unless hardware was used.
3. Label each change `BUG FIX` in the source or review note. Explain what was wrong and how the change corrects it.
4. Commit related fixes in logical chunks. After implementation, the implementer records `work_status=re_review` in the ledger and hands the scope to an independent reviewer. This ledger handoff is the implementer's final review-related action: the implementer must not audit or approve their own changes. Assign a different reviewer to compare the fix with C and inspect the complete final diff. Prefer the original static auditor so the reviewer can reuse its source-comparison context. If the original auditor is unavailable or was also the implementer, assign another independent reviewer. Record the original auditor in the linked note so the handoff can find them.
5. The independent reviewer owns the re-review and its result. A fix awaiting that pass uses `review_status=needs_re_review` and `work_status=re_review` when no other confirmed blocker remains. If another confirmed mismatch remains, keep `review_status=parity_blocked` while `work_status=re_review` records the current bounded re-review activity. After the reviewer accepts the fix, clear the owner and set `work_status=idle`. Set `review_status=reviewed` only when parity is established and no unresolved non-safety finding remains. If the re-review confirms a mismatch, set `review_status=parity_blocked`, set `work_status=finding`, and assign its report owner. If the remaining issue is only unfinished review scope, use `review_status=review_incomplete` instead.

## Ledger fields

`config/static-review-scope.csv` is the shared coordination ledger. It has one row per crate.

| Field | Meaning |
| --- | --- |
| `package` | Workspace crate name. |
| `review_status` | Parity result: `reviewed`, `parity_blocked`, `needs_re_review`, or `review_incomplete`. `reviewed` means C parity is established for the required scope, allowing only documented safety deviations. |
| `owner` | Thread and agent name, such as `thread-a/root` or `thread-b/gpt-6-luna-1`. Blank means unclaimed. |
| `work_status` | Current activity: `idle`, `in_review`, `finding`, `fix_in_progress`, `re_review`, or `doc_update`. This is separate from the parity result. |
| `note` | Markdown report for the crate or related crate group. |
| `updated` | Date of the latest claim or status update. |
| `summary` | Short handoff note: scope, finding, or next action. |

The `review_status` values are mutually exclusive:

- `reviewed`: parity is established for the required scope. Documented safety-only differences are permitted.
- `parity_blocked`: at least one confirmed non-safety behavior or ABI mismatch remains. This status takes precedence even when a separate fix is currently in `re_review`.
- `needs_re_review`: no confirmed blocker is currently recorded, but a fix or bounded change must pass independent parity review before parity can be declared.
- `review_incomplete`: no confirmed blocker is currently recorded, but the required C/Rust comparison or validation scope is not complete. This state does not itself assert a bug.

## Example claim and handoff

Before reviewing a crate group, change its rows to `review_incomplete,in_review`, set `owner` to a unique thread/agent label, and add a short scope summary. For example:

```csv
ogc-si,review_incomplete,thread-b/gpt-6-luna-1,in_review,docs/review/libsi.md,2026-10-01,Reviewing SI behavior against SI C sources.
ogc-si-sys,review_incomplete,thread-b/gpt-6-luna-1,in_review,docs/review/libsi.md,2026-10-01,Grouped with ogc-si.
```

Run `cargo xtask review-status` to check that the ledger has one valid row for every crate and no duplicate or stale claims. Commit the claim before starting work. At handoff, add the evidence and finding summary to the Markdown note, update the ledger state, and commit both together. A fixer uses `fix_in_progress`; after the fix commit, a different owner uses `re_review`.

Write `config/static-review-scope.csv` as valid CSV. Quote any field that contains a comma, quote, or newline, then run `cargo xtask review-status` before committing. This prevents a summary from shifting columns or hiding later crate rows from the ledger parser.

When an owner finishes a pass, update the ledger and note in the same commit. A new thread must read the ledger first and claim rows before editing or reviewing them. For first-pass reviews, record the auditor's agent identifier in the report. At fix handoff, the implementer marks the related rows `re_review` and assigns the independent reviewer; the implementer does not conduct a self-review. If a crate group shares one C implementation, claim all related crates together.

## Historical exceptions

- Commit `655ab20` contains the GameCube AI register fix. It was committed before this review/fix separation was stated.
- An `ogc-gx` TPL file-loading change was in the worktree before this workflow
  was stated. Check the current GX review and ledger for its present status.
