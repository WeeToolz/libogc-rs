pub fn board_changed<const N: usize>(before: &[u8; N], after: &[u8; N]) -> bool {
    before != after
}

#[cfg(test)]
mod tests {
    use super::board_changed;

    #[test]
    fn cursor_only_move_between_different_cell_values_is_not_an_edit() {
        let before = [1, 2, 0, 0];
        let after = before;
        let previous_cursor = 0;
        let current_cursor = 1;
        assert_ne!(before[previous_cursor], before[current_cursor]);
        assert!(!board_changed(&before, &after));
    }

    #[test]
    fn actual_cell_edit_is_reported() {
        let before = [1, 2, 0, 0];
        let after = [1, 3, 0, 0];
        assert!(board_changed(&before, &after));
    }
}
