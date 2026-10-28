//! Explicit ABI failure injection against the unchanged production std adapter.
pub use std::io;
#[path = "../../rust-std/overlay/std/src/sys/stdio/wii.rs"]
mod adapter;
mod sys {
    pub mod wii {
        use std::cell::RefCell;
        #[derive(Default)]
        pub struct State {
            pub result: isize,
            pub bytes: Vec<u8>,
            pub flushes: usize,
        }
        thread_local! { pub static STATE: RefCell<State> = RefCell::new(State::default()); }
        pub unsafe fn output(bytes: *const u8, length: usize, flush: bool) -> isize {
            STATE.with_borrow_mut(|state| {
                if flush {
                    state.flushes += 1;
                } else if length != 0 {
                    // SAFETY: Production Write passes a live slice to the ABI.
                    state
                        .bytes
                        .extend_from_slice(unsafe { core::slice::from_raw_parts(bytes, length) });
                }
                state.result
            })
        }
    }
}
use io::{Read, Write};
use sys::wii::STATE;
#[test]
fn writes_pass_actual_bytes_and_partial_counts() {
    STATE.with_borrow_mut(|state| state.result = 3);
    let count = adapter::Stdout::new().write(b"abcde").unwrap();
    assert_eq!(count, 3);
    STATE.with_borrow(|state| assert_eq!(state.bytes, b"abcde"));
}
#[test]
fn write_and_flush_preserve_transport_errors() {
    STATE.with_borrow_mut(|state| state.result = -19);
    assert_eq!(
        adapter::Stdout::new()
            .write(b"abc")
            .unwrap_err()
            .raw_os_error(),
        Some(19)
    );
    STATE.with_borrow_mut(|state| state.result = -5);
    assert_eq!(
        adapter::Stdout::new().flush().unwrap_err().raw_os_error(),
        Some(5)
    );
    STATE.with_borrow(|state| assert_eq!(state.flushes, 1));
}
#[test]
fn default_panic_sink_uses_the_same_checked_transport() {
    STATE.with_borrow_mut(|state| state.result = 12);
    adapter::panic_output()
        .expect("real panic sink")
        .write_all(b"panic marker")
        .unwrap();
    STATE.with_borrow(|state| assert_eq!(state.bytes, b"panic marker"));
}
#[test]
fn stdin_has_no_false_input_success() {
    assert_eq!(
        adapter::Stdin::new().read(&mut [0; 1]).unwrap_err().kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(adapter::Stdin::new().read(&mut []).unwrap(), 0);
    assert!(adapter::is_ebadf(&io::Error::from_raw_os_error(9)));
    assert_eq!(adapter::STDIN_BUF_SIZE, 0);
}
