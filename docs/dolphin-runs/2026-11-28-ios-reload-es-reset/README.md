# IOS reload resets and reopens ES

Date: 2026-11-28 UTC. Dolphin 2609a. The probe passed 144 checks and reported
no failures. The DOL SHA-256 is
`44bd9b3316fe710acec5aeb325b2171f954bcf396e56680a38f62a6cb402c426`.

The `ios_reload_probe` completed three IOS replacements. A probe-only check
queried ES immediately after the reset point and received `ES_ENOTINIT` each
time. After `reload_ios` completed its subsystem initialization, ES returned a
nonzero title count on the new IOS. The probe log records both results for all
three replacements.

The runner used a private Dolphin user profile and its copied NAND. This run
checks the Dolphin IOS path. Physical Wii reload behavior remains open.

To repeat, run `cargo xtask dolphin-validate --probe ios_reload_probe` with the
Dolphin path, NAND source, and artifact directory set for the local machine.
