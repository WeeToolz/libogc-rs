//! Checked LWP keys. The provider owns the application's shared thread state.

use crate::sys::wii;

pub type Key = u32;

pub fn create(destructor: Option<unsafe extern "C" fn(*mut u8)>) -> Key {
    let mut key = 0;
    // SAFETY: key is writable and the Rust TLS engine owns its destructor values.
    let result = unsafe { wii::tls_create(destructor, &mut key) };
    if result != 0 {
        rtabort!("Wii TLS key creation failed: {}", result);
    }
    key
}

pub unsafe fn get(key: Key) -> *mut u8 {
    let mut value = crate::ptr::null_mut();
    // SAFETY: value is writable for the synchronous provider call.
    let result = unsafe { wii::tls_get(key, &mut value) };
    if result != 0 {
        rtabort!("Wii TLS read failed: {}", result);
    }
    value
}

pub unsafe fn set(key: Key, value: *mut u8) {
    // SAFETY: The Rust TLS engine owns value through replacement or destruction.
    let result = unsafe { wii::tls_set(key, value) };
    if result != 0 {
        rtabort!("Wii TLS write failed: {}", result);
    }
}

pub unsafe fn destroy(key: Key) {
    // SAFETY: This engine owns key and deletes it after its last required use.
    let result = unsafe { wii::tls_destroy(key) };
    if result != 0 {
        rtabort!("Wii TLS key deletion failed: {}", result);
    }
}
