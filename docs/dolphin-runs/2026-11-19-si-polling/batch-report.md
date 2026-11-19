# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 531. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si::set_xy | SI wrapper sets polling coordinates | PASS | [log L279](runtime_probe/probe.log#L279), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::set_xy | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::enable_polling | SI wrapper enables and disables polling | PASS | [log L281](runtime_probe/probe.log#L281), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::enable_polling | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::disable_polling | SI wrapper enables and disables polling | PASS | [log L281](runtime_probe/probe.log#L281), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::disable_polling | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::set_command | SI wrapper writes channel command | PASS | [log L283](runtime_probe/probe.log#L283), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::set_command | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::set_xy | SI raw sets polling coordinates | PASS | [log L280](runtime_probe/probe.log#L280), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::set_xy | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::enable_polling | SI raw enables and disables polling | PASS | [log L282](runtime_probe/probe.log#L282), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::enable_polling | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::disable_polling | SI raw enables and disables polling | PASS | [log L282](runtime_probe/probe.log#L282), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::disable_polling | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::set_command | SI raw writes channel command | PASS | [log L284](runtime_probe/probe.log#L284), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::set_command | SI polling and command registers restored | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin SI polling register transitions and channel 3 command register only. Controller response, sampling timing, and physical Wii remain unverified.
