//! Bounded ordered Bluetooth HID input events shared by the producer and
//! controller manager. A full queue applies backpressure; it never drops an
//! accepted input event.

use crate::{BluetoothAddress, ControllerFamily, HID_SLOTS, switch::SwitchCalibration};

/// Maximum report payload accepted by the Bluetooth input queue.
pub const BLUETOOTH_REPORT_MAX_BYTES: usize = 512;
/// Number of ordered input events the queue can hold before applying backpressure.
pub const BLUETOOTH_INPUT_QUEUE_CAPACITY: usize = 64;

/// An accepted connection, report, or disconnection in producer order.
/// The fixed report storage is outside the event tag. This keeps the queue
/// allocation-free without an enum whose variants have very different sizes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct BluetoothInputEvent {
    pub slot: usize,
    pub generation: u32,
    pub kind: BluetoothInputKind,
    pub bytes: [u8; BLUETOOTH_REPORT_MAX_BYTES],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BluetoothInputKind {
    Connected {
        address: BluetoothAddress,
        handle: u16,
        family: ControllerFamily,
        calibration: Option<SwitchCalibration>,
    },
    Report {
        len: u16,
    },
    Disconnected {
        reason: u8,
    },
}

/// The reason an event could not be added to the input queue.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BluetoothInputQueueError {
    Full,
    InvalidSlot,
    AlreadyConnected,
    NotConnected,
    StaleGeneration,
    ReportTooLarge,
    GenerationExhausted,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct SlotState {
    generation: u32,
    connected: bool,
}

/// Fixed-capacity, allocation-free FIFO for Bluetooth controller input.
///
/// Enqueue operations update slot state only after they can append the event.
/// Callers can therefore retain the source event and retry after the consumer
/// frees capacity. A generation identifies one connection lifetime in a slot.
pub struct BluetoothInputQueue {
    events: [Option<BluetoothInputEvent>; BLUETOOTH_INPUT_QUEUE_CAPACITY],
    head: usize,
    len: usize,
    slots: [SlotState; HID_SLOTS],
}

