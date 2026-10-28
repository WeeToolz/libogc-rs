# Probe logging integrity

The probe logger now keeps pending output until each backend write succeeds.
The current `DeviceFile::write_all` API does not report the accepted prefix on
failure. The logger therefore retains the full failed buffer and stops retrying
that backend for the rest of the run. The failed file can end with a partial
record. The logger does not retry it, so it does not duplicate that prefix.
The bounded queue reports its lost-byte count instead of silently treating
truncation as success. The first console, backend, capacity, or sync failure is
sticky. A requested backend that cannot open is a failure; the explicit
`Disabled` policy does not require a backend and does not create that failure.

Before the final summary, the logger opens deferred output, writes queued
records, and syncs the backend. A logger failure adds a failed check and makes
the summary `FAIL`. The logger prints the first error and lost-byte count
directly to the display, without feeding that diagnostic back into the failed
logger. The WIR2 report uses flag bit 0 for screen-transcript truncation and
bit 1 for a logger write, capacity, open, or sync failure.

The bounded state is production code in `ogc-system-probe/src/log_state.rs`.
The standalone host test package includes that same source file. Its tests
check queued-byte retention after failure, partial commit behavior, counted
capacity loss, and sticky first-error behavior. The target logger still needs
hardware checks with failed and full storage devices; the host tests do not
simulate the storage backend.
