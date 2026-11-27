//! USB transport state changes shared by the driver and host failure tests.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotResetStep {
    ClassReset,
    DelayMs(u64),
    ClearInHalt,
    ClearOutHalt,
}

/// Complete reset recovery and return the first failed operation.
pub fn reset(mut run: impl FnMut(BotResetStep) -> i32) -> i32 {
    let mut result = run(BotResetStep::ClassReset);
    for step in [
        BotResetStep::DelayMs(60),
        BotResetStep::ClearInHalt,
        BotResetStep::DelayMs(10),
        BotResetStep::ClearOutHalt,
        BotResetStep::DelayMs(10),
    ] {
        let next_result = run(step);
        if result >= 0 && next_result < 0 {
            result = next_result;
        }
    }
    result
}

/// Commit the suspended state only after the USB request succeeds.
pub fn suspend(suspended: &mut u8, fd: i32, suspend_device: impl FnOnce(i32) -> i32) -> i32 {
    if *suspended == 1 {
        return 0;
    }
    let result = suspend_device(fd);
    if result < 0 {
        return result;
    }
    *suspended = 1;
    0
}

/// A malformed Bulk-Only Transport command status wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CswError {
    Signature,
    Tag,
}

/// Decode the 13-byte little-endian command status wrapper.
pub fn decode_csw(
    csw: &[u8; 13],
    expected_tag: u32,
    status: &mut u8,
    residue: &mut u32,
) -> Result<(), CswError> {
    let signature = u32::from_le_bytes(csw[0..4].try_into().unwrap());
    let tag = u32::from_le_bytes(csw[4..8].try_into().unwrap());
    // C publishes the response only after validating the signature, but
    // before validating the tag. Keep the previous outputs on bad signature.
    if signature != 0x5342_5355 {
        return Err(CswError::Signature);
    }
    *residue = u32::from_le_bytes(csw[8..12].try_into().unwrap());
    *status = csw[12];
    if tag != expected_tag {
        return Err(CswError::Tag);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    const STEPS: [BotResetStep; 6] = [
        BotResetStep::ClassReset,
        BotResetStep::DelayMs(60),
        BotResetStep::ClearInHalt,
        BotResetStep::DelayMs(10),
        BotResetStep::ClearOutHalt,
        BotResetStep::DelayMs(10),
    ];

    #[test]
    fn csw_validates_signature_before_copying_and_tag_after_copying() {
        let mut csw = [0u8; 13];
        csw[0..4].copy_from_slice(&0x5342_5354u32.to_le_bytes());
        csw[4..8].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        csw[8..12].copy_from_slice(&0x1122_3344u32.to_le_bytes());
        csw[12] = 1;
        let mut status = 0xa5;
        let mut residue = 0xa5a5_a5a5;
        assert_eq!(
            decode_csw(&csw, 0, &mut status, &mut residue),
            Err(CswError::Signature)
        );
        assert_eq!((status, residue), (0xa5, 0xa5a5_a5a5));
        csw[0..4].copy_from_slice(&0x5342_5355u32.to_le_bytes());
        assert_eq!(
            decode_csw(&csw, 0, &mut status, &mut residue),
            Err(CswError::Tag)
        );
        assert_eq!((status, residue), (1, 0x1122_3344));
        assert_eq!(
            decode_csw(&csw, 0x1234_5678, &mut status, &mut residue),
            Ok(())
        );
    }

    #[test]
    fn reset_reports_each_failure_and_completes_recovery_in_order() {
        for failure in 0..STEPS.len() {
            let mut calls = Vec::new();
            let result = reset(|step| {
                calls.push(step);
                if calls.len() - 1 == failure { -55 } else { 0 }
            });
            assert_eq!(result, -55);
            assert_eq!(calls, STEPS);
        }
    }

    #[test]
    fn reset_preserves_the_first_error() {
        let mut calls = 0;
        assert_eq!(
            reset(|_| {
                calls += 1;
                -40 - calls
            }),
            -41
        );
        assert_eq!(calls, 6);
    }

    #[test]
    fn reset_success_requires_all_recovery_steps() {
        let mut calls = Vec::new();
        assert_eq!(
            reset(|step| {
                calls.push(step);
                0
            }),
            0
        );
        assert_eq!(calls, STEPS);
    }

    #[test]
    fn suspend_failure_preserves_state_and_allows_retry() {
        let mut suspended = 0;
        assert_eq!(
            suspend(&mut suspended, 23, |fd| {
                assert_eq!(fd, 23);
                -55
            }),
            -55
        );
        assert_eq!(suspended, 0);
        assert_eq!(
            suspend(&mut suspended, 23, |fd| {
                assert_eq!(fd, 23);
                0
            }),
            0
        );
        assert_eq!(suspended, 1);
        assert_eq!(
            suspend(&mut suspended, 23, |_| panic!("already suspended")),
            0
        );
    }
}
