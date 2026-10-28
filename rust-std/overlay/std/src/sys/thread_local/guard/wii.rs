pub(crate) fn enable() {
    // Register cleanup outside the OS key table. LWP calls it after all key
    // destructor passes, even when user TLS is initialized during destruction.
    let result = unsafe { crate::sys::wii::thread_cleanup_register(Some(cleanup)) };
    if result != 0 {
        rtabort!("Wii runtime cleanup registration failed: {result}");
    }
}

unsafe extern "C" fn cleanup() {
    crate::rt::thread_cleanup();
}
