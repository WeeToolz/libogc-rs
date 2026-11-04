# Network socket option diagnostics

An experimental IOS getsockopt implementation and temporary option logging
ran on Dolphin. The [log](probe.log) records 44 passes and seven failures. The
local IP is redacted. The experiment was reverted; the committed baseline has
eight failures.

For `SO_REUSEADDR`, set and get returned zero, but the returned integer was
67108864 after setting 1. IP TTL set and get both returned -1; TCP_NODELAY
set returned zero and get returned -1; TCP_KEEPALIVE set returned -1 and get
returned zero with 131072. The timeout option, connect error, and bind error
also failed. These observations do not establish whether byte order, option
mapping, or Dolphin support causes each result. The temporary implementation
and diagnostic probe must not be treated as a verified fix.