impl Default for BluetoothInputQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl BluetoothInputQueue {
    pub const fn new() -> Self {
        Self {
            events: [None; BLUETOOTH_INPUT_QUEUE_CAPACITY],
            head: 0,
            len: 0,
            slots: [SlotState {
                generation: 0,
                connected: false,
            }; HID_SLOTS],
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn is_full(&self) -> bool {
        self.len == BLUETOOTH_INPUT_QUEUE_CAPACITY
    }

    /// Return the active connection generation for a slot.
    pub fn active_generation(&self, slot: usize) -> Option<u32> {
        self.slots
            .get(slot)
            .filter(|state| state.connected)
            .map(|state| state.generation)
    }

    /// Queue a new connection and return its nonzero generation.
    ///
    /// A reconnect always starts a new generation, even when a previous
    /// disconnect has not yet been consumed. FIFO order preserves that
    /// disconnect before this connection event.
    pub fn push_connected(
        &mut self,
        slot: usize,
        address: BluetoothAddress,
        handle: u16,
        family: ControllerFamily,
        calibration: Option<SwitchCalibration>,
    ) -> Result<u32, BluetoothInputQueueError> {
        let state = self
            .slots
            .get(slot)
            .copied()
            .ok_or(BluetoothInputQueueError::InvalidSlot)?;
        if self.is_full() {
            return Err(BluetoothInputQueueError::Full);
        }
        if state.connected {
            return Err(BluetoothInputQueueError::AlreadyConnected);
        }
        let generation = state
            .generation
            .checked_add(1)
            .filter(|&value| value != 0)
            .ok_or(BluetoothInputQueueError::GenerationExhausted)?;
        self.push(BluetoothInputEvent {
            slot,
            generation,
            kind: BluetoothInputKind::Connected {
                address,
                handle,
                family,
                calibration,
            },
            bytes: [0; BLUETOOTH_REPORT_MAX_BYTES],
        });
        self.slots[slot] = SlotState {
            generation,
            connected: true,
        };
        Ok(generation)
    }

    /// Queue a report only for the currently active connection generation.
    pub fn push_report(
        &mut self,
        slot: usize,
        generation: u32,
        report: &[u8],
    ) -> Result<(), BluetoothInputQueueError> {
        if report.len() > BLUETOOTH_REPORT_MAX_BYTES {
            return Err(BluetoothInputQueueError::ReportTooLarge);
        }
        let state = self
            .slots
            .get(slot)
            .copied()
            .ok_or(BluetoothInputQueueError::InvalidSlot)?;
        if !state.connected {
            return Err(BluetoothInputQueueError::NotConnected);
        }
        if state.generation != generation {
            return Err(BluetoothInputQueueError::StaleGeneration);
        }
        if self.is_full() {
            return Err(BluetoothInputQueueError::Full);
        }

        let mut bytes = [0; BLUETOOTH_REPORT_MAX_BYTES];
        bytes[..report.len()].copy_from_slice(report);
        self.push(BluetoothInputEvent {
            slot,
            generation,
            kind: BluetoothInputKind::Report {
                len: report.len() as u16,
            },
            bytes,
        });
        Ok(())
    }

    /// Queue a disconnection for the currently active connection generation.
    pub fn push_disconnected(
        &mut self,
        slot: usize,
        generation: u32,
        reason: u8,
    ) -> Result<(), BluetoothInputQueueError> {
        let state = self
            .slots
            .get(slot)
            .copied()
            .ok_or(BluetoothInputQueueError::InvalidSlot)?;
        if !state.connected {
            return Err(BluetoothInputQueueError::NotConnected);
        }
        if state.generation != generation {
            return Err(BluetoothInputQueueError::StaleGeneration);
        }
        if self.is_full() {
            return Err(BluetoothInputQueueError::Full);
        }

        self.push(BluetoothInputEvent {
            slot,
            generation,
            kind: BluetoothInputKind::Disconnected { reason },
            bytes: [0; BLUETOOTH_REPORT_MAX_BYTES],
        });
        self.slots[slot].connected = false;
        Ok(())
    }

    /// Remove the oldest event.
    pub fn pop(&mut self) -> Option<BluetoothInputEvent> {
        if self.len == 0 {
            return None;
        }
        let event = self.events[self.head].take();
        self.head = (self.head + 1) % BLUETOOTH_INPUT_QUEUE_CAPACITY;
        self.len -= 1;
        event
    }

    fn push(&mut self, event: BluetoothInputEvent) {
        debug_assert!(!self.is_full());
        let tail = (self.head + self.len) % BLUETOOTH_INPUT_QUEUE_CAPACITY;
        debug_assert!(self.events[tail].is_none());
        self.events[tail] = Some(event);
        self.len += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(last: u8) -> BluetoothAddress {
        BluetoothAddress::new([0, 1, 2, 3, 4, last])
    }

    fn connect(queue: &mut BluetoothInputQueue, slot: usize, family: ControllerFamily) -> u32 {
        queue
            .push_connected(slot, address(slot as u8), slot as u16 + 1, family, None)
            .unwrap()
    }

    #[test]
    fn preserves_paired_controller_report_order_between_scans() {
        let mut queue = BluetoothInputQueue::new();
        let _left = connect(&mut queue, 0, ControllerFamily::JoyConLeft);
        let right = connect(&mut queue, 1, ControllerFamily::JoyConRight);
        queue.push_report(1, right, &[0x30, 1]).unwrap();
        queue.push_report(1, right, &[0x30, 0]).unwrap();

        assert!(matches!(
            queue.pop(),
            Some(BluetoothInputEvent {
                slot: 0,
                kind: BluetoothInputKind::Connected { .. },
                ..
            })
        ));
        assert!(matches!(
            queue.pop(),
            Some(BluetoothInputEvent {
                slot: 1,
                kind: BluetoothInputKind::Connected { .. },
                ..
            })
        ));
        assert_eq!(
            queue.pop(),
            Some(BluetoothInputEvent {
                slot: 1,
                generation: right,
                kind: BluetoothInputKind::Report { len: 2 },
                bytes: {
                    let mut bytes = [0; BLUETOOTH_REPORT_MAX_BYTES];
                    bytes[..2].copy_from_slice(&[0x30, 1]);
                    bytes
                },
            })
        );
        assert_eq!(
            queue.pop(),
            Some(BluetoothInputEvent {
                slot: 1,
                generation: right,
                kind: BluetoothInputKind::Report { len: 2 },
                bytes: {
                    let mut bytes = [0; BLUETOOTH_REPORT_MAX_BYTES];
                    bytes[..2].copy_from_slice(&[0x30, 0]);
                    bytes
                },
            })
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn generations_fence_reports_after_disconnect_and_reconnect() {
        let mut queue = BluetoothInputQueue::new();
        let first = connect(&mut queue, 2, ControllerFamily::SwitchPro);
        queue.push_disconnected(2, first, 0x13).unwrap();
        let second = connect(&mut queue, 2, ControllerFamily::SwitchPro);
        assert_ne!(first, second);

        assert_eq!(
            queue.push_report(2, first, &[0x30]),
            Err(BluetoothInputQueueError::StaleGeneration)
        );
        assert_eq!(queue.active_generation(2), Some(second));
        assert!(
            matches!(queue.pop(), Some(BluetoothInputEvent { generation, kind: BluetoothInputKind::Connected { .. }, .. }) if generation == first)
        );
        assert!(
            matches!(queue.pop(), Some(BluetoothInputEvent { generation, kind: BluetoothInputKind::Disconnected { .. }, .. }) if generation == first)
        );
        assert!(
            matches!(queue.pop(), Some(BluetoothInputEvent { generation, kind: BluetoothInputKind::Connected { .. }, .. }) if generation == second)
        );
    }

    #[test]
    fn generations_are_independent_per_slot_and_duplicate_connect_is_rejected() {
        let mut queue = BluetoothInputQueue::new();
        let first = connect(&mut queue, 0, ControllerFamily::JoyConLeft);
        let other = connect(&mut queue, 1, ControllerFamily::JoyConRight);
        assert_eq!(first, other);
        assert_eq!(
            queue.push_connected(0, address(9), 10, ControllerFamily::JoyConLeft, None,),
            Err(BluetoothInputQueueError::AlreadyConnected)
        );
        assert_eq!(queue.active_generation(0), Some(first));
        assert_eq!(queue.active_generation(1), Some(other));

        queue.push_disconnected(0, first, 0).unwrap();
        let next = connect(&mut queue, 0, ControllerFamily::JoyConLeft);
        assert_eq!(next, first + 1);
        assert_eq!(queue.active_generation(1), Some(other));
    }

    #[test]
    fn rejects_reports_without_a_live_connection() {
        let mut queue = BluetoothInputQueue::new();
        assert_eq!(
            queue.push_report(0, 1, &[1]),
            Err(BluetoothInputQueueError::NotConnected)
        );
        assert_eq!(
            queue.push_report(HID_SLOTS, 1, &[1]),
            Err(BluetoothInputQueueError::InvalidSlot)
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn full_queue_backpressures_without_changing_connection_state() {
        let mut queue = BluetoothInputQueue::new();
        let generation = connect(&mut queue, 0, ControllerFamily::Xbox);
        for value in 0..(BLUETOOTH_INPUT_QUEUE_CAPACITY - 1) {
            queue.push_report(0, generation, &[value as u8]).unwrap();
        }
        assert!(queue.is_full());

        assert_eq!(
            queue.push_disconnected(0, generation, 0x13),
            Err(BluetoothInputQueueError::Full)
        );
        assert_eq!(queue.active_generation(0), Some(generation));
        assert_eq!(queue.len(), BLUETOOTH_INPUT_QUEUE_CAPACITY);

        queue.pop().unwrap();
        queue.push_disconnected(0, generation, 0x13).unwrap();
        assert_eq!(queue.active_generation(0), None);
    }

    #[test]
    fn report_length_limit_is_transactional() {
        let mut queue = BluetoothInputQueue::new();
        let generation = connect(&mut queue, 3, ControllerFamily::Generic);
        let oversized = [0x55; BLUETOOTH_REPORT_MAX_BYTES + 1];
        assert_eq!(
            queue.push_report(3, generation, &oversized),
            Err(BluetoothInputQueueError::ReportTooLarge)
        );
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active_generation(3), Some(generation));
        assert!(matches!(
            queue.pop(),
            Some(BluetoothInputEvent {
                kind: BluetoothInputKind::Connected { .. },
                ..
            })
        ));
        assert!(queue.is_empty());
    }

    #[test]
    fn connection_event_preserves_switch_calibration() {
        let mut queue = BluetoothInputQueue::new();
        let calibration = SwitchCalibration::default();
        queue
            .push_connected(
                0,
                address(0),
                3,
                ControllerFamily::SwitchPro,
                Some(calibration),
            )
            .unwrap();
        assert!(
            matches!(queue.pop(), Some(BluetoothInputEvent { kind: BluetoothInputKind::Connected { calibration: Some(value), .. }, .. }) if value == calibration)
        );
    }
}
