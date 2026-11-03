# FAT file and mount checks in Dolphin

Dolphin 2609a ran `fat_sd_probe` with a private virtual Wii SD card. The DOL
SHA-256 was `4dc5e9d6d0c6030661ddf24031f5629fba40e4f8cabaf32a5509e48511cba7e0`.
The probe passed 39 checks with no failures. It created, wrote, truncated, and
read a temporary file; reopened it read-only and checked its length and bytes;
and verified that a write through the read-only handle failed. It also checked
the create API and removed the temporary file, then mounted the SD volume with
explicit read-only options and unmounted it. See the [probe log](probe.log).

This run does not verify physical SD cards, SD Gecko adapters, or USB media.
