# Network getsockopt experiment

The previous standard and development DOL runs both reported the same eight
network failures, so the development wrapper is not shown to cause them.

An experimental IOS `getsockopt` implementation used ioctl 8 and an aligned
36-byte output request, matching the request layout in the existing std
network service. The [Dolphin log](probe.log) records 44 passes and seven
failures: `SO_RCVBUF` changed from fail to pass, while five option checks,
connect, and bind error conversion still failed. The local IP is redacted.
The experimental implementation was reverted. It needs option-specific return
and byte diagnostics, bounds and failure tests, and confirmation of the IOS
contract before it can be accepted. No network failure is marked fixed here.
