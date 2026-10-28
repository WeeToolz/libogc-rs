/// Validate ring pointers and calculate the occupied byte count.
pub(crate) fn fifo_pointers(
    base: u32,
    size: u32,
    read: u32,
    write: u32,
) -> Option<(u32, u32, u32)> {
    let end = base.checked_add(size)?;
    if size == 0 || !(base..end).contains(&read) || !(base..end).contains(&write) {
        return None;
    }
    let distance = if write >= read {
        write - read
    } else {
        size - (read - write)
    };
    Some((read, write, distance))
}
