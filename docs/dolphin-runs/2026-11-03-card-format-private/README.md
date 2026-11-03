# Private memory-card format in Dolphin

Dolphin 2609a ran `storage_probe` with `--format-private-card` and a private
copy of the verified 16 MiB card image. The runner supplied the probe's
destructive-format argument through Dolphin's debugger. The run used the
existing DOL and the runner changes based on revision `d43738e`.

The [probe log](probe.log) reports 226 passes and zero failures. The format
checks report successful mount, format, unmount, remount, free-block count,
and final unmount. The source card SHA-256 before and after the run was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The DOL SHA-256 was
`33585efa230e3f00aebf00ac0937c4526a2e62f75d5513ecdb5efe3ea5fc7e2a`.

The run did not create a marker file immediately before formatting. It does
not prove that formatting removes existing save data. Card removal and
physical-card behavior also remain unverified.
