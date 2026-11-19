# Dolphin evidence report

Probe `hid_descriptor_probe` reported `pass`.
Passed checks 19. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb-sys::convert_hid_descriptor_lengths | HID lengths convert for every complete entry | PASS | [log L83](hid_descriptor_probe/probe.log#L83), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::convert_hid_descriptor_lengths | HID truncated entry is left unchanged | PASS | [log L84](hid_descriptor_probe/probe.log#L84), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::convert_hid_descriptor_lengths | HID short header remains unchanged | PASS | [log L85](hid_descriptor_probe/probe.log#L85), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::prepare_hid_v5_device_with | HID v5 open checks and frees descriptors after best-effort resume | PASS | [log L86](hid_descriptor_probe/probe.log#L86), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::prepare_hid_v5_device_with | HID v5 open suspends and returns descriptor failure | PASS | [log L87](hid_descriptor_probe/probe.log#L87), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::prepare_hid_v5_device_with | HID v5 open accepts positive descriptor status | PASS | [log L88](hid_descriptor_probe/probe.log#L88), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::prepare_hid_v5_device_with | HID v5 open preserves descriptor error when suspend fails | PASS | [log L89](hid_descriptor_probe/probe.log#L89), [page-003](hid_descriptor_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Descriptor conversion and injected preflight policy only; no USB device, IOS transfer, or physical Wii behavior is covered.
