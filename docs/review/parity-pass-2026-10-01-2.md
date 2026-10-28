# Additional C-parity pass — EXI and libmad stream helpers

Date: 2026-10-01

This pass compared two localized open areas against the current uploaded C libogc tree.

## `ogc-exi-sys`

Fixed the two findings from `libexi-interrupt-cleanup-audit.md`: queued unlock callbacks retain C's disabled-interrupt context while allowing EXI re-entry, and EXI0/EXI2 mask control now follows C's coupled/channel-specific rules including the debugger gate.

## `mad-sys`

Fixed small `mad_stream_*` state differences: bit-pointer initialization, buffer sync state, cumulative skip length, and the `MAD_BUFFER_GUARD`/state contract of stream synchronization. The current source already contains the earlier decoder async-layout and initialization fixes, so the review note was updated to record that those findings are stale.

This is a source-level parity pass only. The larger libmad algorithm/frame/filter/synthesis differences remain open. Build and hardware validation are not claimed here.
