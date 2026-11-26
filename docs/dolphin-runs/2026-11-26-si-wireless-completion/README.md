# SI wireless ID completion in Dolphin

Dolphin 2609a ran `runtime_probe` with 878 passes and zero failures. The
probe used controlled SI transfer completions. It checked that channel 2
discovery calls the registered SRAM getter and setter with ID `0x5231`, waits
for the wireless handshake, and calls the queued type callback once. The
probe restored the SRAM ID and previous accessor registration.

See `suite.json` and `runtime_probe/probe.log` for the DOL hash, launch
configuration, and full results. This run does not verify a wireless receiver
or interrupt delivery on physical Wii hardware.
