# SI interrupt masking and pending dispatch in Dolphin

Dolphin 2609a ran `startup_probe` with 18 passes and zero failures. With
the SI interrupt masked, an empty-channel transfer reached the SI pending
state without calling its callback. A pending-dispatch attempt left the
masked transfer pending. After unmasking, pending dispatch completed the
transfer and delivered its callback once. The probe restored the original
interrupt mask and global interrupt state.

See `suite.json` and `startup_probe/probe.log` for the DOL hash, launch
configuration, and transcript. Physical Wii evidence remains pending.
