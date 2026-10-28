//! Validation used by checked system initialization.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitError {
    NotCompleted,
    FailedStage(u32),
}

/// Validate the production startup snapshot without losing a failed stage.
pub fn validate(completed: bool, failed_stage: u32) -> Result<(), InitError> {
    if failed_stage != 0 {
        Err(InitError::FailedStage(failed_stage))
    } else if !completed {
        Err(InitError::NotCompleted)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_sequence_with_failed_stage_is_not_success() {
        assert_eq!(validate(true, 6), Err(InitError::FailedStage(6)));
        assert_eq!(validate(false, 3), Err(InitError::FailedStage(3)));
        assert_eq!(validate(false, 0), Err(InitError::NotCompleted));
        assert_eq!(validate(true, 0), Ok(()));
    }
}
