# WPAD public lifecycle

This Dolphin run checks seven public `wpad-sys` APIs:

- `get_status`, `scan_pads`, and `shutdown` in the WPAD lifecycle.
- `probe` when no controller is present.
- `orientation`, `read_pending`, and `pad_status` with an invalid channel.

Dolphin passed 43 checks. The batch audit reduced the unverified count from 487
to 480. The full inventory refresh also found that 26 functions in the private
`powerpc_backend` module were marked `pub` but were only reachable inside the
crate. They now use `pub(crate)` visibility and no longer appear as separate
public API entries. The inventory now contains 3,404 public API entries.

The probe does not test connected Wii Remote traffic, pairing, or physical Wii
behavior. See the [batch report](batch-report.md), [suite result](suite.json),
and [completeness audit](completeness-audit.json).
