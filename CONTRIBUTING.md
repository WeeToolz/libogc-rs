# Contributing

Contributions, issue reports, and independent reviews are welcome. The project
is under active development. Start with [the documentation guide](docs/README.md)
and [the implementation status](docs/implementation-status.md) to find open
work and its evidence limits.

For a bug report, include the target, steps to reproduce, expected and actual
results, and relevant logs. For a change, explain the behavior, the checks you
ran, and any untested conditions. Keep changes focused and preserve useful error
details. The root README lists build and test commands.

PowerPC runtime checks use DOL probes in Dolphin. See [the probe guide](probes/README.md).
Dolphin evidence does not establish behavior on physical hardware. Before Wii
hardware work, follow [the hardware validation guide](docs/wii-hardware-validation.md)
and the mandatory instructions in [AGENTS.md](AGENTS.md).
The local workspace hardware guide referenced there and Wiiload-rs are not
included in this repository. Obtain the applicable guide and tools before
physical diagnostic work. Some historical scripts use a specific local
network setup; verify their settings before use.

See [license and source notices](docs/source-licenses.md) before contributing
code or assets from another project. Retain the applicable notices and identify
the source and changes.
