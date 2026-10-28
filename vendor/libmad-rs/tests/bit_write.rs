//! Verify write values, neighboring bits, and failure atomicity.

use libmad_rs::bit::{BitError, BitPtr};

#[test]
fn write_keeps_bits_outside_the_requested_range() {
    let mut data = [0xa5, 0x5a, 0xc3];
    let mut ptr = BitPtr::new(0);
    ptr.skip(3, &data).unwrap();
    ptr.write(&mut data, 10, 0b1100110011).unwrap();
    assert_eq!(data, [0xb9, 0x9a, 0xc3]);
    assert_eq!((ptr.byte, ptr.left, ptr.cache), (1, 3, 0x9a));
}

#[test]
fn write_that_runs_out_of_input_keeps_bytes_and_cursor_unchanged() {
    let mut data = [0xa5, 0x5a];
    let mut ptr = BitPtr::new(0);
    ptr.skip(3, &data).unwrap();
    let before = ptr;
    assert_eq!(ptr.write(&mut data, 14, 0), Err(BitError::OutOfBounds));
    assert_eq!(data, [0xa5, 0x5a]);
    assert_eq!(ptr, before);
    // The failed write must leave the cursor usable for the exact remaining bits.
    ptr.write(&mut data, 13, 0).unwrap();
    assert_eq!(data, [0xa0, 0x00]);
    assert_eq!((ptr.byte, ptr.left), (2, 8));
}

#[test]
fn excessive_width_is_rejected_before_input_or_cursor_changes() {
    let mut data = [0xa5; 8];
    let mut ptr = BitPtr::new(0);
    ptr.skip(5, &data).unwrap();
    let before = ptr;
    assert_eq!(
        ptr.write(&mut data, 33, u32::MAX),
        Err(BitError::TooManyBits)
    );
    assert_eq!(ptr.read(&data, 33), Err(BitError::TooManyBits));
    assert_eq!(data, [0xa5; 8]);
    assert_eq!(ptr, before);
}

#[test]
fn zero_width_operations_at_buffer_end_do_not_access_input() {
    let mut data = [];
    let mut ptr = BitPtr::new(0);
    let before = ptr;
    assert_eq!(ptr.write(&mut data, 0, u32::MAX), Ok(()));
    assert_eq!(ptr.read(&data, 0), Ok(0));
    assert_eq!(ptr, before);
}

#[test]
fn full_width_write_spans_five_bytes_without_losing_high_bits() {
    let mut data = [0xa5, 0x5a, 0xc3, 0x6d, 0x5a];
    let mut ptr = BitPtr::new(0);
    ptr.skip(3, &data).unwrap();
    ptr.write(&mut data, 32, 0x1234_5678).unwrap();
    assert_eq!(data, [0xa2, 0x46, 0x8a, 0xcf, 0x1a]);
    assert_eq!((ptr.byte, ptr.left, ptr.cache), (4, 5, 0x1a));
}
