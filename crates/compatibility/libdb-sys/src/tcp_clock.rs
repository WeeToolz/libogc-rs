#[derive(Default)]
pub(crate) struct TcpClock {
    accumulated: u64,
    started: Option<u64>,
}

impl TcpClock {
    pub(crate) fn elapsed(&self, now: u64) -> u64 {
        self.accumulated
            .wrapping_add(self.started.map_or(0, |start| now.wrapping_sub(start)))
    }

    pub(crate) fn start(&mut self, now: u64) {
        if self.started.is_none() {
            self.started = Some(now);
        }
    }

    pub(crate) fn stop(&mut self, now: u64) {
        self.accumulated = self.elapsed(now);
        self.started = None;
    }

    pub(crate) fn is_running(&self) -> bool {
        self.started.is_some()
    }
}
