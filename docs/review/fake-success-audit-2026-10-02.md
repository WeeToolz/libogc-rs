# Generated `fake-success` audit — 2026-10-02

The generated inventory previously classified 13 APIs as `fake-success` solely
because their Rust body contained one or more `let _ = operation()` bindings.
That source pattern is now a review signal rather than an implementation verdict.

The current libogc C source was checked for all 13 entries:

- `ogc-aram::arq::manager::init`: C `ARQM_Init` is `void` and intentionally
  ignores the `ARQM_PushData` return used to install the zero buffer.
- `ogc-aram::arq::post_request`: C `ARQ_PostRequest` is `void`; the synchronous
  queue sleep is control flow, not the API result.
- `ogc-input-sys::pad_read`: C `PAD_Read` ignores the `SI_Transfer` return when
  requesting origin data and returns the connected-channel mask.
- `ogc-ios-sys::reload_ios`: C `IOS_ReloadIOS` explicitly ignores the
  `__ES_Close` result after launch failure and returns the reload/subsystem error.
- `ogc-si-sys::init`: C `__si_init` is `void` and does not use the
  `IRQ_Request` return.
- `ogc-system-sys::sys_pre_main`: C `SYS_PreMain` is `void` and does not expose
  subsystem initialization results.
- `ogc-video-sys::wait_v_sync`: C `VIDEO_WaitVSync` is `void`; sleeping is an
  internal wait primitive.
- `wiikeyboard-sys::deinit`: C runs close/flush/deinitialize unconditionally and
  returns `1`. Rust previously propagated close/deinitialize failures and could
  skip later cleanup. It now follows C: cleanup results and stdin-device restore
  bookkeeping are intentionally not part of the public result.
- `wpad-sys::{stop_search,shutdown,scan_pads}`: C ignores the corresponding BTE
  helper results where applicable. `WPAD_StopSearch` and `WPAD_Shutdown` return
  their defined success value; `WPAD_ScanPads` returns `WPAD_ReadPending`.

`ogc-card-sys::{find_first,find_next}` exposed one real robustness issue adjacent
to the audit flag: after locating a matching directory index, Rust discarded the
`fill_dir` result and returned `CARD_ERROR_READY`. Those paths now return
`CARD_ERROR_BROKEN` if the directory table disappears before the final fill.

After this change, wildcard-discard findings no longer overwrite the declared
implementation class with `fake-success`. Without exact host/probe evidence they
remain `unverified`, and the generated CSV keeps the source-review note so the
operation can still be audited deliberately.
