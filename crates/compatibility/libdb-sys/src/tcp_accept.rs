use alloc::collections::VecDeque;

pub(crate) const TCP_CONTROL_BLOCK_LIMIT: usize = 4;

pub(crate) fn has_control_block_capacity(current: usize) -> bool {
    current < TCP_CONTROL_BLOCK_LIMIT
}

#[derive(Default)]
pub(crate) struct AcceptedConnections(VecDeque<u32>);

impl AcceptedConnections {
    pub(crate) fn enqueue(&mut self, connection_id: u32) {
        self.0.push_back(connection_id);
    }

    pub(crate) fn dequeue(&mut self) -> Option<u32> {
        self.0.pop_front()
    }

    pub(crate) fn remove(&mut self, connection_id: u32) {
        self.0.retain(|queued_id| *queued_id != connection_id);
    }
}
