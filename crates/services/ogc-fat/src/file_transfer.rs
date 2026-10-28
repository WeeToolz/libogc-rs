//! Sector transfer accounting shared by FAT read and write operations.
//!
//! A failure before any transfer is returned. A failure after a completed
//! prefix returns that prefix's byte count, as required for partial file I/O.
pub fn transfer<E>(
    length: usize,
    invalid: E,
    mut step: impl FnMut(usize) -> Result<usize, E>,
) -> Result<usize, E> {
    let mut completed = 0;
    while completed < length {
        let count = match step(completed) {
            Ok(count) => count,
            Err(error) => {
                return if completed == 0 {
                    Err(error)
                } else {
                    Ok(completed)
                };
            }
        };
        if count == 0 || count > length - completed {
            return Err(invalid);
        }
        completed += count;
    }
    Ok(completed)
}

/// Clear dirty state only after metadata and transport operations complete.
pub fn flush<E>(
    dirty: &mut u32,
    online: impl FnOnce() -> Result<(), E>,
    metadata: impl FnOnce() -> Result<(), E>,
    transport: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    online()?;
    if *dirty != 0 {
        metadata()?;
    }
    transport()?;
    *dirty = 0;
    Ok(())
}

/// Always release ownership after a close attempt, preserving the flush error.
pub fn close<T, E>(
    state: &mut T,
    flush: impl FnOnce(&mut T) -> Result<(), E>,
    release: impl FnOnce(&mut T),
) -> Result<(), E> {
    let result = flush(state);
    release(state);
    result
}

