pub(super) fn read_count(request: impl FnOnce(&mut u32) -> i32) -> (i32, u32) {
    let mut value = 0;
    let result = request(&mut value);
    (result, value)
}

#[cfg(test)]
mod tests {
    use super::read_count;

    #[test]
    fn copies_count_and_preserves_ios_result_on_success_and_failure() {
        assert_eq!(read_count(|_| -17), (-17, 0));
        assert_eq!(
            read_count(|value| {
                *value = 3;
                7
            }),
            (7, 3)
        );
        assert_eq!(
            read_count(|value| {
                *value = 5;
                -4
            }),
            (-4, 5)
        );
    }
}
