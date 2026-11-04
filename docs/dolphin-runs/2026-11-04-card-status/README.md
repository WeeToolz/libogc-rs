# CARD synchronous metadata setters

Dolphin 2609a ran the new `set_status` and `set_status_ex` checks on private
copies of a virtual 16 MiB card. Each check changes a metadata value, reads it
back, restores the original value, and checks the restored value. The
[GameCube log](card-probe.log) records 204 passes and zero failures; the
[Wii log](storage-probe.log) records 325 passes and zero failures. The
[suite report](suite.json) contains the DOL hashes and runner results. Card
format and hotplug were not part of this run. These results do not establish
physical card behavior.
