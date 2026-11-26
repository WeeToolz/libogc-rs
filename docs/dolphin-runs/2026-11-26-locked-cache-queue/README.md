# Locked cache queue in Dolphin

Dolphin 2609a ran `bus_probe` with 78 passes and zero failures. The probe
sampled queue depth immediately after a one-line DMA submission and read zero.
The DMA data and empty queue after flush passed. Nonzero queue depth remains
untested, and physical Wii evidence remains pending. See `suite.json` and
`bus_probe/probe.log` for the DOL identity, configuration, and transcript.
