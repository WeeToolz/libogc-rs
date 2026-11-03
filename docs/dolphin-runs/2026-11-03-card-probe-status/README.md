# CARD probe status diagnostic

Dolphin 2609a ran both probes with private copies of the same 16 MiB card
image, SHA-256 `f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The source image was unchanged. The Wii DOL SHA-256 was
`554d0f7983f664b4e8ccb1db4c114f0efc4d19b4dd5fa79d7cefefc56ca798b7`;
the GameCube DOL SHA-256 was
`151daa1894d054cbc9cafdedc99216ad26d4c0e02afdcff60277ae337cf7848c`.

The Wii probe passed 112 checks with zero failures. The GameCube probe
reported VI retrace timeouts and stopped while opening its deferred log, before
the final summary. Both logs show that the status preservation check passed
and the asynchronous mount request was accepted and completed with no card.
Save operations were skipped. The image was attached, but neither run proves
that the CARD implementation can mount it or persist data. Physical hardware
remains unverified. See the [Wii log](wii-probe.log) and
[GameCube log](gamecube-probe.log).
