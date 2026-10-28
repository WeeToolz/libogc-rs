#![no_std]

//! Coalescing allocator for a reserved range of MEM2.
//!
//! The allocator owns only the range passed to `initialize_region`. The Wii
//! initializer asks its caller to advance arena2's low boundary after the
//! allocator is ready. It leaves the requested space at the high end for
//! other runtime users.

use core::cell::UnsafeCell;
use core::cmp;
#[cfg(target_arch = "powerpc")]
use core::ffi::c_void;
use core::ptr::{self, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_irq_sys::{k_irq_disable_all, k_irq_restore_all};

const BLOCK_ALIGNMENT: usize = 16;
const BLOCK_ALIGN_MASK: usize = BLOCK_ALIGNMENT - 1;

#[repr(C, align(16))]
struct Block {
    size: usize,
    previous_size: usize,
    next_free: *mut Block,
    previous_free: *mut Block,
    is_free: bool,
}

const HEADER_SIZE: usize = core::mem::size_of::<Block>();
const MIN_BLOCK_SIZE: usize = HEADER_SIZE + BLOCK_ALIGNMENT;

struct State {
    start: usize,
    end: usize,
    free_head: *mut Block,
    initialized: bool,
}

impl State {
    const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            free_head: null_mut(),
            initialized: false,
        }
    }
}

/// A bounded free-list allocator. It coalesces adjacent free blocks.
pub struct ArenaAllocator {
    locked: AtomicBool,
    state: UnsafeCell<State>,
}

// State access is serialized by `locked`.
unsafe impl Sync for ArenaAllocator {}

