//! Bounded final-screen failure storage. Overflow is always reported.
const CAPACITY: usize = 64;
const WIDTH: usize = 96;
pub const PAGE_SIZE: usize = 4;

pub struct FailureSummary {
    labels: [[u8; WIDTH]; CAPACITY],
    lengths: [usize; CAPACITY],
    details: [[u8; WIDTH]; CAPACITY],
    detail_lengths: [usize; CAPACITY],
    total: usize,
}
impl FailureSummary {
    pub const fn new() -> Self {
        Self {
            labels: [[0; WIDTH]; CAPACITY],
            lengths: [0; CAPACITY],
            details: [[0; WIDTH]; CAPACITY],
            detail_lengths: [0; CAPACITY],
            total: 0,
        }
    }
    pub fn record(&mut self, label: &str) {
        let index = self.total;
        self.total = self.total.saturating_add(1);
        if index >= CAPACITY {
            return;
        }
        let truncated = label.len() > WIDTH;
        let mut length = label.len().min(if truncated { WIDTH - 3 } else { WIDTH });
        while !label.is_char_boundary(length) {
            length -= 1;
        }
        self.labels[index][..length].copy_from_slice(&label.as_bytes()[..length]);
        if truncated {
            self.labels[index][length..length + 3].copy_from_slice(b"...");
            length += 3;
        }
        self.lengths[index] = length;
    }
    pub fn detail_last(&mut self, detail: &str) {
        let Some(index) = self.total.checked_sub(1).filter(|index| *index < CAPACITY) else {
            return;
        };
        let truncated = detail.len() > WIDTH;
        let mut length = detail.len().min(if truncated { WIDTH - 3 } else { WIDTH });
        while !detail.is_char_boundary(length) {
            length -= 1;
        }
        self.details[index][..length].copy_from_slice(&detail.as_bytes()[..length]);
        if truncated {
            self.details[index][length..length + 3].copy_from_slice(b"...");
            length += 3;
        }
        self.detail_lengths[index] = length;
    }
    #[cfg(test)]
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        (0..self.total.min(CAPACITY)).map(|index| {
            // SAFETY: record copies complete UTF-8 characters and ASCII suffixes.
            unsafe { core::str::from_utf8_unchecked(&self.labels[index][..self.lengths[index]]) }
        })
    }
    pub fn entries(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        (0..self.total.min(CAPACITY)).map(|index| {
            let label = unsafe {
                core::str::from_utf8_unchecked(&self.labels[index][..self.lengths[index]])
            };
            let detail_length = self.detail_lengths[index];
            let detail = (detail_length > 0).then(|| unsafe {
                core::str::from_utf8_unchecked(&self.details[index][..detail_length])
            });
            (label, detail)
        })
    }
    pub fn pages(&self) -> usize {
        self.total.min(CAPACITY).div_ceil(PAGE_SIZE).max(1)
    }
    pub fn page(&self, index: usize) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.entries()
            .skip((index % self.pages()) * PAGE_SIZE)
            .take(PAGE_SIZE)
    }
    pub fn omitted(&self) -> usize {
        self.total.saturating_sub(CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_pages_cover_every_retained_check_once_and_keep_details() {
        let mut failures = FailureSummary::new();
        assert_eq!(failures.pages(), 1);
        assert_eq!(failures.page(0).count(), 0);
        for index in 0..19 {
            failures.record(&format!("failed check {index}"));
            failures.detail_last(&format!("real error {index}"));
        }
        assert_eq!(failures.pages(), 5);
        let pages = (0..failures.pages())
            .flat_map(|page| failures.page(page))
            .collect::<std::vec::Vec<_>>();
        assert_eq!(pages, failures.entries().collect::<std::vec::Vec<_>>());
        assert_eq!(
            failures.page(1).next(),
            Some(("failed check 4", Some("real error 4")))
        );
        assert_eq!(failures.page(5).next(), failures.page(0).next());
    }

    #[test]
    fn final_screen_keeps_earlier_failure_names_in_order() {
        let mut failures = FailureSummary::new();
        failures.record("allocation failure preserved bytes");
        failures.record("USB final close result");
        assert_eq!(
            failures.labels().collect::<std::vec::Vec<_>>(),
            [
                "allocation failure preserved bytes",
                "USB final close result"
            ]
        );
        assert_eq!(failures.omitted(), 0);
    }
    #[test]
    fn full_screen_reports_every_omitted_failure() {
        let mut failures = FailureSummary::new();
        for _ in 0..CAPACITY + 4 {
            failures.record("failure");
        }
        assert_eq!(failures.labels().count(), CAPACITY);
        assert_eq!(failures.omitted(), 4);
    }
    #[test]
    fn long_labels_keep_valid_utf8_and_show_truncation() {
        let mut failures = FailureSummary::new();
        failures.record(&"é".repeat(WIDTH));
        let label = failures.labels().next().unwrap();
        assert!(label.len() <= WIDTH);
        assert!(label.ends_with("..."));
        assert!(label.starts_with("é"));
    }
    #[test]
    fn final_screen_keeps_error_detail_with_its_failure() {
        let mut failures = FailureSummary::new();
        failures.record("UDP bind reaches IOS");
        failures.detail_last("domain 1 code -28 errno 22");
        failures.record("TCP request completes");
        assert_eq!(
            failures.entries().collect::<std::vec::Vec<_>>(),
            [
                ("UDP bind reaches IOS", Some("domain 1 code -28 errno 22")),
                ("TCP request completes", None)
            ]
        );
    }
}
