//! The USB Gecko command is one immediate transfer with one completion wait.

pub(crate) trait Transport {
    fn select(&mut self) -> bool;
    fn start(&mut self, command: &mut u16) -> bool;
    fn sync(&mut self) -> bool;
    fn deselect(&mut self) -> bool;
}

pub(crate) fn send(transport: &mut impl Transport, command: &mut u16) -> bool {
    if !transport.select() {
        return false;
    }
    let started = transport.start(command);
    let completed = started && transport.sync();
    let deselected = transport.deselect();
    completed && deselected
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bus {
        pending: bool,
        waits: u32,
        deselected: bool,
        fail_start: bool,
        fail_sync: bool,
        fail_deselect: bool,
    }

    impl Transport for Bus {
        fn select(&mut self) -> bool {
            true
        }
        fn start(&mut self, command: &mut u16) -> bool {
            assert_eq!(*command, 0xc000);
            self.pending = !self.fail_start;
            *command = 0x0400;
            self.pending
        }
        fn sync(&mut self) -> bool {
            self.waits += 1;
            let completed = self.pending && !self.fail_sync;
            self.pending = false;
            completed
        }
        fn deselect(&mut self) -> bool {
            self.deselected = true;
            !self.fail_deselect
        }
    }

    #[test]
    fn command_waits_once_and_returns_the_device_reply() {
        let mut bus = Bus {
            pending: false,
            waits: 0,
            deselected: false,
            fail_start: false,
            fail_sync: false,
            fail_deselect: false,
        };
        let mut command = 0xc000;
        assert!(send(&mut bus, &mut command));
        assert_eq!(command, 0x0400);
        assert_eq!(bus.waits, 1);
        assert!(bus.deselected);
        assert!(!bus.pending);
    }

    #[test]
    fn transfer_and_cleanup_failures_are_not_success() {
        for failure in 0..3 {
            let mut bus = Bus {
                pending: false,
                waits: 0,
                deselected: false,
                fail_start: failure == 0,
                fail_sync: failure == 1,
                fail_deselect: failure == 2,
            };
            assert!(!send(&mut bus, &mut 0xc000));
            assert!(bus.deselected);
            assert_eq!(bus.waits, u32::from(failure != 0));
        }
    }
}