/// Keep a new start cluster pending in metadata even if a later operation fails.
pub fn allocation<T, E>(
    start: &mut u32,
    dirty: &mut u32,
    operation: impl FnOnce(&mut u32) -> Result<T, E>,
) -> Result<T, E> {
    let previous = *start;
    let result = operation(start);
    if *start != previous {
        *dirty = 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{allocation, close, flush, transfer};
    #[test]
    fn read_failure_returns_only_completed_bytes_and_cursor() {
        // Explicit controlled medium. The production FAT loop uses this same engine.
        let persisted = [10, 20, 30, 40, 50, 60];
        let mut output = [0xcc; 6];
        let mut cursor = 0;
        let result = transfer(6, "invalid", |done| {
            if done == 4 {
                return Err("media removed");
            }
            output[done..done + 2].copy_from_slice(&persisted[cursor..cursor + 2]);
            cursor += 2;
            Ok(2)
        });
        assert_eq!(result, Ok(4));
        assert_eq!(cursor, 4);
        assert_eq!(output, [10, 20, 30, 40, 0xcc, 0xcc]);
        assert_eq!(
            transfer(2, "invalid", |_| Err("media removed")),
            Err("media removed")
        );
    }
    #[test]
    fn write_failure_preserves_completed_bytes_after_medium_reload() {
        let input = [1, 2, 3, 4, 5, 6];
        let mut persisted = [0; 6];
        let mut cursor = 0;
        let result = transfer(6, "invalid", |done| {
            if done == 4 {
                return Err("write failed");
            }
            persisted[cursor..cursor + 2].copy_from_slice(&input[done..done + 2]);
            cursor += 2;
            Ok(2)
        });
        assert_eq!(result, Ok(4));
        assert_eq!(cursor, 4);
        let reopened_medium = persisted;
        assert_eq!(reopened_medium, [1, 2, 3, 4, 0, 0]);
    }
    #[test]
    fn first_failure_does_not_become_success() {
        assert_eq!(
            transfer(4, "invalid", |_| Err("transport error")),
            Err("transport error")
        );
    }
    #[test]
    fn rejects_zero_and_excessive_transfer_counts() {
        assert_eq!(transfer(4, "invalid", |_| Ok(0)), Err("invalid"));
        assert_eq!(transfer(4, "invalid", |_| Ok(5)), Err("invalid"));
    }
    #[test]
    fn empty_io_does_not_use_the_transport() {
        assert_eq!(
            transfer(0, "invalid", |_| panic!("unexpected transfer")),
            Ok(0)
        );
    }

    #[test]
    fn clean_file_still_reports_transport_flush_failure() {
        let mut dirty = 0;
        let result = flush(
            &mut dirty,
            || Ok(()),
            || panic!("clean metadata"),
            || Err("flush failed"),
        );
        assert_eq!(result, Err("flush failed"));
        assert_eq!(dirty, 0);
    }

    #[test]
    fn metadata_failure_preserves_dirty_state_and_stops_transport() {
        let mut dirty = 1;
        let result = flush(
            &mut dirty,
            || Ok(()),
            || Err("metadata failed"),
            || panic!("unexpected transport"),
        );
        assert_eq!(result, Err("metadata failed"));
        assert_eq!(dirty, 1);
    }

    #[test]
    fn transport_failure_preserves_dirty_state_for_retry() {
        let mut dirty = 1;
        let mut persisted = [0u8; 4];
        let result = flush(
            &mut dirty,
            || Ok(()),
            || {
                persisted.copy_from_slice(&[7, 8, 9, 10]);
                Ok(())
            },
            || Err("flush failed"),
        );
        assert_eq!(result, Err("flush failed"));
        assert_eq!(dirty, 1);
        assert_eq!(persisted, [7, 8, 9, 10]);
        assert_eq!(
            flush(&mut dirty, || Ok(()), || Ok::<_, &str>(()), || Ok(())),
            Ok(())
        );
        assert_eq!(dirty, 0);
    }

    #[test]
    fn new_allocation_is_not_lost_after_a_later_failure() {
        let mut start = 0;
        let mut dirty = 0;
        let result = allocation(&mut start, &mut dirty, |start| {
            *start = 7;
            Err::<(), _>("later cluster operation failed")
        });
        assert_eq!(result, Err("later cluster operation failed"));
        assert_eq!(start, 7);
        assert_eq!(dirty, 1);
        let mut stored_start = [0; 4];
        assert_eq!(
            flush(
                &mut dirty,
                || Ok(()),
                || {
                    stored_start = start.to_le_bytes();
                    Ok::<_, &str>(())
                },
                || Ok(())
            ),
            Ok(())
        );
        assert_eq!(stored_start, [7, 0, 0, 0]);
    }

    #[test]
    fn failed_allocation_without_a_state_change_preserves_metadata() {
        let mut start = 0;
        let mut dirty = 0;
        assert_eq!(
            allocation(&mut start, &mut dirty, |_| Err::<(), _>("no space")),
            Err("no space")
        );
        assert_eq!(start, 0);
        assert_eq!(dirty, 0);
    }

    #[test]
    fn readonly_flush_preserves_metadata_and_clean_state() {
        // Explicit controlled metadata sector. Clean/read-only files must not
        // set the archive bit or rewrite their stored size/start cluster.
        let mut dirty = 0;
        let stored = [1, 2, 3, 4];
        let mut committed = None;
        assert_eq!(
            flush(
                &mut dirty,
                || Ok::<_, &str>(()),
                || panic!("read-only metadata write"),
                || {
                    committed = Some(stored);
                    Ok(())
                },
            ),
            Ok(())
        );
        assert_eq!(committed, Some([1, 2, 3, 4]));
        assert_eq!(stored, [1, 2, 3, 4]);
        assert_eq!(dirty, 0);
    }

    #[test]
    fn offline_flush_preserves_clean_and_dirty_state_without_writes() {
        for initial in [0, 1] {
            let mut dirty = initial;
            assert_eq!(
                flush(
                    &mut dirty,
                    || Err("device offline"),
                    || panic!("offline metadata write"),
                    || panic!("offline USB flush"),
                ),
                Err("device offline")
            );
            assert_eq!(dirty, initial);
        }
    }

    #[test]
    fn clean_usb_disconnection_does_not_dirty_metadata() {
        let mut dirty = 0;
        assert_eq!(
            flush(
                &mut dirty,
                || Ok(()),
                || panic!("clean metadata write"),
                || Err("USB disconnected"),
            ),
            Err("USB disconnected")
        );
        assert_eq!(dirty, 0);
    }

    #[test]
    fn successful_metadata_flush_survives_reload_and_skips_repeat_writes() {
        let mut dirty = 1;
        let mut sector = [0; 4];
        let mut committed = None;
        assert_eq!(
            flush(
                &mut dirty,
                || Ok::<_, &str>(()),
                || {
                    sector = [7, 8, 9, 10];
                    Ok(())
                },
                || Ok(()),
            ),
            Ok(())
        );
        assert_eq!(dirty, 0);
        assert_eq!(
            flush(
                &mut dirty,
                || Ok::<_, &str>(()),
                || panic!("repeat metadata write"),
                || {
                    committed = Some(sector);
                    Ok(())
                },
            ),
            Ok(())
        );
        let reopened = committed.unwrap();
        assert_eq!(reopened, [7, 8, 9, 10]);
        assert_eq!(dirty, 0);
    }

    #[test]
    fn close_releases_ownership_after_success_and_flush_failures() {
        struct State {
            dirty: u32,
            active: bool,
            released: u32,
        }
        for failure in [None, Some("offline"), Some("metadata"), Some("USB")] {
            let mut state = State {
                dirty: 1,
                active: true,
                released: 0,
            };
            let mut stored = [0; 4];
            let result = close(
                &mut state,
                |state| {
                    flush(
                        &mut state.dirty,
                        || {
                            if failure == Some("offline") {
                                Err("offline")
                            } else {
                                Ok(())
                            }
                        },
                        || {
                            if failure == Some("metadata") {
                                Err("metadata")
                            } else {
                                stored = [7, 8, 9, 10];
                                Ok(())
                            }
                        },
                        || {
                            if failure == Some("USB") {
                                Err("USB")
                            } else {
                                Ok(())
                            }
                        },
                    )
                },
                |state| {
                    state.active = false;
                    state.released += 1;
                },
            );
            assert_eq!(result, failure.map_or(Ok(()), Err));
            assert!(!state.active);
            assert_eq!(state.released, 1);
            assert_eq!(state.dirty, u32::from(failure.is_some()));
            let reopened = stored;
            assert_eq!(
                reopened,
                if matches!(failure, Some("offline" | "metadata")) {
                    [0; 4]
                } else {
                    [7, 8, 9, 10]
                }
            );
        }
    }

    #[test]
    fn readonly_close_flushes_transport_and_releases_after_disconnection() {
        for disconnected in [false, true] {
            let mut dirty = 0;
            let mut released = false;
            let result = close(
                &mut dirty,
                |dirty| {
                    flush(
                        dirty,
                        || Ok(()),
                        || panic!("read-only close metadata write"),
                        || {
                            if disconnected {
                                Err("USB disconnected")
                            } else {
                                Ok(())
                            }
                        },
                    )
                },
                |_| released = true,
            );
            assert_eq!(
                result,
                if disconnected {
                    Err("USB disconnected")
                } else {
                    Ok(())
                }
            );
            assert!(released);
            assert_eq!(dirty, 0);
        }
    }
}
