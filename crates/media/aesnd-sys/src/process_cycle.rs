//! DSP request completion state shared by the production callbacks and tests.

#[derive(Clone, Copy)]
pub(super) struct ProcessCycle {
    ready: bool,
    started: Option<u64>,
    output_requested: bool,
    elapsed: u64,
}

impl ProcessCycle {
    pub const fn new() -> Self {
        Self {
            ready: false,
            started: None,
            output_requested: false,
            elapsed: 0,
        }
    }

    pub fn task_ready(&mut self) {
        self.ready = true;
    }
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    pub fn is_active(&self) -> bool {
        self.started.is_some()
    }
    pub fn output_requested(&self) -> bool {
        self.output_requested
    }

    pub fn begin(&mut self, now: u64) -> bool {
        if !self.ready || self.started.is_some() {
            return false;
        }
        self.ready = false;
        self.elapsed = 0;
        self.started = Some(now);
        true
    }

    pub fn request_output(&mut self) {
        self.output_requested = true;
    }

    pub fn complete(&mut self, now: u64) -> bool {
        if !self.output_requested {
            return false;
        }
        let Some(started) = self.started.take() else {
            return false;
        };
        self.elapsed = now.wrapping_sub(started);
        self.output_requested = false;
        self.ready = true;
        true
    }

    pub fn task_done(&mut self) {
        self.ready = false;
        self.started = None;
        self.output_requested = false;
    }

    pub fn idle(&mut self) {
        self.elapsed = 0;
    }

    pub fn microseconds(&self, ticks_per_second: u64) -> u32 {
        ((u128::from(self.elapsed) * 1_000_000 / u128::from(ticks_per_second))
            .min(u128::from(u32::MAX))) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_reports_dsp_interval_only_after_the_output_response() {
        let mut cycle = ProcessCycle::new();
        assert!(!cycle.begin(100));
        cycle.task_ready();
        assert!(cycle.begin(100));
        assert!(!cycle.begin(200));
        assert!(!cycle.complete(300));
        assert_eq!(cycle.microseconds(60_750_000), 0);
        cycle.request_output();
        assert!(cycle.output_requested());
        assert!(cycle.complete(60_850));
        assert!(!cycle.output_requested());
        assert!(cycle.is_ready());
        assert!(!cycle.is_active());
        assert_eq!(cycle.microseconds(60_750_000), 1_000);
        assert!(!cycle.complete(90_000));
        assert_eq!(cycle.microseconds(60_750_000), 1_000);
    }
    #[test]
    fn timer_wrap_and_task_shutdown_preserve_completed_metric() {
        let mut cycle = ProcessCycle::new();
        cycle.task_ready();
        assert!(cycle.begin(u64::MAX - 9));
        cycle.request_output();
        assert!(cycle.complete(10));
        assert_eq!(cycle.microseconds(1_000_000), 20);
        cycle.task_done();
        assert!(!cycle.is_ready());
        assert_eq!(cycle.microseconds(1_000_000), 20);
        cycle.idle();
        assert_eq!(cycle.microseconds(1_000_000), 0);
    }
}
