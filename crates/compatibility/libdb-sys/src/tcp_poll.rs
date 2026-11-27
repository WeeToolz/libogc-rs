pub(crate) fn poll_frames_before_timers<C, F, E>(
    context: &mut C,
    mut poll_frame: impl FnMut(&mut C) -> Result<Option<F>, E>,
    mut process_frame: impl FnMut(&mut C, F) -> Result<(), E>,
    mut poll_timers: impl FnMut(&mut C) -> Result<(), E>,
) -> Result<(), E> {
    let mut frame_error = None;
    while let Some(frame) = poll_frame(context)? {
        if let Err(error) = process_frame(context, frame)
            && frame_error.is_none()
        {
            frame_error = Some(error);
        }
    }
    poll_timers(context)?;
    frame_error.map_or(Ok(()), Err)
}
