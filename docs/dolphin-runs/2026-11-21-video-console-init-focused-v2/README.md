# Default console initialization check

The batch calls `ogc_video::console_init` twice in the runtime probe. It checks
that the console and framebuffer are initialized, that a repeated call keeps
the same console and framebuffer, and that the probe's original display is
restored afterward.

Dolphin 2609a passed 751 runtime checks with zero failures. The focused check
is in [batch-report.md](batch-report.md), at
[`runtime_probe/probe.log`, line 422](runtime_probe/probe.log#L422). The
captured screen is [page 17](runtime_probe/pages/page-017.png).

The inventory changed from 192 to 191 unverified APIs. This checks the valid
initialization path in Dolphin; it does not prove behavior on a physical Wii.
`ogc-video-sys::set_next_right_framebuffer` remains unverified.

An initial run through `video_probe` passed this selected check but failed five
other GX checks in that probe. The accepted batch uses the passing
`runtime_probe` and is stored in this directory.
