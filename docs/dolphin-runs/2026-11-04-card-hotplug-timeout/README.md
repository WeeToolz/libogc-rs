# Card hotplug probe: no removal event

Dolphin 2609a ran the opt-in GameCube card probe with a private card copy.
The [log](probe.log) records 10 passes and one failure: no detach callback
arrived during the wait. Initial card detection, mount, file open, and timeout
cleanup passed. No removal was requested in this run. The [suite report](suite.json)
records the DOL hash and launch configuration. This run does not test removal,
stale handles, reinsertion, or remount.

A live slot-change control is still required in the existing CLI runner. The
probe and reproduction options are described in `probes/README.md`.
