# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 78. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::open_content | ES active content rejects absent title | PASS | [log L110](general_probe/probe.log#L110), [page-004](general_probe/pages/page-004.png) |
| ogc-es-sys::open_title_content | ES title content opens and rejects null view | PASS | [log L111](general_probe/probe.log#L111), [page-004](general_probe/pages/page-004.png) |
| ogc-es-sys::open_title_content | ES title content reads fixture after seek and closes | PASS | [log L112](general_probe/probe.log#L112), [page-004](general_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Uses system-menu content and a ticket view from the private Dolphin NAND. Active-title success, other content, ticket permissions, and physical hardware remain open.