impl ArenaAllocator {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            state: UnsafeCell::new(State::new()),
        }
    }

    /// Start using an exclusively owned memory range.
    ///
    /// # Safety
    /// For the first initialization, `start..start + len` must be writable memory,
    /// is not used by another allocator or runtime service, and remains valid
    /// for the lifetime of this allocator. Later calls return AlreadyInitialized
    /// without accessing the supplied region or changing live allocations.
    pub unsafe fn initialize_region(&self, start: *mut u8, len: usize) -> Result<(), InitError> {
        let _guard = self.lock();
        // SAFETY: The allocator state is protected by the lock.
        if unsafe { (*self.state.get()).initialized } {
            return Err(InitError::AlreadyInitialized);
        }
        let start_addr = start as usize;
        let aligned_start = start_addr
            .checked_next_multiple_of(BLOCK_ALIGNMENT)
            .ok_or(InitError::AddressOverflow)?;
        let end_addr = start_addr
            .checked_add(len)
            .ok_or(InitError::AddressOverflow)?;
        let end = end_addr & !BLOCK_ALIGN_MASK;
        let usable_len = end.checked_sub(aligned_start).ok_or(InitError::TooSmall)?;
        if start.is_null() || usable_len < MIN_BLOCK_SIZE {
            return Err(InitError::TooSmall);
        }

        let first = aligned_start as *mut Block;
        // SAFETY: The caller guarantees exclusive writable ownership of the
        // range. The header fits because usable_len >= MIN_BLOCK_SIZE.
        unsafe {
            ptr::write(
                first,
                Block {
                    size: usable_len,
                    previous_size: 0,
                    next_free: null_mut(),
                    previous_free: null_mut(),
                    is_free: true,
                },
            );
            let state = &mut *self.state.get();
            state.start = aligned_start;
            state.end = end;
            state.free_head = first;
            state.initialized = true;
        }
        Ok(())
    }

    /// Allocate memory with the requested alignment. This is exposed to test
    /// allocator behavior on hosts without creating a host global allocator.
    pub fn allocate(&self, size: usize, alignment: usize) -> *mut u8 {
        if alignment == 0 || !alignment.is_power_of_two() {
            return null_mut();
        }
        let _guard = self.lock();
        // SAFETY: All state changes are protected by the lock.
        unsafe { self.allocate_locked(size, alignment) }
    }

    /// Allocate and clear exactly the requested payload bytes.
    pub fn allocate_zeroed(&self, size: usize, alignment: usize) -> *mut u8 {
        let allocation = self.allocate(size, alignment);
        if !allocation.is_null() {
            // SAFETY: allocate returned uniquely owned storage for size bytes.
            unsafe { allocation.write_bytes(0, size) };
        }
        allocation
    }

    /// Resize an allocation without changing its alignment.
    ///
    /// A null result leaves the original allocation and its contents intact.
    ///
    /// # Safety
    /// `pointer` must be a live allocation from this allocator. `size` and
    /// `alignment` must match its original request. `new_size` must be nonzero.
    pub unsafe fn reallocate(
        &self,
        pointer: *mut u8,
        size: usize,
        alignment: usize,
        new_size: usize,
    ) -> *mut u8 {
        if pointer.is_null()
            || new_size == 0
            || core::alloc::Layout::from_size_align(new_size, alignment).is_err()
        {
            return null_mut();
        }
        let replacement = self.allocate(new_size, alignment);
        if !replacement.is_null() {
            // SAFETY: The caller supplies a live source allocation. The new
            // allocation is separate and holds new_size bytes.
            unsafe {
                ptr::copy_nonoverlapping(pointer, replacement, cmp::min(size, new_size));
                self.deallocate(pointer, size, alignment);
            }
        }
        replacement
    }

    /// Release a pointer returned by this allocator.
    ///
    /// # Safety
    /// `pointer` must be a live pointer returned by this allocator. `size` and
    /// `alignment` must match the original allocation request.
    pub unsafe fn deallocate(&self, pointer: *mut u8, size: usize, alignment: usize) {
        if pointer.is_null() || alignment == 0 || !alignment.is_power_of_two() {
            return;
        }
        let _guard = self.lock();
        // SAFETY: The caller promises pointer/layout validity. The checks in
        // release_locked also reject pointers outside the managed range.
        unsafe { self.release_locked(pointer, size, alignment) };
    }

    /// Return the reserved region under the allocator lock.
    pub fn region(&self) -> Option<core::ops::Range<usize>> {
        let _guard = self.lock();
        // SAFETY: The lock protects all state fields.
        let state = unsafe { &*self.state.get() };
        state.initialized.then_some(state.start..state.end)
    }

    pub fn capacity(&self) -> usize {
        let _guard = self.lock();
        // SAFETY: State is protected by the lock.
        unsafe {
            let state = &*self.state.get();
            state.end.saturating_sub(state.start)
        }
    }

    /// Return the payload bytes held by live allocations.
    pub fn used_bytes(&self) -> usize {
        let _guard = self.lock();
        // SAFETY: State and all block headers are protected by the allocator lock.
        unsafe {
            let state = &*self.state.get();
            if !state.initialized {
                return 0;
            }
            let mut address = state.start;
            let mut used = 0usize;
            while address < state.end {
                let block = address as *const Block;
                let size = (*block).size;
                if size < HEADER_SIZE || address.saturating_add(size) > state.end {
                    break;
                }
                if !(*block).is_free {
                    used = used.saturating_add(size - HEADER_SIZE);
                }
                address += size;
            }
            used
        }
    }

    unsafe fn allocate_locked(&self, size: usize, alignment: usize) -> *mut u8 {
        // SAFETY: Caller holds the lock.
        let state = unsafe { &mut *self.state.get() };
        if !state.initialized {
            return null_mut();
        }
        let requested = cmp::max(size, 1);
        let align = cmp::max(alignment, BLOCK_ALIGNMENT);
        let payload_bytes = match requested.checked_next_multiple_of(BLOCK_ALIGNMENT) {
            Some(value) => value,
            None => return null_mut(),
        };
        let allocation_size = match HEADER_SIZE.checked_add(payload_bytes) {
            Some(value) => value,
            None => return null_mut(),
        };

        let mut block = state.free_head;
        while !block.is_null() {
            // SAFETY: Free-list nodes are initialized blocks inside the arena.
            let block_data = unsafe { &*block };
            let block_start = block as usize;
            let block_end = match block_start.checked_add(block_data.size) {
                Some(value) => value,
                None => return null_mut(),
            };
            let mut user = match block_start
                .saturating_add(HEADER_SIZE)
                .checked_next_multiple_of(align)
            {
                Some(value) => value,
                None => return null_mut(),
            };
            let mut allocation_start = user - HEADER_SIZE;
            let mut prefix = allocation_start - block_start;

            // Keep any leading fragment large enough to hold a block header
            // and a minimum aligned payload. Smaller padding is absorbed by
            // moving the user pointer forward by one requested alignment.
            if prefix != 0 && prefix < MIN_BLOCK_SIZE {
                user = match user.checked_add(align) {
                    Some(value) => value,
                    None => return null_mut(),
                };
                allocation_start = user - HEADER_SIZE;
                prefix = allocation_start - block_start;
            }

            let allocation_end = match allocation_start.checked_add(allocation_size) {
                Some(value) => value,
                None => return null_mut(),
            };
            if allocation_end > block_end {
                block = block_data.next_free;
                continue;
            }

            let mut allocated_size = allocation_size;
            let suffix = block_end - allocation_end;
            let split_suffix = suffix >= MIN_BLOCK_SIZE;
            if !split_suffix {
                allocated_size += suffix;
            }

            let previous_size = if prefix == 0 {
                block_data.previous_size
            } else {
                prefix
            };
            let old_next_free = block_data.next_free;
            let old_previous_free = block_data.previous_free;
            if prefix == 0 {
                unsafe { self.unlink_free(block, old_previous_free, old_next_free) };
            } else {
                // Retain the original header as the free prefix.
                unsafe {
                    (*block).size = prefix;
                    (*block).is_free = true;
                }
            }

            let allocated = allocation_start as *mut Block;
            // SAFETY: The allocation range was proven to fit the free block.
            unsafe {
                ptr::write(
                    allocated,
                    Block {
                        size: allocated_size,
                        previous_size,
                        next_free: null_mut(),
                        previous_free: null_mut(),
                        is_free: false,
                    },
                );
            }

            if split_suffix {
                let suffix_block = (allocation_start + allocated_size) as *mut Block;
                // SAFETY: suffix >= MIN_BLOCK_SIZE, so its header and payload
                // fit within the original free block.
                unsafe {
                    ptr::write(
                        suffix_block,
                        Block {
                            size: block_end - (suffix_block as usize),
                            previous_size: allocated_size,
                            next_free: null_mut(),
                            previous_free: null_mut(),
                            is_free: true,
                        },
                    );
                    let next = block_end as *mut Block;
                    if (next as usize) < state.end {
                        (*next).previous_size = (*suffix_block).size;
                    }
                    self.insert_free(suffix_block);
                }
            } else if block_end < state.end {
                // SAFETY: block_end is the start of the next physical block.
                unsafe { (*(block_end as *mut Block)).previous_size = allocated_size };
            }
            return user as *mut u8;
        }
        null_mut()
    }

    unsafe fn release_locked(&self, pointer: *mut u8, _size: usize, _alignment: usize) {
        // SAFETY: Caller holds lock and promises a valid allocation pointer.
        let state = unsafe { &mut *self.state.get() };
        if !state.initialized {
            return;
        }
        let user = pointer as usize;
        let header_addr = match user.checked_sub(HEADER_SIZE) {
            Some(value) => value,
            None => return,
        };
        if header_addr < state.start || header_addr >= state.end {
            return;
        }
        let mut block = header_addr as *mut Block;
        // SAFETY: A valid allocation points to its header. Bounds check above
        // rejects pointers outside the managed arena.
        unsafe {
            if (*block).is_free || (*block).size < HEADER_SIZE {
                return;
            }
            (*block).is_free = true;

            let block_end = (block as usize).checked_add((*block).size);
            if let Some(end) = block_end
                && end < state.end
            {
                let next = end as *mut Block;
                if (*next).is_free {
                    self.unlink_free(next, (*next).previous_free, (*next).next_free);
                    (*block).size += (*next).size;
                }
            }

            let previous_size = (*block).previous_size;
            if previous_size != 0
                && let Some(previous_addr) = (block as usize).checked_sub(previous_size)
                && previous_addr >= state.start
            {
                let previous = previous_addr as *mut Block;
                if (*previous).is_free {
                    self.unlink_free(previous, (*previous).previous_free, (*previous).next_free);
                    (*previous).size += (*block).size;
                    block = previous;
                }
            }

            let block_end = (block as usize) + (*block).size;
            if block_end < state.end {
                (*(block_end as *mut Block)).previous_size = (*block).size;
            }
            self.insert_free(block);
        }
    }

    unsafe fn unlink_free(&self, block: *mut Block, previous: *mut Block, next: *mut Block) {
        // SAFETY: Caller passes a member of this allocator's free list.
        unsafe {
            if previous.is_null() {
                (*self.state.get()).free_head = next;
            } else {
                (*previous).next_free = next;
            }
            if !next.is_null() {
                (*next).previous_free = previous;
            }
            (*block).next_free = null_mut();
            (*block).previous_free = null_mut();
        }
    }

    unsafe fn insert_free(&self, block: *mut Block) {
        // SAFETY: Caller passes a writable block inside the arena.
        unsafe {
            let head = (*self.state.get()).free_head;
            (*block).is_free = true;
            (*block).previous_free = null_mut();
            (*block).next_free = head;
            if !head.is_null() {
                (*head).previous_free = block;
            }
            (*self.state.get()).free_head = block;
        }
    }

    fn lock(&self) -> LockGuard<'_> {
        // Match libogc's heap critical sections: save and mask external IRQs
        // before acquiring the lock, so an IRQ cannot re-enter and spin here.
        let irq_state = acquire_lock_with_interrupts(&self.locked, disable_interrupts);
        LockGuard {
            lock: &self.locked,
            irq_state,
        }
    }
}

