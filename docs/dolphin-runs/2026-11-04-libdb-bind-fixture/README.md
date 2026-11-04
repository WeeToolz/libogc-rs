# Debugger bind fixture

The probe now allocates two handles with `tcpip_socket`, binds the first, and
checks that the second bind to the same port returns `ERR_USE` (-10). The
[Dolphin log](probe.log) records those checks as passes. The complete network
probe records 43 passes and eight failures; the socket-option, connect, and IOS
bind-error checks remain open. `tcpip_close` has no return value, and bound
state cleanup after close is not verified. The local IP is redacted.

The requested development DOL was built with `--dev --probe-features
dolphin-automation`; its SHA-256 was
`8a3796fd1507a2fa669cc79fc227a2a8bfd5c7a77aa009c63281bfcea02c83c0`.
Running that DOL in the existing Dolphin runner reported `crash` before a
probe result. The passing fixture checks above used the standard automation
DOL, not the development DOL. Debug-wrapper startup remains unverified.
