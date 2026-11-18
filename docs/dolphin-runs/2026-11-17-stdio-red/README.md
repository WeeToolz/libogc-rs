# Standard stream redirection failure

The callback probe saves the standard stream device table, clears the stdout
and stderr entries, and calls `stdio_report(false)` and
`stdio_report(true)`. It checks for a writable stderr device after the
first call and writable stdout and stderr devices after the second call. It
restores both table entries before reporting the result.

Dolphin reported the check as a failure. The target implementation only
stores the requested flag. The reference implementation installs the UART
device on stderr and optionally stdout.

The Wii Rust standard-library overlay uses one output ABI for stdout and
stderr. A complete correction must distinguish those streams through the
runtime and deliver bytes to the real UART path. It must preserve callers of
the existing output ABI and propagate transport failures. The failing check
does not yet test stream bytes, buffering, or physical hardware.
