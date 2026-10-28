//! EXI RTC transaction rules. Production supplies the real EXI operations.

use crate::clock::{ClockError, RtcStage};

pub(crate) trait RtcTransport {
    fn lock(&mut self) -> bool;
    fn select(&mut self) -> bool;
    fn command(&mut self) -> bool;
    fn sync(&mut self) -> bool;
    fn read(&mut self, value: &mut u32) -> bool;
    fn deselect(&mut self) -> bool;
    fn unlock(&mut self) -> bool;
}

pub(crate) fn read(transport: &mut impl RtcTransport) -> Result<u32, ClockError> {
    if !transport.lock() {
        return Err(ClockError::RtcFailure {
            stage: RtcStage::Lock,
            cleanup_failures: 0,
        });
    }
    let selected = transport.select();
    let mut value = 0;
    let failed = if !selected {
        Some(RtcStage::Select)
    } else if !transport.command() {
        Some(RtcStage::Write)
    } else if !transport.sync() {
        Some(RtcStage::WriteSync)
    } else if !transport.read(&mut value) {
        Some(RtcStage::Read)
    } else if !transport.sync() {
        Some(RtcStage::ReadSync)
    } else {
        None
    };
    let deselect_failed = selected && !transport.deselect();
    let unlock_failed = !transport.unlock();
    let cleanup_failures = u8::from(deselect_failed) | (u8::from(unlock_failed) << 1);
    let failed = failed.or(if deselect_failed {
        Some(RtcStage::Deselect)
    } else if unlock_failed {
        Some(RtcStage::Unlock)
    } else {
        None
    });
    match failed {
        Some(stage) => Err(ClockError::RtcFailure {
            stage,
            cleanup_failures,
        }),
        None => Ok(value),
    }
}

pub(crate) fn consistent(
    mut read: impl FnMut() -> Result<u32, ClockError>,
) -> Result<u32, ClockError> {
    for _ in 0..16 {
        let first = read()?;
        let second = read()?;
        if first == second {
            return Ok(first);
        }
    }
    Err(ClockError::RtcUnstable)
}

#[cfg(test)]
mod tests {
    use super::*;

    // An explicit transport fault fixture exercises the production transaction
    // engine. It supplies a fixed counter and fails only named EXI boundaries.
    struct FaultTransport {
        failed: Vec<RtcStage>,
        calls: Vec<RtcStage>,
        syncs: u8,
    }
    impl FaultTransport {
        fn step(&mut self, stage: RtcStage) -> bool {
            self.calls.push(stage);
            !self.failed.contains(&stage)
        }
    }
    impl RtcTransport for FaultTransport {
        fn lock(&mut self) -> bool {
            self.step(RtcStage::Lock)
        }
        fn select(&mut self) -> bool {
            self.step(RtcStage::Select)
        }
        fn command(&mut self) -> bool {
            self.step(RtcStage::Write)
        }
        fn sync(&mut self) -> bool {
            self.syncs += 1;
            self.step(if self.syncs == 1 {
                RtcStage::WriteSync
            } else {
                RtcStage::ReadSync
            })
        }
        fn read(&mut self, value: &mut u32) -> bool {
            if !self.step(RtcStage::Read) {
                return false;
            }
            *value = 0x1357_2468;
            true
        }
        fn deselect(&mut self) -> bool {
            self.step(RtcStage::Deselect)
        }
        fn unlock(&mut self) -> bool {
            self.step(RtcStage::Unlock)
        }
    }
    fn transport(failed: Vec<RtcStage>) -> FaultTransport {
        FaultTransport {
            failed,
            calls: Vec::new(),
            syncs: 0,
        }
    }

    #[test]
    fn successful_read_checks_final_completion_and_releases_exi() {
        let mut bus = transport(vec![]);
        assert_eq!(read(&mut bus), Ok(0x1357_2468));
        assert_eq!(
            bus.calls,
            [
                RtcStage::Lock,
                RtcStage::Select,
                RtcStage::Write,
                RtcStage::WriteSync,
                RtcStage::Read,
                RtcStage::ReadSync,
                RtcStage::Deselect,
                RtcStage::Unlock
            ]
        );
    }

    #[test]
    fn each_operation_failure_is_returned_with_required_cleanup() {
        for stage in [
            RtcStage::Lock,
            RtcStage::Select,
            RtcStage::Write,
            RtcStage::WriteSync,
            RtcStage::Read,
            RtcStage::ReadSync,
            RtcStage::Deselect,
            RtcStage::Unlock,
        ] {
            let mut bus = transport(vec![stage]);
            let cleanup_failures = match stage {
                RtcStage::Deselect => 1,
                RtcStage::Unlock => 2,
                _ => 0,
            };
            assert_eq!(
                read(&mut bus),
                Err(ClockError::RtcFailure {
                    stage,
                    cleanup_failures
                })
            );
            if stage == RtcStage::Lock {
                assert_eq!(bus.calls, [RtcStage::Lock]);
            } else {
                assert_eq!(bus.calls.last(), Some(&RtcStage::Unlock));
            }
            if stage == RtcStage::Select {
                assert!(!bus.calls.contains(&RtcStage::Deselect));
            }
        }
    }

    #[test]
    fn cleanup_failures_do_not_hide_the_transport_failure() {
        let mut bus = transport(vec![
            RtcStage::ReadSync,
            RtcStage::Deselect,
            RtcStage::Unlock,
        ]);
        assert_eq!(
            read(&mut bus),
            Err(ClockError::RtcFailure {
                stage: RtcStage::ReadSync,
                cleanup_failures: 3,
            })
        );
        let mut bus = transport(vec![RtcStage::Select, RtcStage::Unlock]);
        assert_eq!(
            read(&mut bus),
            Err(ClockError::RtcFailure {
                stage: RtcStage::Select,
                cleanup_failures: 2,
            })
        );
    }

    #[test]
    fn consistency_is_bounded_and_preserves_read_errors() {
        let mut reads = [Ok(119), Ok(120), Ok(120), Ok(120)].into_iter();
        assert_eq!(consistent(|| reads.next().unwrap()), Ok(120));
        let mut count = 0;
        assert_eq!(
            consistent(|| {
                count += 1;
                Ok(count)
            }),
            Err(ClockError::RtcUnstable)
        );
        assert_eq!(count, 32);
        assert_eq!(consistent(|| Err(ClockError::Busy)), Err(ClockError::Busy));
    }
}
