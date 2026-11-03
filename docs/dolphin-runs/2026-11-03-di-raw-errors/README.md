# DI raw read errors in Dolphin

Dolphin 2609a ran `dvd_probe` without a disc. The DOL SHA-256 was
`7efb6a1125a28c41b63d44c5353a9535d8acf8f54ce40573fc8df819ef4c5a41`.
The probe passed 120 checks with no failures. It called the raw DI read API
and checked null and unaligned buffer errors. The unaligned check verified
that the output bytes were unchanged. After closing DI, it checked that a read
returned the missing-descriptor error. See the [probe log](probe.log).

This run does not verify successful disc reads, partition access, disc contents,
or physical Wii behavior.
