//! Single-owner asynchronous IOS adapter service for the HID host.
use crate::*;
use alloc::vec::Vec;
const IO_TIMEOUT_MS: u64 = 5_000;
const MAX_WRITES: usize = 8;
enum WriteRequest {
    Hci(HciCommandRequest),
    Acl(TransferRequest),
}
impl WriteRequest {
    fn result(&self) -> Option<i32> {
        match self {
            Self::Hci(r) => r.raw_result(),
            Self::Acl(r) => r.raw_result(),
        }
    }
}

/// Own one service for the internal adapter and share it across controller families.
/// Poll from one service thread. Serialize other host access with that service.
/// Completion callbacks only publish request results.
pub struct WiiHidHost {
    protocol: HidHost,
    adapter: Option<BtSyncHost>,
    open: Option<AdapterOpenRequest>,
    opening_deadline: u64,
    event_read: Option<TransferRequest>,
    acl_read: Option<TransferRequest>,
    writes: Vec<(WriteRequest, u64)>,
    close: Option<HciCommandRequest>,
    closing: bool,
    close_deadline: u64,
}
impl Default for WiiHidHost {
    fn default() -> Self {
        Self::new()
    }
}
impl WiiHidHost {
    pub fn new() -> Self {
        Self {
            protocol: HidHost::new(),
            adapter: None,
            open: None,
            opening_deadline: 0,
            event_read: None,
            acl_read: None,
            writes: Vec::new(),
            close: None,
            closing: false,
            close_deadline: 0,
        }
    }
    pub fn diagnostics(&self) -> HostDiagnostics {
        let mut d = self.protocol.diagnostics();
        if self.open.is_some() {
            d.stage = HostStage::AdapterOpen;
        }
        if self.closing {
            d.stage = HostStage::Closing;
        }
        d
    }
    /// Return validated Switch stick calibration for a ready controller slot.
    pub fn switch_calibration(&self, slot: usize) -> Option<switch::SwitchCalibration> {
        self.protocol.switch_calibration(slot)
    }
    /// Load a paired controller key from the platform's persistent store.
    pub fn load_link_key(&mut self, address: BluetoothAddress, key: [u8; 16]) {
        self.protocol.load_link_key(address, key);
    }
    /// Load a guest key from the platform's guest pairing store.
    pub fn load_guest_link_key(&mut self, address: BluetoothAddress, key: [u8; 16]) {
        self.protocol.load_guest_link_key(address, key);
    }
    pub fn start(&mut self, now: u64) -> Result<()> {
        if self.open.is_some() || self.adapter.is_some() {
            return Ok(());
        }
        if self.closing && !self.shutdown_complete() {
            return Err(Error::UnexpectedEvent);
        }
        if self.close.as_ref().is_some_and(|r| !r.is_complete()) {
            return Err(Error::UnexpectedEvent);
        }
        self.close.take();
        self.closing = false;
        if matches!(
            self.protocol.diagnostics().stage,
            HostStage::Failed | HostStage::Closing
        ) || self.protocol.diagnostics().stage != HostStage::Disabled
        {
            self.protocol.reset_preserving_keys();
        }
        self.open = Some(BtSyncHost::open_async()?);
        self.protocol.adapter_opening();
        self.opening_deadline = now + IO_TIMEOUT_MS;
        Ok(())
    }
    pub fn search(&mut self, now: u64) -> bool {
        self.protocol.search(now)
    }
    pub fn search_wii_guest(&mut self, now: u64) -> bool {
        self.protocol.search_wii_guest(now)
    }
    /// Drop volatile guest keys while retaining normal registered keys.
    pub fn clear_guest_link_keys(&mut self) {
        self.protocol.clear_guest_link_keys();
    }
    pub fn wii_remote_link_mask(&self) -> u8 {
        self.protocol.wii_remote_link_mask()
    }
    pub fn stop_search(&mut self) {
        self.protocol.stop_search();
    }
    pub fn send_report(&mut self, slot: usize, report: &[u8]) -> bool {
        self.protocol.send_report(slot, report)
    }
    pub fn disconnect(&mut self, slot: usize) {
        self.protocol.disconnect(slot);
    }
    /// Request disconnection; completion is reported through the event stream.
    pub fn request_disconnect(&mut self, slot: usize) -> core::result::Result<(), crate::Failure> {
        self.protocol.request_disconnect(slot)
    }
    /// Return whether the adapter still owns a connection in this slot.
    pub fn has_link(&self, slot: usize) -> bool {
        self.protocol.has_link(slot)
    }
    pub fn next_event(&mut self) -> Option<HidEvent> {
        self.protocol.next_event()
    }
    pub fn peek_event(&self) -> Option<HidEvent> {
        self.protocol.peek_event()
    }
    pub fn shutdown(&mut self, now: u64) {
        self.closing = true;
        self.close_deadline = now + IO_TIMEOUT_MS;
        self.open.take(); // A late open owns and closes its descriptor independently.
        for slot in 0..HID_SLOTS {
            self.protocol.disconnect(slot);
        }
        if let Some(adapter) = self.adapter.as_ref() {
            // The reset packet owns its memory until completion. IOS orders this
            // control request before the close request on the same descriptor.
            if let Err(error) = adapter.send_hci_command_async_owned(HciOpcode::RESET, &[]) {
                let result = match error {
                    Error::Ipc(result) => result,
                    _ => -4,
                };
                self.protocol.fail_transport(result);
            }
        }
        self.submit_close();
    }
    fn submit_close(&mut self) {
        if self.close.is_some() {
            return;
        }
        if let Some(adapter) = self.adapter.as_ref() {
            match adapter.close_async() {
                Ok(request) => {
                    self.close = Some(request);
                    self.adapter.take();
                }
                Err(Error::Ipc(result)) => self.protocol.fail_transport(result),
                Err(_) => self.protocol.fail_transport(-4),
            }
        }
        // Pending read/write contexts remain owned by IOS, even after detach.
        self.event_read.take();
        self.acl_read.take();
        self.writes.clear();
    }
    pub fn shutdown_complete(&self) -> bool {
        self.closing && self.adapter.is_none() && self.close.is_none()
    }
    pub fn poll(&mut self, now: u64) {
        if self.closing {
            self.submit_close();
            if let Some(request) = self.close.as_ref() {
                if let Some(result) = request.raw_result() {
                    if result < 0 {
                        self.protocol.fail_transport(result);
                    }
                    self.close.take();
                } else if now >= self.close_deadline {
                    self.protocol.fail_timeout();
                }
            }
            return;
        }
        if let Some(open) = self.open.as_ref() {
            match open.take_host() {
                Some(Ok(host)) => {
                    self.open.take();
                    self.adapter = Some(host);
                    self.protocol.start();
                }
                Some(Err(Error::Ipc(result))) => {
                    self.open.take();
                    self.protocol.fail_transport(result);
                }
                Some(Err(_)) => {
                    self.open.take();
                    self.protocol.fail_transport(-4);
                }
                None if now >= self.opening_deadline => {
                    self.open.take();
                    self.protocol.fail_timeout();
                }
                None => return,
            }
        }
        self.protocol.tick(now);
        let mut failure = None;
        self.writes
            .retain(|(request, deadline)| match request.result() {
                Some(result) => {
                    if result < 0 {
                        failure = Some(result);
                    }
                    false
                }
                None if now >= *deadline => {
                    failure = Some(-116);
                    false
                }
                None => true,
            });
        if let Some(result) = failure {
            self.protocol.fail_transport(result);
        }
        for event in [true, false] {
            if !self.protocol.has_event_capacity() {
                // Keep completed reads and their bytes until the consumer has
                // room. Either HCI or ACL input can publish protocol events.
                continue;
            }
            let pending = if event {
                &mut self.event_read
            } else {
                &mut self.acl_read
            };
            if let Some(request) = pending.as_ref() {
                let mut bytes = [0; 1024];
                if let Some(result) = request.copy_to(&mut bytes) {
                    pending.take();
                    match result {
                        Ok(0) => {}
                        Ok(used) if event => self.protocol.receive_event(&bytes[..used], now),
                        Ok(used) => self.protocol.receive_acl(&bytes[..used]),
                        Err(Error::Ipc(result)) => self.protocol.fail_transport(result),
                        Err(_) => self.protocol.fail_transport(-4),
                    }
                }
            }
        }
        if self.protocol.diagnostics().stage == HostStage::Failed {
            self.submit_close();
            return;
        }
        let Some(adapter) = self.adapter.as_ref() else {
            return;
        };
        // Keep both reads active during startup, pairing, and normal input.
        if self.event_read.is_none() && self.protocol.has_event_capacity() {
            match adapter.read_event_async() {
                Ok(r) => self.event_read = Some(r),
                Err(e) => {
                    self.io_error(e);
                    return;
                }
            }
        }
        if self.acl_read.is_none() && self.protocol.has_event_capacity() {
            match adapter.read_acl_async() {
                Ok(r) => self.acl_read = Some(r),
                Err(e) => {
                    self.io_error(e);
                    return;
                }
            }
        }
        for _ in 0..4 {
            if self.writes.len() >= MAX_WRITES {
                break;
            }
            let Some(operation) = self.protocol.next_outgoing(now) else {
                break;
            };
            let result = match operation {
                Outgoing::Hci { opcode, params } => adapter
                    .send_hci_command_async_owned(opcode, &params)
                    .map(WriteRequest::Hci),
                Outgoing::Acl(data) => adapter.write_acl_async(&data).map(WriteRequest::Acl),
            };
            match result {
                Ok(r) => self.writes.push((r, now + IO_TIMEOUT_MS)),
                Err(e) => {
                    self.io_error(e);
                    break;
                }
            }
        }
    }
    fn io_error(&mut self, error: Error) {
        let result = match error {
            Error::Ipc(result) => result,
            _ => -4,
        };
        self.protocol.fail_transport(result);
    }
}

#[cfg(test)]
mod tests {
    use super::WiiHidHost;

    #[test]
    fn new_host_has_no_active_wii_remote_links() {
        let mut host = WiiHidHost::new();
        assert_eq!(host.wii_remote_link_mask(), 0);
        assert!(!host.search_wii_guest(0));
    }
}

#[cfg(test)]
mod tests {
    use super::WiiHidHost;

    #[test]
    fn has_link_is_false_before_a_controller_connects() {
        let host = WiiHidHost::new();
        assert!(!host.has_link(0));
        assert!(!host.has_link(usize::MAX));
    }
}
