/// No PowerPC entropy transport has been verified for Wii. Reject requests
/// rather than seed std from clocks, addresses, console IDs, or a fixed PRNG.
pub fn fill_bytes(bytes: &mut [u8]) {
    if !bytes.is_empty() {
        panic!("Wii random data is unsupported: no verified entropy service");
    }
}

#[cfg(test)]
mod tests {
    use super::fill_bytes;

    #[test]
    #[should_panic(expected = "no verified entropy service")]
    fn rejects_a_nonempty_request() {
        fill_bytes(&mut [0xa5; 16]);
    }

    #[test]
    fn empty_request_needs_no_entropy() {
        fill_bytes(&mut []);
    }
}
