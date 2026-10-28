pub(super) const LIBDI_MAX_RETRIES: u32 = 16;

pub(super) fn retry_di_read(mut read: impl FnMut() -> i32, max_retries: u32) -> i32 {
    let mut result = 0;
    for _ in 0..max_retries {
        result = read();
        if result == 0 {
            return 0;
        }
    }
    result
}
