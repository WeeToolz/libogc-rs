# Dolphin evidence report

Probe `hid_descriptor_probe` reported `pass`.
Passed checks 34. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::read_intr_msg | USB unavailable read_intr_msg | PASS | [log L83](hid_descriptor_probe/probe.log#L83), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_intr_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_intr_msg_async | USB unavailable read_intr_msg_async | PASS | [log L84](hid_descriptor_probe/probe.log#L84), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_intr_msg_async | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_blk_msg | USB unavailable read_blk_msg | PASS | [log L85](hid_descriptor_probe/probe.log#L85), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_blk_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_blk_msg_async | USB unavailable read_blk_msg_async | PASS | [log L86](hid_descriptor_probe/probe.log#L86), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_blk_msg_async | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_intr_msg | USB unavailable write_intr_msg | PASS | [log L87](hid_descriptor_probe/probe.log#L87), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_intr_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_intr_msg_async | USB unavailable write_intr_msg_async | PASS | [log L88](hid_descriptor_probe/probe.log#L88), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_intr_msg_async | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_blk_msg | USB unavailable write_blk_msg | PASS | [log L89](hid_descriptor_probe/probe.log#L89), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_blk_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_blk_msg_async | USB unavailable write_blk_msg_async | PASS | [log L90](hid_descriptor_probe/probe.log#L90), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_blk_msg_async | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_ctrl_msg | USB unavailable read_ctrl_msg | PASS | [log L91](hid_descriptor_probe/probe.log#L91), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_ctrl_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_ctrl_msg | USB unavailable write_ctrl_msg | PASS | [log L92](hid_descriptor_probe/probe.log#L92), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_ctrl_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_iso_msg | USB unavailable read_iso_msg | PASS | [log L93](hid_descriptor_probe/probe.log#L93), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_iso_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_iso_msg_async | USB unavailable read_iso_msg_async | PASS | [log L94](hid_descriptor_probe/probe.log#L94), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::read_iso_msg_async | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_iso_msg | USB unavailable write_iso_msg | PASS | [log L95](hid_descriptor_probe/probe.log#L95), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_iso_msg | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_iso_msg_async | USB unavailable write_iso_msg_async | PASS | [log L96](hid_descriptor_probe/probe.log#L96), [page-003](hid_descriptor_probe/pages/page-003.png) |
| ogc-usb::write_iso_msg_async | USB rejected transfers leave buffers and callbacks untouched | PASS | [log L97](hid_descriptor_probe/probe.log#L97), [page-003](hid_descriptor_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- No USB host is open. Only unavailable-host rejection and its buffer/callback side effects are tested; successful transfers and physical hardware are not covered.
