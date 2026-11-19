# Numeric resolver session

The previous run resolved `127.0.0.1`, then timed out before UDP bind completed.
Its Dolphin log records two opens of `/dev/net/ip/top` for the resolver before
the socket operations. The Rust resolver opened and closed that extra handle.
The C reference in `libogc/network_wii.c` uses the initialized network session
handle instead. The fix uses that session and does not close it.

This run resolves the same address twice, completes the UDP and TCP checks,
and confirms that a lookup after network deinitialization returns null. The
before and after runs use different DOL builds, and the change to handle use
is the intended difference. The result supports the session fix on this Dolphin
configuration. It does not prove external DNS, concurrent lookups, concurrent
deinitialization, or physical Wii behavior. The resolver uses shared response
storage, and callers must account for its lifetime and concurrency limits.
