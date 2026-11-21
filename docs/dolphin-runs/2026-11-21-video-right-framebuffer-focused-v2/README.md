# Right framebuffer wrapper check

The batch added a check for `ogc-video::set_next_right_framebuffer` to
`probe_video_framebuffer_updates`. It calls the setter with the live
framebuffer, checks `VideoSnapshot`, then restores the prior snapshot value.

Dolphin 2609a passed 749 runtime checks with zero failures. The focused result
is in [batch-report.md](batch-report.md); its check appears at
[`runtime_probe/probe.log`, line 418](runtime_probe/probe.log#L418), and the
captured screen is [page 17](runtime_probe/pages/page-017.png).

This proves the wrapper stores the pointer and reaches its system wrapper call.
It does not prove that VI latches or displays a right-eye framebuffer.
`ogc-video-sys::set_next_right_framebuffer` remains unverified. `console_init`
and `console_init_ex` also remain open because this batch did not test their
state-changing behavior.

The inventory changed from 194 to 193 unverified APIs. The evidence row is in
`config/implementation-evidence.csv`; the refreshed inventory is
`docs/generated/implementation-inventory.csv`. The global audit still finds
193 unverified APIs and three tracked `ogc-wiikeyboard-sys` audit-debt entries.
The batch helper's `finish` command therefore did not complete. The saved
[`completeness-audit.json`](completeness-audit.json) records the remaining API
entries. No evidence was added for those open entries.

The probe build and Dolphin run passed. `cargo fmt --all --check`,
`cargo clippy --all-targets`, `cargo test`, and
`cargo test -p xtask -p ogc-controller` passed during the finish attempt before
the global audit stopped it.
