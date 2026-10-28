//! ABI supplied by the application's ogc-std-runtime Rust crate.

#[path = "wii_net_abi.rs"]
mod net_abi;
pub use net_abi::NetRequest;

#[path = "wii_fs_abi.rs"]
pub mod fs_abi;
pub use fs_abi::FsRequest;

unsafe extern "C" {
    #[link_name = "__wii_std_output"]
    pub fn output(bytes: *const u8, length: usize, flush: bool) -> isize;
    #[link_name = "__wii_std_fs"]
    pub fn fs(request: *mut FsRequest) -> i64;
    #[link_name = "__wii_std_net"]
    pub fn net(request: *mut NetRequest) -> i64;
    #[link_name = "__wii_std_alloc"]
    pub fn alloc(size: usize, alignment: usize) -> *mut u8;
    #[link_name = "__wii_std_alloc_zeroed"]
    pub fn alloc_zeroed(size: usize, alignment: usize) -> *mut u8;
    #[link_name = "__wii_std_dealloc"]
    pub fn dealloc(pointer: *mut u8, size: usize, alignment: usize);
    #[link_name = "__wii_std_realloc"]
    pub fn realloc(pointer: *mut u8, size: usize, alignment: usize, new_size: usize) -> *mut u8;
    #[link_name = "__wii_std_errno"]
    pub fn errno() -> i32;
    #[link_name = "__wii_std_tls_create"]
    pub fn tls_create(destructor: Option<unsafe extern "C" fn(*mut u8)>, key: *mut u32) -> i32;
    #[link_name = "__wii_std_tls_get"]
    pub fn tls_get(key: u32, output: *mut *mut u8) -> i32;
    #[link_name = "__wii_std_tls_set"]
    pub fn tls_set(key: u32, value: *mut u8) -> i32;
    #[link_name = "__wii_std_tls_destroy"]
    pub fn tls_destroy(key: u32) -> i32;
    #[link_name = "__wii_std_futex_wait"]
    pub fn futex_wait(
        word: *const crate::sync::atomic::AtomicU32,
        expected: u32,
        seconds: u64,
        nanoseconds: u32,
        timed: bool,
    ) -> i32;
    #[link_name = "__wii_std_futex_wake"]
    pub fn futex_wake(word: *const crate::sync::atomic::AtomicU32, all: bool) -> i32;
    #[link_name = "__wii_std_runtime_enter"]
    pub fn runtime_enter(argc: isize, argv: *const *const u8) -> i32;
    #[link_name = "__wii_std_runtime_cleanup"]
    pub fn runtime_cleanup() -> i32;
    #[link_name = "__wii_std_arguments"]
    pub fn arguments(argc: *mut usize, argv: *mut *const *const u8) -> i32;
    #[link_name = "__wii_std_thread_create"]
    pub fn thread_create(
        entry: Option<unsafe extern "C" fn(*mut crate::ffi::c_void) -> *mut crate::ffi::c_void>,
        data: *mut crate::ffi::c_void,
        stack: usize,
        output: *mut u32,
    ) -> i32;
    #[link_name = "__wii_std_thread_join"]
    pub fn thread_join(thread: u32) -> i32;
    #[link_name = "__wii_std_thread_detach"]
    pub fn thread_detach(thread: u32) -> i32;
    #[link_name = "__wii_std_thread_current"]
    pub fn thread_current(output: *mut u32) -> i32;
    #[link_name = "__wii_std_thread_cleanup_register"]
    pub fn thread_cleanup_register(cleanup: Option<unsafe extern "C" fn()>) -> i32;
    #[link_name = "__wii_std_thread_set_name"]
    pub fn thread_set_name(bytes: *const u8, length: usize) -> i32;
    #[link_name = "__wii_std_thread_sleep"]
    pub fn thread_sleep(seconds: u64, nanoseconds: u32) -> i32;
    #[link_name = "__wii_std_thread_yield"]
    pub fn thread_yield() -> i32;
    #[link_name = "__wii_std_thread_parallelism"]
    pub fn thread_parallelism(output: *mut usize) -> i32;
    #[link_name = "__wii_std_clock_now"]
    pub fn clock_now(wall: bool, seconds: *mut u64, nanos: *mut u32) -> i32;

}
