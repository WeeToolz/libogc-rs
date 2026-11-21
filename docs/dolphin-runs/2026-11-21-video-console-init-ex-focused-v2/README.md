# Extended console initialization check

The batch calls `ogc_video::console_init_ex` twice with the active preferred
mode and framebuffer available. It checks that the default console has a live
buffer, that its dimensions and byte strides match the requested mode, that a
second same-size initialization reuses the same buffer, and that the retrace
callback is installed. It then restores the direct framebuffer console and
the prior callback.

Dolphin 2609a passed 750 runtime checks with zero failures. The focused check
is in [batch-report.md](batch-report.md), at
[`runtime_probe/probe.log`, line 420](runtime_probe/probe.log#L420). The
captured screen is [page 17](runtime_probe/pages/page-017.png).

The inventory changed from 193 to 192 unverified APIs. The generated audit
still lists 192 other APIs. This run verifies the valid initialized-console
path in Dolphin; it does not prove behavior on a physical Wii. `console_init`
and `ogc-video-sys::set_next_right_framebuffer` remain open.
