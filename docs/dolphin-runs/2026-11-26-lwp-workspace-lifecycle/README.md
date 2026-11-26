# LWP workspace lifecycle in Dolphin

Dolphin 2609a ran `runtime_probe` with 880 passes and zero failures. The
probe wrapped the registered production workspace allocator, created a
thread with a workspace-backed stack, joined it, and checked that the same
allocation was freed. It then rejected a stack allocation, checked that
thread creation failed, and created and joined another thread. The probe
checked three allocation calls, two frees, and restoration of the original
allocator callbacks.

The injected failure occurred before a stack allocation was returned. It
therefore checks rejection and slot recovery, not cleanup after a later
failure. See `suite.json` and `runtime_probe/probe.log` for the DOL hash,
configuration, and full transcript. Physical Wii evidence remains pending.
