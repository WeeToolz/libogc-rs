# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 50. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::search | WPAD Wii discovery requires an adapter | PASS | [log L75](wpad_probe/probe.log#L75), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::search | WPAD discovery starts with adapter disabled | PASS | [log L74](wpad_probe/probe.log#L74), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::search | WPAD discovery leaves adapter disabled | PASS | [log L80](wpad_probe/probe.log#L80), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID discovery requires an adapter | PASS | [log L76](wpad_probe/probe.log#L76), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::search_bluetooth_hid | WPAD discovery starts with adapter disabled | PASS | [log L74](wpad_probe/probe.log#L74), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::search_bluetooth_hid | WPAD discovery leaves adapter disabled | PASS | [log L80](wpad_probe/probe.log#L80), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::stop_search | WPAD stop discovery requires an adapter | PASS | [log L77](wpad_probe/probe.log#L77), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::stop_search | WPAD discovery starts with adapter disabled | PASS | [log L74](wpad_probe/probe.log#L74), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::stop_search | WPAD discovery leaves adapter disabled | PASS | [log L80](wpad_probe/probe.log#L80), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::start_pairing | WPAD pairing rejects discovery without an adapter | PASS | [log L78](wpad_probe/probe.log#L78), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::start_pairing | WPAD discovery starts with adapter disabled | PASS | [log L74](wpad_probe/probe.log#L74), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::start_pairing | WPAD discovery leaves adapter disabled | PASS | [log L80](wpad_probe/probe.log#L80), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::set_host_sync_button_callback | WPAD sync callback fires once and can be removed | PASS | [log L79](wpad_probe/probe.log#L79), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::set_host_sync_button_callback | WPAD discovery starts with adapter disabled | PASS | [log L74](wpad_probe/probe.log#L74), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::set_host_sync_button_callback | WPAD discovery leaves adapter disabled | PASS | [log L80](wpad_probe/probe.log#L80), [page-002](wpad_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Checks disabled adapter errors and the synchronous host sync callback in Dolphin. Active discovery, wireless pairing, and hardware behavior remain open.