impl Default for ArenaAllocator {
    fn default() -> Self {
        Self::new()
    }
}

struct LockGuard<'a> {
    lock: &'a AtomicBool,
    irq_state: u32,
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        release_lock_and_restore(self.lock, self.irq_state, restore_interrupts);
    }
}

#[inline]
fn disable_interrupts() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        k_irq_disable_all()
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[inline]
fn restore_interrupts(state: u32) {
    #[cfg(target_arch = "powerpc")]
    k_irq_restore_all(state);
    #[cfg(not(target_arch = "powerpc"))]
    let _ = state;
}

#[inline]
fn acquire_lock_with_interrupts(lock: &AtomicBool, disable: impl FnOnce() -> u32) -> u32 {
    let irq_state = disable();
    while lock
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    irq_state
}

#[inline]
fn release_lock_and_restore(lock: &AtomicBool, irq_state: u32, restore: impl FnOnce(u32)) {
    lock.store(false, Ordering::Release);
    restore(irq_state);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitError {
    AlreadyInitialized,
    TooSmall,
    AddressOverflow,
    NotEnoughArena,
}

/// Maximum managed heap size when the caller uses the recommended default.
pub const DEFAULT_MAX_MEM2_HEAP_BYTES: usize = 16 * 1024 * 1024;
/// Leave this many bytes at the top of MEM2 by default.
pub const DEFAULT_MEM2_RESERVE_BYTES: usize = 2 * 1024 * 1024;

/// Initialize a Rust allocator and reserve its range in MEM2.
///
/// The current arena2 low boundary is the allocation start. The allocator
/// takes at most `max_heap_bytes` and leaves at least `reserve_bytes` above
/// the new low boundary for other runtime users. After the allocator is
/// ready, `set_arena2_lo` is called once with the end of the Rust heap. The
/// callback must update the shared arena2 low boundary, for example with
/// `ogc_system::set_arena2_lo`, so C and newlib allocations do not reuse the
/// Rust heap.
///
/// # Safety
/// `arena_low` and `arena_high` must describe the current arena2 bounds. Call
/// this during startup, before other code allocates from the range. The
/// `set_arena2_lo` callback must update the shared arena2 low boundary.
#[cfg(target_arch = "powerpc")]
pub unsafe fn initialize_wii(
    allocator: &ArenaAllocator,
    arena_low: *mut u8,
    arena_high: *mut u8,
    max_heap_bytes: usize,
    reserve_bytes: usize,
    set_arena2_lo: impl FnOnce(*mut c_void),
) -> Result<*mut u8, InitError> {
    let low = arena_low as usize;
    let high = arena_high as usize;
    let available = high
        .checked_sub(low)
        .and_then(|size| size.checked_sub(reserve_bytes))
        .ok_or(InitError::NotEnoughArena)?;
    let heap_size = cmp::min(available, max_heap_bytes) & !BLOCK_ALIGN_MASK;
    let end = low
        .checked_add(heap_size)
        .ok_or(InitError::AddressOverflow)?;
    if low == 0 || heap_size < MIN_BLOCK_SIZE {
        return Err(InitError::NotEnoughArena);
    }
    // SAFETY: This initializer is unsafe because the caller must choose a
    // startup point before any other code uses the range being reserved.
    unsafe { allocator.initialize_region(low as *mut u8, end - low)? };
    set_arena2_lo(end as *mut c_void);
    Ok(end as *mut u8)
}

#[cfg(target_arch = "powerpc")]
unsafe impl core::alloc::GlobalAlloc for ArenaAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.allocate(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: core::alloc::Layout) {
        // SAFETY: GlobalAlloc's caller must provide a valid pointer/layout pair.
        unsafe { self.deallocate(pointer, layout.size(), layout.align()) };
    }

    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.allocate_zeroed(layout.size(), layout.align())
    }

    unsafe fn realloc(
        &self,
        pointer: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies the live allocation and layout.
        unsafe { self.reallocate(pointer, layout.size(), layout.align(), new_size) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[repr(align(64))]
    struct TestMemory<const N: usize>([u8; N]);

    #[test]
    fn region_reports_only_the_initialized_reserved_range() {
        let allocator = ArenaAllocator::new();
        assert_eq!(allocator.region(), None);
        let mut memory = TestMemory([0; 1024]);
        let start = memory.0.as_mut_ptr() as usize;
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), 1024) }.unwrap();
        assert_eq!(allocator.region(), Some(start..start + 1024));
        let pointer = allocator.allocate(64, 32);
        assert!(!pointer.is_null());
        assert_eq!(allocator.region(), Some(start..start + 1024));
        unsafe { allocator.deallocate(pointer, 64, 32) };
        assert_eq!(allocator.region(), Some(start..start + 1024));
    }

    #[test]
    fn interrupt_state_is_saved_before_lock_and_restored_after_unlock() {
        let lock = AtomicBool::new(false);
        let saved_state = acquire_lock_with_interrupts(&lock, || {
            assert!(!lock.load(Ordering::Acquire));
            0x1234
        });
        assert!(lock.load(Ordering::Acquire));

        let restored_state = Cell::new(None);

        release_lock_and_restore(&lock, saved_state, |state| {
            assert!(!lock.load(Ordering::Acquire));
            restored_state.set(Some(state));
        });

        assert_eq!(restored_state.get(), Some(0x1234));
    }

    #[test]
    fn invalid_or_overflowing_allocations_preserve_the_arena() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 2048]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        for (size, alignment) in [(16, 0), (16, 3), (usize::MAX, 16)] {
            assert!(allocator.allocate(size, alignment).is_null());
            assert_eq!(allocator.used_bytes(), 0);
        }
        let allocation = allocator.allocate(17, 64);
        assert!(!allocation.is_null());
        assert_eq!(allocation as usize % 64, 0);
        unsafe { allocator.deallocate(allocation, 17, 64) };
        assert_eq!(allocator.used_bytes(), 0);
    }

    #[test]
    fn allocations_are_aligned_and_reused_after_free() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 16 * 1024]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let first = allocator.allocate(17, 64);
        assert!(!first.is_null());
        assert_eq!(first as usize % 64, 0);
        unsafe { allocator.deallocate(first, 17, 64) };
        let again = allocator.allocate(17, 64);
        assert_eq!(again, first);
    }

    #[test]
    fn adjacent_frees_coalesce_to_fit_a_larger_request() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 1024]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let a = allocator.allocate(200, 16);
        let b = allocator.allocate(200, 16);
        let c = allocator.allocate(200, 16);
        assert!(!a.is_null() && !b.is_null() && !c.is_null());
        unsafe {
            allocator.deallocate(a, 200, 16);
            allocator.deallocate(b, 200, 16);
        }
        let joined = allocator.allocate(400, 16);
        assert_eq!(joined, a);
    }

    #[test]
    fn allocation_fails_cleanly_when_capacity_is_exhausted() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 512]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let allocation = allocator.allocate(4096, 16);
        assert!(allocation.is_null());
    }

    #[test]
    fn zeroed_allocation_clears_reused_dirty_bytes() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0xa5; 2048]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let allocation = allocator.allocate_zeroed(113, 64);
        assert!(!allocation.is_null());
        assert_eq!(allocation as usize % 64, 0);
        assert_eq!(
            unsafe { core::slice::from_raw_parts(allocation, 113) },
            &[0; 113]
        );
        unsafe { allocator.deallocate(allocation, 113, 64) };
        assert_eq!(allocator.used_bytes(), 0);
    }

    #[test]
    fn resize_preserves_bytes_alignment_and_releases_old_storage() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 4096]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let allocation = allocator.allocate(8, 64);
        assert!(!allocation.is_null());
        let expected = [0x13, 0x27, 0x39, 0x4b, 0x5d, 0x6f, 0x71, 0x83];
        unsafe { allocation.copy_from_nonoverlapping(expected.as_ptr(), expected.len()) };
        let grown = unsafe { allocator.reallocate(allocation, 8, 64, 128) };
        assert!(!grown.is_null());
        assert_eq!(grown as usize % 64, 0);
        assert_eq!(unsafe { core::slice::from_raw_parts(grown, 8) }, &expected);
        let shrunk = unsafe { allocator.reallocate(grown, 128, 64, 4) };
        assert!(!shrunk.is_null());
        assert_eq!(shrunk as usize % 64, 0);
        assert_eq!(
            unsafe { core::slice::from_raw_parts(shrunk, 4) },
            &expected[..4]
        );
        unsafe { allocator.deallocate(shrunk, 4, 64) };
        assert_eq!(allocator.used_bytes(), 0);
    }

    #[test]
    fn failed_resize_preserves_the_live_allocation() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 1024]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let allocation = allocator.allocate(8, 32);
        assert!(!allocation.is_null());
        unsafe { allocation.write_bytes(0x9d, 8) };
        let used = allocator.used_bytes();
        for requested in [8192, usize::MAX] {
            let failed = unsafe { allocator.reallocate(allocation, 8, 32, requested) };
            assert!(failed.is_null());
            assert_eq!(allocator.used_bytes(), used);
            assert_eq!(
                unsafe { core::slice::from_raw_parts(allocation, 8) },
                &[0x9d; 8]
            );
        }
        unsafe { allocator.deallocate(allocation, 8, 32) };
        assert_eq!(allocator.used_bytes(), 0);
    }

    #[test]
    fn reports_live_allocation_bytes() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 1024]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        assert_eq!(allocator.used_bytes(), 0);
        let allocation = allocator.allocate(128, 16);
        assert!(!allocation.is_null());
        assert!(allocator.used_bytes() >= 128);
        unsafe { allocator.deallocate(allocation, 128, 16) };
        assert_eq!(allocator.used_bytes(), 0);
    }

    #[test]
    fn rejects_invalid_region_sizes_and_alignments() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 64]);
        assert_eq!(
            unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), 8) },
            Err(InitError::TooSmall)
        );
        assert!(allocator.allocate(8, 3).is_null());
    }

    #[test]
    fn repeated_initialization_does_not_reset_live_allocations() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 1024]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();
        let allocation = allocator.allocate(8, 16);
        assert!(!allocation.is_null());
        unsafe { allocation.write_bytes(0x4e, 8) };
        let used = allocator.used_bytes();
        assert_eq!(
            unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) },
            Err(InitError::AlreadyInitialized)
        );
        assert_eq!(allocator.used_bytes(), used);
        assert_eq!(
            unsafe { core::slice::from_raw_parts(allocation, 8) },
            &[0x4e; 8]
        );
        unsafe { allocator.deallocate(allocation, 8, 16) };
        assert_eq!(allocator.used_bytes(), 0);
    }

    #[cfg(target_arch = "powerpc")]
    #[test]
    fn initialize_wii_caps_heap_and_leaves_reserved_space() {
        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; 1024]);
        let arena_low = memory.0.as_mut_ptr();
        let arena_high = arena_low.wrapping_add(memory.0.len());

        // SAFETY: the test buffer is writable, exclusively owned, and remains
        // alive for the allocator's entire use in this test.
        let mut published_low = core::ptr::null_mut();
        let heap_end = unsafe {
            initialize_wii(&allocator, arena_low, arena_high, 512, 128, |new_low| {
                assert_eq!(allocator.capacity(), 512);
                published_low = new_low;
            })
        }
        .expect("test buffer can hold the bounded heap");

        assert_eq!(heap_end as usize - arena_low as usize, 512);
        assert_eq!(published_low as usize, heap_end as usize);
        assert_eq!(allocator.capacity(), 512);
    }
}
