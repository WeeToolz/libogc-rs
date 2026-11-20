# Dolphin evidence report

Probe `gx_api_probe` reported `pass`.
Passed checks 22. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::abort_frame | GX abort frame empties pending GP FIFO work | PASS | [log L99](gx_api_probe/probe.log#L99), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::clear_gp_metric | GX GP metric clear resets counter zero and leaves unsupported counter one untouched | PASS | [log L96](gx_api_probe/probe.log#L96), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::disable_break_pt | GX breakpoint disable clears status | PASS | [log L88](gx_api_probe/probe.log#L88), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::enable_break_pt | GX null breakpoint enable leaves status unchanged | PASS | [log L87](gx_api_probe/probe.log#L87), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::init | GX init rejects null and misaligned FIFO storage | PASS | [log L86](gx_api_probe/probe.log#L86), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::read_clks_per_vtx | GX clocks per vertex query returns its documented byte range | PASS | [log L97](gx_api_probe/probe.log#L97), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::redirect_write_gather_pipe | GX write gather redirect selects its aperture | PASS | [log L92](gx_api_probe/probe.log#L92), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::restore_write_gather_pipe | GX write gather restore keeps the CPU FIFO live | PASS | [log L93](gx_api_probe/probe.log#L93), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::set_cpu_fifo | GX CPU FIFO setter preserves the active FIFO | PASS | [log L90](gx_api_probe/probe.log#L90), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::set_gp_fifo | GX GP FIFO setter preserves the active FIFO | PASS | [log L91](gx_api_probe/probe.log#L91), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::set_misc | GX misc DL save mode restores vertex descriptors | PASS | [log L94](gx_api_probe/probe.log#L94), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::set_misc | GX misc DL no-save mode keeps vertex descriptor changes | PASS | [log L95](gx_api_probe/probe.log#L95), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::set_v_cache_metric | GX vertex cache metric setter preserves the current counters | PASS | [log L98](gx_api_probe/probe.log#L98), [page-003](gx_api_probe/pages/page-003.png) |
| ogc-gx::tpl_open_tpl_from_file | GX TPL file open handles null inputs | PASS | [log L89](gx_api_probe/probe.log#L89), [page-003](gx_api_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
