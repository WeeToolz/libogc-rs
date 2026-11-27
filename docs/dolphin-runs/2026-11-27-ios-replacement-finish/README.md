# IOS replacement cleanup follow-up

Date: 2026-11-27. Dolphin 2609a on macOS. The IOS lifecycle probe ran after
`launch_ios_from_es` changed to finish the replacement before returning a
version mismatch.

The normal IOS 58 reload passed 112 checks with zero failures. The probe DOL
SHA-256 was
`3e366fd761e8ee1652ac7a87c90aa5ec127e3c309e5d0ae91e5020c4f3e418d4`.
The probe confirmed successful reload and cleanup of held old requests. It
cannot exercise an unexpected-version launch.

PowerPC target tests compile checks cover cleanup ordering for matching and
mismatching version results. They did not run on the PowerPC target. No
physical Wii check was performed. See `../../review/libios-timing-cache-stm-audit.md`
for the source change and remaining limits.
