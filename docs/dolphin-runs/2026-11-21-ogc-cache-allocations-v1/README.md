# Initial locked-cache allocation attempt

This exploratory run passed 60 bus checks. It tested `lc_alloc_tags` with one
non-zero tag and checked the adjacent lines. Review showed that one tag did
not test the loop's 32-byte stepping behavior. The probe was expanded to two
adjacent tags and a two-line `lc_alloc` request in the later
[accepted allocation batch](../2026-11-21-ogc-cache-allocations-v3/README.md).

This run was not recorded as API evidence. Keep it as a record of the narrower
check that led to the expanded coverage.
