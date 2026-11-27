# Dolphin evidence report

Probe `hid_descriptor_probe` reported `pass`.
Passed checks 49. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb-sys::read_intr_msg | USB sys unavailable read_intr_msg | PASS | [log L98](hid_descriptor_probe/probe.log#L98), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::read_intr_msg_async | USB sys unavailable read_intr_msg_async | PASS | [log L99](hid_descriptor_probe/probe.log#L99), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::read_blk_msg | USB sys unavailable read_blk_msg | PASS | [log L100](hid_descriptor_probe/probe.log#L100), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::read_blk_msg_async | USB sys unavailable read_blk_msg_async | PASS | [log L101](hid_descriptor_probe/probe.log#L101), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::write_intr_msg | USB sys unavailable write_intr_msg | PASS | [log L102](hid_descriptor_probe/probe.log#L102), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::write_intr_msg_async | USB sys unavailable write_intr_msg_async | PASS | [log L103](hid_descriptor_probe/probe.log#L103), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::write_blk_msg | USB sys unavailable write_blk_msg | PASS | [log L104](hid_descriptor_probe/probe.log#L104), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::write_blk_msg_async | USB sys unavailable write_blk_msg_async | PASS | [log L105](hid_descriptor_probe/probe.log#L105), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb-sys::read_ctrl_msg | USB sys unavailable read_ctrl_msg | PASS | [log L107](hid_descriptor_probe/probe.log#L107), [page-004](hid_descriptor_probe/pages/page-004.png) |
| ogc-usb-sys::write_ctrl_msg | USB sys unavailable write_ctrl_msg | PASS | [log L108](hid_descriptor_probe/probe.log#L108), [page-004](hid_descriptor_probe/pages/page-004.png) |
| ogc-usb-sys::read_iso_msg | USB sys unavailable read_iso_msg | PASS | [log L109](hid_descriptor_probe/probe.log#L109), [page-004](hid_descriptor_probe/pages/page-004.png) |
| ogc-usb-sys::read_iso_msg_async | USB sys unavailable read_iso_msg_async | PASS | [log L110](hid_descriptor_probe/probe.log#L110), [page-004](hid_descriptor_probe/pages/page-004.png) |
| ogc-usb-sys::write_iso_msg | USB sys unavailable write_iso_msg | PASS | [log L111](hid_descriptor_probe/probe.log#L111), [page-004](hid_descriptor_probe/pages/page-004.png) |
| ogc-usb-sys::write_iso_msg_async | USB sys unavailable write_iso_msg_async | PASS | [log L112](hid_descriptor_probe/probe.log#L112), [page-004](hid_descriptor_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
