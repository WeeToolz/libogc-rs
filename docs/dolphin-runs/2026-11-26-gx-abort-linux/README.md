# Wii GX abort and FIFO recovery — Linux Dolphin 2609b

C reference: `7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`.
Local Rust baseline: `8e471178` (external IRQ suspension correction).
Stock graphical Dolphin 2609b, OpenGL, private stock-menu NAND. No
Dolphin changes were used.

## Source comparison

C `libogc/gx.c` has separate Wii and GameCube abort entry points. On Wii,
`__GX_Abort` (lines 268–305) waits for the stable memory-interface counter
when GX and its GP FIFO are initialized. It asserts PI reset, waits 50
raw timebase ticks, releases reset, and waits five more. The common
`__GX_CleanGPFifo` (340–384) masks interrupts, stops CP reads, clears the
FIFO distance and resets the saved pointers, restores the linked CPU
write pointer and high-only watermark interrupt, clears the breakpoint
callback, resets CP status, and finally enables reads. On Wii,
`GX_AbortFrame` (1638–1647) also calls `__GX_InitRevBits` (238–255),
clears dirty state, and flushes. The GameCube path does not submit these
revision commands after abort.

The Rust baseline already reset PI and discarded the FIFO distance, but
omitted the Wii memory-counter wait and revision commands. It did not
protect the FIFO cleanup or explicitly sequence its interrupt masks.
The corrected implementation follows the C cleanup sequence, shares the
revision initializer with startup, sets the VAT0 and VAT1 software
revision defaults, and clears Rust's separate deferred-state flags before
flushing. Clearing the independent Rust VAT dirty mask also avoids replay
of pre-abort updates when another VAT is modified. C clears the top-level
dirty state rather than explicitly zeroing its separate VATTable here.

Rust retains explicit GP FIFO base/limit restoration from its previous
implementation. Dolphin `ProcessorInterface.cpp` invokes
`CommandProcessorManager::ResetFifo()` on PI reset; that implementation
sets CP control to 0x10 and resets the watermarks. C cleanup does not
rewrite the base/limit registers. Preserving the Rust writes restores the
configured limits in this emulator without changing Dolphin. Physical
Wii register-reset behavior remains pending. The Dolphin memory-interface
registers are stored values, not a cycle-accurate model of pixel-engine
counter progress; passing the new wait is not physical timing evidence.

## Regression method

The new focused test uses a separate aligned 64 KiB FIFO, a real GP
breakpoint, and three 32-byte NOP bursts. Before abort it observes one
breakpoint callback and 96 bytes in CP's pending-distance register. It
then checks the linked CP control (0x15), callback removal, the exact
71-byte Wii revision command sequence in FIFO memory, and fresh PE
completion with an empty FIFO. The eight VAT1 loads occupy 48 bytes, two
XF loads occupy 18 bytes, and the BP load occupies five bytes. The
extended test marks a VAT update pending without emitting it and checks
that bytes 71–95 are NOP padding rather than replayed dirty commands.
The general abort occupancy check now waits for PE completion because
Wii abort itself queues revision commands.

The initial baseline is 48/50 (two failed revision-command checks, six
valid capture pages); the identical probe with the correction is 50/50
(five valid pages). The extended focused probe passes 51/51 (five valid
pages). The subsequent broad video run completes 404/405 with 33 valid
pages. Its sole failure is the historical exact-byte TEX4 expectation
`0x38000000` for VAT1 format 7. C's `__GX_InitRevBits` sets bit 31 and
`__SETVCDFMT` (2590–2596) preserves it with masks `0x08000000` and
`0x70000000`; the independently derived correct word is `0xB8000000`.
Update the expected byte, not the production register state.

Scope: linked FIFO recovery with a stopped GP and no blocked producer,
revision command bytes and a pending VAT update. Abort while a producer
is suspended, detached FIFO configurations, outstanding PE callbacks,
nonzero saved write-pointer/wrap cases, GameCube runtime, physical Wii
counter progress and sticky CP clearing remain open. The GX package
remains `needs_re_review`.

## Final verification

After correcting the C-derived VAT1 expectation, the stock graphical
Dolphin video suite passes 405/405 (32 valid capture pages), and the
focused GX suite passes 51/51 (five valid pages). The earlier failing
video run is preserved, as is the interrupted attempt without a final
report. Host GX tests pass 5/5 and xtask tests pass 164/164. Wii
hardware-group compilation passes for six packages. The 3,419-entry
implementation inventory is current. The private evidence archive hash
and per-run DOL hashes are in `summary.json`. Raw reports, guest logs,
transport bytes and captured pages remain in that private archive; NAND
and user profiles are excluded. Final DOLs are included there; the earlier
DOLs were replaced by subsequent builds and only their hashes remain.
