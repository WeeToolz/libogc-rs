# Wireless scan diagnostic in Dolphin, 2026-10-31

HBC's UI acceptance result recorded a scan error and a return to a listening
receiver. It did not record the libogc failure stage or code. A diagnostic
runtime probe called `ogc_wd::scan_once_detailed_result` with default settings
and a 256-byte sentinel buffer. Its test expected `DriverOpen` and `-6` based
on the earlier driver-open hypothesis. The check failed. It recorded stage 5
(`NetworkDriverUnlock`) and code `-1`; the final probe summary was 32 passes
and one failure. This result does not prove the scan output or buffer contents,
because the combined check stopped after the unexpected error comparison.

The diagnostic patch is retained with the [run evidence](evidence/wd-scan-dolphin-2026-10-31/diagnostic.patch).
The test was removed from the standard probe because its expected stage was not
established. The Dolphin result does not establish physical Wii behavior.

Source review found that `scan_once_detailed` attempts unlock after driver-open
failure but discards its result. After a scan error, it reports the scan error
before close or unlock errors. Thus this API cannot report all cleanup failures
when failures occur together. A focused failure-injection check and a decision
on error representation are still required. The HBC result cannot prove that
unlock succeeded.

## Repeated scan check

A follow-up Dolphin probe called the production API twice. The first call
returned `NetworkDriverUnlock`, code `-1`. The second returned
`NetworkDriverLock`, code `-4`, so the repeat check failed. The result is
consistent with a lock left held, but does not by itself prove the lock state
or identify the cause of Dolphin's unlock failure. The check and both runs are
retained under `evidence/wd-scan-repeat-detail-dolphin-2026-10-31/` and
`evidence/wd-scan-repeat-dolphin-2026-10-31/`. The final run reported 33 passes
and one failure. The failing diagnostic patch was not added to the standard
probe. Cleanup failure injection and physical Wii behavior remain open.

## NCD handle fix — 2026-11-02

The current failure was in the NCD lock and unlock IOCTLV buffers. Dolphin's
`Manage.cpp` writes the operation result to word 0 and the lock handle to word
1 of the lock output. Unlock reads the handle from word 0 of its input and
writes its result to word 0 of its output. The Rust code read the result as the
handle and sent the handle in the output buffer. This made the first scan
report unlock failure `-1`; the second scan then failed to acquire the held
lock with `-4`.

`ogc-wd` now reads the handle from the second big-endian word and sends it in a
separate four-byte input buffer to unlock. It checks the lock operation result
before returning the handle. The network probe calls the production scan API
twice after it stops the socket stack. It checks the returned count against the
buffer capacity on each call.

The regression check failed before the fix in Dolphin: the first scan reported
stage 5/code `-1`, and the second reported stage 1/code `-4`. After the fix,
both scans passed and each returned one valid result. Evidence is in
`evidence/wd-scan-red-2026-11-02/` and
`evidence/wd-scan-verified-2026-11-02/`. The complete network probe still
reported six socket-open failures, which also appeared in the earlier
2026-11-02 network baseline. Those failures are outside this wireless scan
fix.

This is Dolphin-only evidence. A Wii hardware scan remains untested.
