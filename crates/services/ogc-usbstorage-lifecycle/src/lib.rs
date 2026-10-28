#![no_std]

//! USB mass-storage close sequencing.

pub mod drives;
pub mod transport;

pub trait CloseOperations {
    fn max_lun(&self) -> u8;
    fn lun_is_mounted(&self, lun: u8) -> bool;
    fn flush_lun(&mut self, lun: u8) -> i32;
    fn close_transport(&mut self) -> i32;
}

pub fn close<O: CloseOperations>(operations: &mut O) -> i32 {
    let mut first_error = 0;
    for lun in 0..operations.max_lun() {
        if !operations.lun_is_mounted(lun) {
            continue;
        }
        let result = operations.flush_lun(lun);
        if result < 0 && first_error == 0 {
            first_error = result;
        }
    }

    let result = operations.close_transport();
    if result < 0 && first_error == 0 {
        first_error = result;
    }
    first_error
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Call {
        Flush(u8),
        Close,
    }

    struct TestOperations {
        mounted: [bool; 4],
        flush_results: [i32; 4],
        close_result: i32,
        calls: Vec<Call>,
    }

    impl CloseOperations for TestOperations {
        fn max_lun(&self) -> u8 {
            self.mounted.len() as u8
        }

        fn lun_is_mounted(&self, lun: u8) -> bool {
            self.mounted[lun as usize]
        }

        fn flush_lun(&mut self, lun: u8) -> i32 {
            self.calls.push(Call::Flush(lun));
            self.flush_results[lun as usize]
        }

        fn close_transport(&mut self) -> i32 {
            self.calls.push(Call::Close);
            self.close_result
        }
    }

    fn operations() -> TestOperations {
        TestOperations {
            mounted: [true, false, true, false],
            flush_results: [0; 4],
            close_result: 0,
            calls: Vec::new(),
        }
    }

    #[test]
    fn flushes_each_mounted_lun_then_closes() {
        let mut operations = operations();

        assert_eq!(close(&mut operations), 0);
        assert_eq!(
            operations.calls,
            [Call::Flush(0), Call::Flush(2), Call::Close]
        );
    }

    #[test]
    fn flush_failure_does_not_prevent_transport_close() {
        let mut operations = operations();
        operations.flush_results[0] = -101;
        operations.flush_results[2] = -202;

        assert_eq!(close(&mut operations), -101);
        assert_eq!(
            operations.calls,
            [Call::Flush(0), Call::Flush(2), Call::Close]
        );
    }

    #[test]
    fn each_flush_boundary_failure_still_closes_transport() {
        for (lun, error) in [(0, -10), (2, -20)] {
            let mut operations = operations();
            operations.flush_results[lun] = error;

            assert_eq!(close(&mut operations), error);
            assert_eq!(operations.calls.last(), Some(&Call::Close));
        }
    }

    #[test]
    fn transport_close_failure_is_returned_after_flushes() {
        let mut operations = operations();
        operations.close_result = -303;

        assert_eq!(close(&mut operations), -303);
        assert_eq!(
            operations.calls,
            [Call::Flush(0), Call::Flush(2), Call::Close]
        );
    }

    #[test]
    fn first_flush_failure_is_preserved_when_later_close_also_fails() {
        let mut operations = operations();
        operations.flush_results[0] = -101;
        operations.close_result = -303;

        assert_eq!(close(&mut operations), -101);
        assert_eq!(
            operations.calls,
            [Call::Flush(0), Call::Flush(2), Call::Close]
        );
    }

    #[test]
    fn handle_without_mounted_luns_still_closes_transport() {
        let mut operations = operations();
        operations.mounted = [false; 4];

        assert_eq!(close(&mut operations), 0);
        assert_eq!(operations.calls, [Call::Close]);
    }

    #[test]
    fn close_operations_report_luns_and_run_requested_actions() {
        let mut operations = operations();

        assert_eq!(operations.max_lun(), 4);
        assert!(operations.lun_is_mounted(0));
        assert!(!operations.lun_is_mounted(1));
        assert_eq!(operations.flush_lun(0), 0);
        assert_eq!(operations.close_transport(), 0);
        assert_eq!(operations.calls, [Call::Flush(0), Call::Close]);
    }
}

pub mod bulk;
pub mod deinitialize;
