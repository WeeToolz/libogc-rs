//! Borrowed IOS ioctl buffers with canonical absent-buffer encoding.
use core::ffi::c_void;
use core::marker::PhantomData;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct IoctlBuffers<'a> {
    buffer_in: *mut c_void,
    len_in: i32,
    buffer_io: *mut c_void,
    len_io: i32,
    lifetime: PhantomData<&'a mut [u8]>,
}

impl<'a> IoctlBuffers<'a> {
    pub const fn empty() -> Self {
        Self {
            buffer_in: core::ptr::null_mut(),
            len_in: 0,
            buffer_io: core::ptr::null_mut(),
            len_io: 0,
            lifetime: PhantomData,
        }
    }

    pub fn from_slices(buffer_in: Option<&'a mut [u8]>, buffer_io: Option<&'a mut [u8]>) -> Self {
        let (buffer_in_ptr, len_in) = match buffer_in {
            Some(buf) if !buf.is_empty() => (buf.as_mut_ptr().cast::<c_void>(), buf.len() as i32),
            _ => (core::ptr::null_mut(), 0),
        };
        let (buffer_io_ptr, len_io) = match buffer_io {
            Some(buf) if !buf.is_empty() => (buf.as_mut_ptr().cast::<c_void>(), buf.len() as i32),
            _ => (core::ptr::null_mut(), 0),
        };
        Self {
            buffer_in: buffer_in_ptr,
            len_in,
            buffer_io: buffer_io_ptr,
            len_io,
            lifetime: PhantomData,
        }
    }

    pub const fn len_in(&self) -> i32 {
        self.len_in
    }

    pub const fn len_io(&self) -> i32 {
        self.len_io
    }

    pub const fn buffer_in(&self) -> *mut c_void {
        self.buffer_in
    }

    pub const fn buffer_io(&self) -> *mut c_void {
        self.buffer_io
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_and_empty_buffers_both_encode_null_and_zero() {
        let mut input = [];
        let mut output = [];
        for buffers in [
            IoctlBuffers::empty(),
            IoctlBuffers::from_slices(None, None),
            IoctlBuffers::from_slices(Some(&mut input), Some(&mut output)),
        ] {
            assert!(buffers.buffer_in().is_null());
            assert!(buffers.buffer_io().is_null());
            assert_eq!((buffers.len_in(), buffers.len_io()), (0, 0));
        }
    }
    #[test]
    fn real_buffers_keep_their_exact_addresses_and_lengths() {
        let mut input = [1u8; 36];
        let mut output = [2u8; 5];
        let input_pointer = input.as_mut_ptr().cast();
        let output_pointer = output.as_mut_ptr().cast();
        let buffers = IoctlBuffers::from_slices(Some(&mut input), Some(&mut output));
        assert_eq!(buffers.buffer_in(), input_pointer);
        assert_eq!(buffers.buffer_io(), output_pointer);
        assert_eq!((buffers.len_in(), buffers.len_io()), (36, 5));
    }
}
