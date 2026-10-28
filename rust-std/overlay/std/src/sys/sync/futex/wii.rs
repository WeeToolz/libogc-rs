use crate::sync::atomic::AtomicU32;
use crate::time::Duration;

pub type Futex = AtomicU32;
pub type Primitive = u32;
pub type SmallFutex = AtomicU32;
pub type SmallPrimitive = u32;

// The pinned std algorithms provide acquire/release ordering on the words.
// LWP provides comparison and wait registration atomically against wake on the
// single Wii CPU. It allocates no lazy lock/condition handle or waiter object.
pub fn futex_wait(word: &AtomicU32, expected: u32, timeout: Option<Duration>) -> bool {
    let duration = timeout.unwrap_or(Duration::ZERO);
    let result = unsafe {
        crate::sys::wii::futex_wait(
            word,
            expected,
            duration.as_secs(),
            duration.subsec_nanos(),
            timeout.is_some(),
        )
    };
    match result {
        0 => true,
        1 => false,
        error => panic!("Wii futex wait failed: {error}"),
    }
}

pub fn futex_wake(word: &AtomicU32) -> bool {
    let result = unsafe { crate::sys::wii::futex_wake(word, false) };
    assert!(result >= 0, "Wii futex wake failed: {result}");
    result != 0
}

pub fn futex_wake_all(word: &AtomicU32) {
    let result = unsafe { crate::sys::wii::futex_wake(word, true) };
    assert!(result >= 0, "Wii futex wake-all failed: {result}");
}
