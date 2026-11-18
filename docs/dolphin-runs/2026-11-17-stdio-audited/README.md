# Standard stream redirection

The initial [Dolphin probe](../2026-11-17-stdio-red/batch-report.md) showed
that standard stream device entries did not change. The corrected probe
installs stderr first, then stdout. It checks the device callbacks, an invalid
buffer, and retained stdout redirection. The [OSReport log](callback_probe/dolphin.log)
contains both fixed UART markers.

The [Wii std probe](../2026-11-17-stdio-wii-std-rebuilt/wii_std_fs_probe/probe.log)
uses the rebuilt standard library. Its [Dolphin output](../2026-11-17-stdio-wii-std-rebuilt/wii_std_fs_probe/stderr.log)
contains the stderr-only marker and the two markers after stdout is enabled.
The console-only stdout marker is visible in the captured page and is absent
from the OSReport log. The std probe reported 100 passes.

The reference UART writer reports a consumed length when UART is unavailable.
The Rust writer returns an I/O error in that case so callers can detect that no
output was delivered. UART transport failure injection, pending buffered output
at redirection, and physical hardware remain open.
