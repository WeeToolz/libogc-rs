//! Wii System allocator calls the Rust provider linked by the application.

use crate::alloc::Layout;
use crate::sys::wii;

pub unsafe fn alloc(layout: Layout) -> *mut u8 {
    // SAFETY: Layout supplies the checked size and alignment for the provider.
    unsafe { wii::alloc(layout.size(), layout.align()) }
}

pub unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    // SAFETY: Layout supplies the checked size and alignment for the provider.
    unsafe { wii::alloc_zeroed(layout.size(), layout.align()) }
}

pub unsafe fn dealloc(pointer: *mut u8, layout: Layout) {
    // SAFETY: The System caller supplies its live allocation and original layout.
    unsafe { wii::dealloc(pointer, layout.size(), layout.align()) };
}

pub unsafe fn realloc(pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    // SAFETY: The System caller supplies its live allocation and a nonzero size.
    unsafe { wii::realloc(pointer, layout.size(), layout.align(), new_size) }
}
