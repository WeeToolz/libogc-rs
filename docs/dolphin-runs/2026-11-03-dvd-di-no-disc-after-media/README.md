# DVD no-disc regression after DI media probe

Dolphin 2609a ran the updated `dvd_probe` in Wii mode with no disc. The no-disc
branch ran the DVD and direct DI error checks and passed 178 checks with no
failures. This confirms that the new inserted-media branch does not bypass
the no-disc checks.

The [probe log](probe.log) is from this run. The DOL SHA-256 is
`5c140ab89fb347eae8657a3201cf7ccac8e36d333c484f42849cb5f100304134`.
