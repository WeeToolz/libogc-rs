# Report protocol

This allocation-free `no_std` crate owns the common report encoding and field
parsing used by HBC startup diagnostics and the shared development runtime.
It can run on the host and on PowerPC. It does not start workers, open sockets,
arm recovery, or require the development build mode.

Applications use `ProgressHeader::encode`, `parse_report_endpoint`, and
`parse_recovery_deadline`. The endpoint parser checks IPv4 syntax and a nonzero
port. Each caller applies its own address policy. The header encoder writes
the supplied fixed-width fields; callers must check their payload bounds.

The development runtime also uses `parse`, `Identity`, `LogBuffer`, and the
development report header functions. Its parser requires a recovery deadline,
rejects duplicate identity options, and restricts run IDs. HBC retains its
existing optional deadline and application-specific argument rules.

Run the shared tests from the SDK root:

```sh
cargo test -p ogc-report-protocol --offline
```
