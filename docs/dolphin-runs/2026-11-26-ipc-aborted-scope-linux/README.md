# Aborted IOS replacement scope — stock Linux Dolphin 2609b

Each of three reload iterations opens a real independent STM hook and
submits an asynchronous request. A duplicate hook request establishes that
IOS still holds the first request. Acquire the replacement scope, request an
absent device, observe failure, and drop the scope without calling `finish`.
A subsequent duplicate hook request still returns EEXIST and the old callback
count remains zero. Actual IOS replacement then exercises existing cancellation
and resource recovery checks. Final result: 108/108, ten captured pages.

This tests an aborted replacement scope with a rejected IPC operation. It is
not an actual failed IOS launch or failed post-reload service handshake. No
production IPC changes were made. Raw evidence is retained privately.

Private evidence SHA-256: `09cec7380e8670b575a5f160a9768626cf1cf8f24a20fc3841a08bb75bff88e8`.
