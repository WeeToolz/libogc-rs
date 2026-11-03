# FAT read-only mount in Dolphin

Dolphin 2609a ran `fat_sd_probe` with a private virtual Wii SD card. The DOL
SHA-256 was `6ec009904313c4d9d4a4cbc803069e34c6bac00ec662b67b20d486b9067d9775`.
The probe passed 40 checks with no failures. It mounted the volume read-only,
verified that creating a file returned `ReadOnly`, checked that the file was
absent, and unmounted the volume. See the [log](probe.log) and inspected
[summary page](summary.png).

This run does not establish behavior on physical SD cards or USB media.
