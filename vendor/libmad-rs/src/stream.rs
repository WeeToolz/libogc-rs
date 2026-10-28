//! Input buffer state and recoverable error values from `stream.c`.

use crate::bit::BitPtr;
use alloc::vec::Vec;

pub const MAD_BUFFER_GUARD: usize = 8;
pub const MAD_BUFFER_MDLEN: usize = 511 + 2048 + MAD_BUFFER_GUARD;

// Keep codes, text, and raw-code conversion in one definition.
macro_rules! define_errors {
    ($($name:ident = $code:literal => $text:literal,)*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(u16)]
        pub enum Error { $($name = $code,)* }

        impl Error {
            pub const fn from_code(code: u16) -> Option<Self> {
                match code { $($code => Some(Self::$name),)* _ => None }
            }

            pub const fn is_recoverable(self) -> bool {
                error_is_recoverable(self as i32)
            }

            pub const fn description(self) -> &'static str {
                match self { $(Self::$name => $text,)* }
            }

            /// Static, NUL-terminated text for C ABI adapters.
            pub const fn c_description(self) -> &'static [u8] {
                match self { $(Self::$name => concat!($text, "\0").as_bytes(),)* }
            }
        }
    };
}

/// Apply the libmad recovery mask, including to unknown raw error codes.
pub const fn error_is_recoverable(code: i32) -> bool {
    code & 0xff00 != 0
}

define_errors! {
    None = 0x0000 => "no error",
    BufferLength = 0x0001 => "input buffer too small (or EOF)",
    BufferPointer = 0x0002 => "invalid (null) buffer pointer",
    NoMemory = 0x0031 => "not enough memory",
    LostSync = 0x0101 => "lost synchronization",
    BadLayer = 0x0102 => "reserved header layer value",
    BadBitrate = 0x0103 => "forbidden bitrate value",
    BadSampleRate = 0x0104 => "reserved sample frequency value",
    BadEmphasis = 0x0105 => "reserved emphasis value",
    BadCrc = 0x0201 => "CRC check failed",
    BadBitAllocation = 0x0211 => "forbidden bit allocation value",
    BadScaleFactor = 0x0221 => "bad scalefactor index",
    BadMode = 0x0222 => "bad bitrate/mode combination",
    BadFrameLength = 0x0231 => "bad frame length",
    BadBigValues = 0x0232 => "bad big_values count",
    BadBlockType = 0x0233 => "reserved block_type",
    BadScfsi = 0x0234 => "bad scalefactor selection info",
    BadDataPointer = 0x0235 => "bad main_data_begin pointer",
    BadPart3Length = 0x0236 => "bad audio data length",
    BadHuffmanTable = 0x0237 => "bad Huffman table select",
    BadHuffmanData = 0x0238 => "Huffman data overrun",
    BadStereo = 0x0239 => "incompatible block_type for JS",
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream {
    pub buffer: Vec<u8>,
    pub buffer_set: bool,
    pub skiplen: u32,
    pub sync: bool,
    pub freerate: u32,
    pub this_frame: usize,
    pub next_frame: usize,
    pub ptr: BitPtr,
    pub anc_ptr: BitPtr,
    pub anc_bitlen: u32,
    pub main_data: Vec<u8>,
    pub md_len: u32,
    pub options: u32,
    pub error: Error,
}

impl Default for Stream {
    fn default() -> Self {
        Self::new()
    }
}

impl Stream {
    pub const fn new() -> Self {
        Self {
            buffer: Vec::new(),
            buffer_set: false,
            skiplen: 0,
            sync: false,
            freerate: 0,
            this_frame: 0,
            next_frame: 0,
            ptr: BitPtr::new(0),
            anc_ptr: BitPtr::new(0),
            anc_bitlen: 0,
            main_data: Vec::new(),
            md_len: 0,
            options: 0,
            error: Error::None,
        }
    }

    /// Copy input as a Rust-owned buffer. libmad C retains the caller's pointer;
    /// Rust ownership avoids a dangling pointer and does not change byte data.
    pub fn buffer(&mut self, input: &[u8]) {
        self.buffer.clear();
        self.buffer.extend_from_slice(input);
        self.buffer_set = true;
        self.this_frame = 0;
        self.next_frame = 0;
        self.sync = true;
        self.ptr = BitPtr::new(0);
    }

    pub fn skip(&mut self, length: u32) {
        self.skiplen = self.skiplen.wrapping_add(length);
    }

    /// Find the next MPEG sync and require libmad's eight-byte guard region.
    pub fn sync_to_next(&mut self) -> Result<(), Error> {
        let start = self.ptr.next_byte();
        let found = (start..self.buffer.len().saturating_sub(1))
            .find(|&i| self.buffer[i] == 0xff && self.buffer[i + 1] & 0xe0 == 0xe0);
        let Some(index) = found else {
            return Err(Error::BufferLength);
        };
        if self.buffer.len().saturating_sub(index) < MAD_BUFFER_GUARD {
            return Err(Error::BufferLength);
        }
        self.ptr = BitPtr::new(index);
        Ok(())
    }

    pub fn error_description(&self) -> &'static str {
        self.error.description()
    }

    pub fn set_error(&mut self, error: Error) {
        self.error = error;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_accumulates_with_c_unsigned_wrapping() {
        let mut stream = Stream::new();
        stream.skip(12);
        stream.skip(7);
        assert_eq!(stream.skiplen, 19);
        stream.skip(u32::MAX);
        assert_eq!(stream.skiplen, 18);
    }

    #[test]
    fn sync_requires_guard_and_preserves_cursor_on_failure() {
        let mut stream = Stream::new();
        stream.buffer(&[0x00, 0xff, 0xfb, 1, 2, 3, 4, 5]);
        assert_eq!(stream.sync_to_next(), Err(Error::BufferLength));
        assert_eq!(stream.ptr, BitPtr::new(0));
        stream.buffer(&[0x00, 0xff, 0xfb, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(stream.sync_to_next(), Ok(()));
        assert_eq!(stream.ptr, BitPtr::new(1));
    }

    #[test]
    fn recoverable_errors_use_c_error_class_mask() {
        assert!(!Error::BufferLength.is_recoverable());
        assert!(Error::BadCrc.is_recoverable());
        assert_eq!(Error::BadHuffmanData.description(), "Huffman data overrun");
    }

    #[test]
    fn distinguishes_unset_buffer_from_empty_buffer() {
        let mut stream = Stream::new();
        assert_eq!(
            crate::frame::decode_header(&mut stream),
            Err(Error::BufferPointer)
        );
        stream.buffer(&[]);
        assert_eq!(
            crate::frame::decode_header(&mut stream),
            Err(Error::BufferLength)
        );
    }
}

#[cfg(test)]
mod error_definition_tests {
    use super::*;

    #[test]
    fn error_codes_round_trip_and_c_text_matches() {
        let mut known = 0;
        for code in 0..=u16::MAX {
            if let Some(error) = Error::from_code(code) {
                known += 1;
                assert_eq!(error as u16, code);
                let text = error.c_description();
                assert_eq!(text.last(), Some(&0));
                assert_eq!(&text[..text.len() - 1], error.description().as_bytes());
                assert_eq!(error.is_recoverable(), code & 0xff00 != 0);
            }
        }
        assert_eq!(known, 22);
        assert_eq!(Error::from_code(0x7fff), None);
        assert!(error_is_recoverable(0x7fff));
    }
}
