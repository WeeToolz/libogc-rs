use crate::io::{self, Read, Write};

pub struct Stdin;
pub struct Stdout;
pub type Stderr = Stdout;

impl Stdin {
    pub const fn new() -> Self {
        Self
    }
}
impl Stdout {
    pub const fn new() -> Self {
        Self
    }
}

impl Read for Stdin {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            Ok(0)
        } else {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}
impl Write for Stdout {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = unsafe { crate::sys::wii::output(bytes.as_ptr(), bytes.len(), false) };
        if result < 0 {
            Err(io::Error::from_raw_os_error((-result) as i32))
        } else {
            Ok(result as usize)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = unsafe { crate::sys::wii::output(core::ptr::null(), 0, true) };
        if result < 0 {
            Err(io::Error::from_raw_os_error((-result) as i32))
        } else {
            Ok(())
        }
    }
}

pub const STDIN_BUF_SIZE: usize = 0;
pub fn is_ebadf(error: &io::Error) -> bool {
    error.raw_os_error() == Some(9)
}
pub fn panic_output() -> Option<Stderr> {
    Some(Stderr::new())
}
