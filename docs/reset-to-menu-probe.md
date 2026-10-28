# Reset to System Menu diagnostic DOL

`reset_to_menu_probe` is a small application that initializes the system and
console, sends the existing Wiiload diagnostic report, then calls the production
`ogc_system::reset_system(SYS_RETURNTOMENU, 0, 0)` path. It has no HBC browser,
app scan, controller service, or file log. It uses `ProbeLoggerOptions::disabled`
so the logger does not open SD, USB, or NAND files.

The report proves DOL entry and initialization. It states that reset completion
is unconfirmed. After the report returns, including a report failure, the DOL
requests the System Menu reset. A reset that unexpectedly returns records a
failure and stops; it does not return through the HBC reload stub.

The shared report helper has a new `summary_before_explicit_exit` entry point.
It sends the normal summary while leaving exit selection to this DOL. This
prevents an upload's `--restart-after-probe` argument from returning to HBC
before the DOL can request the System Menu. Other probes keep the existing
`summary` behavior.

Build from `RustPorts/libogc-rs`:

```sh
cargo xtask build-dol ogc-system-probe --bin reset_to_menu_probe \
  --probe-features agent-report \
  --output target/xtask/probes-agent/reset_to_menu_probe.dol
```

The repository root `run-reset-to-menu.command` uses the existing
`run-wii-probe.command` runner. It supplies host 10.0.0.139 and Wii 10.0.0.242.
It disables the optional recovery reset and return-to-HBC options because this
DOL performs its own explicit System Menu reset. Launch the command through
the signed-in Terminal app, as the existing hardware workflow requires.

## Evidence, 2026-10-18

The built DOL is 402,624 bytes. SHA-256:
`cc314d089a038baf16ffbca388b1098be7a0062ea3a06b9d3c3316eb7c0a4d09`.

An isolated Dolphin run showed DOL entry, the explicit System Menu reset
request, the production reset callbacks, and return-to-menu result -4100 in
an empty test NAND. This is evidence of reaching the reset API, not of the
System Menu loading. The log is in the test profile:
`/var/folders/lj/9_lkf2111h7g0310hnkh5m2w0000gn/T/wii-reset-probe-us0c6zk2/Logs/dolphin.log`.
No Wiiload report identity was supplied in this direct emulator boot, so the
network report was not validated in that run.

The real-Wii attempt used run ID `1792331594-309782000`. Its report listener
opened on the Mac, but the Wii refused TCP port 4299 throughout the five-second
readiness wait. No data was uploaded; DOL entry and reset were not tested on
the Wii. The result is saved at:
`target/wii-runs/1792331594-309782000-10.0.0.242/result.json`.

The source audit passes. The completeness audit still stops at the existing
stale `src/lib.rs:840 mp3player-sys::init` implementation evidence.

### Reference Wiiload retry through Terminal

The probe was retried through the devkitPro reference sender after launching the
`.command` file in signed-in Terminal. The command used `WIILOAD=tcp:10.0.0.242`
and `target/xtask/probes-agent/reset_to_menu_probe.dol` (SHA-256
`cc314d089a038baf16ffbca388b1098be7a0062ea3a06b9d3c3316eb7c0a4d09`).
Reference Wiiload returned status 1 with `Connection refused` on TCP 4299.
No DOL data was transferred. The result and command output are
`target/send-reset-reference.status` and `target/send-reset-reference.log`.
