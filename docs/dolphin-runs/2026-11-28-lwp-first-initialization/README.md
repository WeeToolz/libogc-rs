# LWP first initialization

Date: 2026-11-28 UTC. Dolphin 2609a. The `lwp_probe` passed 284 checks and
reported no failures. The DOL SHA-256 is
`b7d71c58bbc7f03344e6f90f81ebff6fcf9f7745f47dfe5d104310b0f3dd98ef`.

Before any probe call that can initialize LWP, the probe drained the retained
scheduler diagnostic queue and found the initialization event from system
startup. It then called the public `lwp::initialize` entry point and confirmed
that the startup thread remained current. This checks first-call startup and
repeat behavior in Dolphin.

The runner used a private Dolphin user profile and its copied NAND. Physical
Wii scheduler and interrupt behavior remain open.

To repeat, run `cargo xtask dolphin-validate --probe lwp_probe` with the local
Dolphin path and artifact directory.
