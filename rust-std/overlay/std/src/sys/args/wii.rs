use crate::ffi::{CStr, OsString};
use crate::ptr;

pub type Args = crate::vec::IntoIter<OsString>;

pub fn args() -> Args {
    let mut argc = 0;
    let mut argv = ptr::null();
    let result = unsafe { crate::sys::wii::arguments(&mut argc, &mut argv) };
    assert_eq!(result, 0, "Wii loader arguments failed: {result}");
    let mut result = Vec::with_capacity(argc);
    for index in 0..argc {
        // Preparation validates and shelters all strings and the vector before
        // BSS clearing. The provider returns that immutable application-lifetime
        // view. Each iterator owns its bytes before it is returned to the caller.
        let argument = unsafe { CStr::from_ptr((*argv.add(index)).cast()) };
        // Wii selects the byte-preserving OsStr backend, so all loader bytes
        // are valid platform encoding, including non-UTF-8 byte sequences.
        result
            .push(unsafe { OsString::from_encoded_bytes_unchecked(argument.to_bytes().to_vec()) });
    }
    result.into_iter()
}
