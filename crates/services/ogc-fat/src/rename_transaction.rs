//! Commit and recovery ordering shared by the production FAT rename operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    PublishDestination,
    ChangeParent,
    RemoveSource,
    RestoreSource,
    RestoreParent,
    RestoreDestination,
    FlushRecovery,
}

/// Keep the destination cluster chain allocated until the source name is gone.
/// If recovery fails, keep the remaining alias instead of freeing live data.
pub fn commit<E>(
    parent_changed: bool,
    mut operation: impl FnMut(Step) -> Result<(), E>,
) -> Result<(), E> {
    let changed = (|| {
        operation(Step::PublishDestination)?;
        if parent_changed {
            operation(Step::ChangeParent)?;
        }
        operation(Step::RemoveSource)
    })();
    if let Err(error) = changed {
        operation(Step::RestoreSource)?;
        if parent_changed {
            operation(Step::RestoreParent)?;
        }
        operation(Step::RestoreDestination)?;
        operation(Step::FlushRecovery)?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Explicit controlled transport. The bytes are directory references, parent
    // cluster, and allocation bits. A failure can occur after a partial write.
    #[test]
    fn successful_commit_preserves_data_references_and_changes_parent() {
        let mut disk = [7, 9, 1, 0b11];
        assert_eq!(
            commit(true, |step| {
                match step {
                    Step::PublishDestination => disk[1] = 7,
                    Step::ChangeParent => disk[2] = 2,
                    Step::RemoveSource => disk[0] = 0,
                    _ => panic!("unexpected recovery"),
                }
                Ok::<_, u8>(())
            }),
            Ok(())
        );
        let persisted = disk.to_vec();
        assert_eq!(persisted.as_slice(), &[0, 7, 2, 0b11]);
    }
    #[test]
    fn every_commit_failure_restores_persisted_references() {
        for failure in [
            Step::PublishDestination,
            Step::ChangeParent,
            Step::RemoveSource,
        ] {
            let mut disk = [7, 9, 1, 0b11];
            let result = commit(true, |step| {
                match step {
                    Step::PublishDestination => disk[1] = 7,
                    Step::ChangeParent => disk[2] = 2,
                    Step::RemoveSource => disk[0] = 0,
                    Step::RestoreSource => disk[0] = 7,
                    Step::RestoreParent => disk[2] = 1,
                    Step::RestoreDestination => disk[1] = 9,
                    Step::FlushRecovery => {}
                }
                if step == failure { Err(19) } else { Ok(()) }
            });
            assert_eq!(result, Err(19));
            let persisted = disk.to_vec();
            assert_eq!(persisted.as_slice(), &[7, 9, 1, 0b11]);
        }
    }
    #[test]
    fn failed_recovery_is_visible_and_keeps_an_alias_to_source_data() {
        let mut disk = [7, 9, 1, 0b11];
        assert_eq!(
            commit(false, |step| {
                match step {
                    Step::PublishDestination => disk[1] = 7,
                    Step::RemoveSource => {
                        disk[0] = 0;
                        return Err(19);
                    }
                    Step::RestoreSource => return Err(5),
                    _ => panic!("continued after recovery failure"),
                }
                Ok(())
            }),
            Err(5)
        );
        assert_eq!(disk, [0, 7, 1, 0b11]);
    }
}
