pub(super) const fn worker_is_quiescent(joined: bool, finished: bool) -> bool {
    joined || finished
}

#[cfg(test)]
mod tests {
    use super::worker_is_quiescent;

    #[test]
    fn worker_is_quiescent_after_join_or_reported_completion() {
        assert!(worker_is_quiescent(true, false));
        assert!(worker_is_quiescent(false, true));
        assert!(!worker_is_quiescent(false, false));
    }
}
