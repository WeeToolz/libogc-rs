/// Tracks occupied and reserved entries in a bounded interrupt callback queue.
///
/// Each reservation has a token so one producer cannot release another
/// producer's protected queue entry.
#[derive(Copy, Clone)]
pub(crate) struct DeferredSlots<const CAPACITY: usize> {
    len: usize,
    reservations: [u32; CAPACITY],
    reserved: usize,
    next_reservation: u32,
}

impl<const CAPACITY: usize> DeferredSlots<CAPACITY> {
    pub(crate) const fn new() -> Self {
        Self {
            len: 0,
            reservations: [0; CAPACITY],
            reserved: 0,
            next_reservation: 1,
        }
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    #[cfg(test)]
    pub(crate) const fn reserved(&self) -> usize {
        self.reserved
    }

    pub(crate) fn reserve(&mut self) -> Option<u32> {
        if self.len + self.reserved >= CAPACITY {
            return None;
        }
        let token = self.next_available_token()?;
        let slot = self.reservations.iter_mut().find(|token| **token == 0)?;
        *slot = token;
        self.reserved += 1;
        Some(token)
    }

    fn next_available_token(&mut self) -> Option<u32> {
        for _ in 0..=CAPACITY {
            let token = self.next_reservation.max(1);
            self.next_reservation = token.wrapping_add(1).max(1);
            if !self.reservations.contains(&token) {
                return Some(token);
            }
        }
        None
    }

    fn take_reservation(&mut self, token: u32) -> bool {
        let Some(slot) = self.reservations.iter_mut().find(|saved| **saved == token) else {
            return false;
        };
        *slot = 0;
        self.reserved -= 1;
        true
    }

    pub(crate) fn cancel_reservation(&mut self, token: u32) -> bool {
        self.take_reservation(token)
    }

    pub(crate) fn enqueue_unreserved(&mut self) -> bool {
        if self.len + self.reserved >= CAPACITY {
            return false;
        }
        self.len += 1;
        true
    }

    pub(crate) fn enqueue_reserved(&mut self, token: u32) -> bool {
        if self.len >= CAPACITY || !self.take_reservation(token) {
            return false;
        }
        self.len += 1;
        true
    }

    pub(crate) fn dequeue(&mut self) -> bool {
        if self.len == 0 {
            return false;
        }
        self.len -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::DeferredSlots;

    #[test]
    fn reserved_slot_stays_available_when_unreserved_callbacks_fill_the_queue() {
        let mut slots = DeferredSlots::<32>::new();

        let token = slots.reserve().expect("capacity permits reservation");
        for _ in 0..31 {
            assert!(slots.enqueue_unreserved());
        }
        assert!(!slots.enqueue_unreserved());
        assert_eq!(slots.len(), 31);
        assert_eq!(slots.reserved(), 1);

        assert!(slots.enqueue_reserved(token));
        assert_eq!(slots.len(), 32);
        assert_eq!(slots.reserved(), 0);
        for _ in 0..32 {
            assert!(slots.dequeue());
        }
        assert!(!slots.dequeue());
    }

    #[test]
    fn cancellation_returns_reserved_capacity_to_unreserved_producers() {
        let mut slots = DeferredSlots::<1>::new();

        let token = slots.reserve().expect("capacity permits reservation");
        assert!(slots.reserve().is_none());
        assert!(!slots.cancel_reservation(token.wrapping_add(1)));
        assert!(slots.cancel_reservation(token));
        assert!(slots.enqueue_unreserved());
        assert!(!slots.cancel_reservation(token));
    }
}
