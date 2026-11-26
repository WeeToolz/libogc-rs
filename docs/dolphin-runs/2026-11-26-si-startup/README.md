# SI startup and interrupt delivery in Dolphin

Dolphin 2609a ran `startup_probe` with 16 passes and zero failures. The
startup record showed that SI initialization completed. The type callback
reported the attached GameCube controller on channel 0. A separate transfer
on empty channel 3 completed through the SI interrupt and reported no
response. The probe checked the callback channel, result, and idle state.

The run used the standard Dolphin runner and its private Wii profile. See
`suite.json` and `startup_probe/probe.log` for the DOL hash, configuration,
and full transcript. Physical Wii evidence remains pending.
