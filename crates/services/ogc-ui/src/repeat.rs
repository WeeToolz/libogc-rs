/// Produces one immediate event, then repeated events for a held key.
///
/// The caller owns the key mapping and selects timing values that match its
/// interface. A changed key starts a new repeat sequence. A missing key resets
/// the sequence, which also prevents a reconnect-held key from resuming an old
/// repeat without a fresh press edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyRepeat<K> {
    key: Option<K>,
    held_frames: u16,
    initial_delay: u16,
    repeat_interval: u16,
    blocked: bool,
}

impl<K: Copy + Eq> KeyRepeat<K> {
    pub const fn new(initial_delay: u16, repeat_interval: u16) -> Self {
        Self {
            key: None,
            held_frames: 0,
            initial_delay,
            repeat_interval: if repeat_interval == 0 {
                1
            } else {
                repeat_interval
            },
            blocked: true,
        }
    }

    /// Return true when this frame should perform the action for `held`.
    pub fn sample(&mut self, held: Option<K>, pressed: Option<K>) -> bool {
        let Some(key) = held else {
            self.reset();
            self.blocked = false;
            return false;
        };
        if pressed == Some(key) {
            self.key = Some(key);
            self.held_frames = 0;
            self.blocked = false;
            return true;
        }
        if self.blocked || self.key != Some(key) {
            self.key = None;
            self.held_frames = 0;
            self.blocked = true;
            return false;
        }
        self.held_frames = self.held_frames.saturating_add(1);
        self.held_frames >= self.initial_delay
            && (self.held_frames - self.initial_delay).is_multiple_of(self.repeat_interval)
    }

    pub fn reset(&mut self) {
        self.key = None;
        self.held_frames = 0;
        self.blocked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::KeyRepeat;

    #[test]
    fn waits_for_a_press_then_repeats_at_configured_frames_and_resets() {
        let mut repeat = KeyRepeat::new(3, 2);
        assert!(!repeat.sample(Some('a'), None));
        assert!(!repeat.sample(Some('a'), None));
        assert!(!repeat.sample(None, None));
        assert!(repeat.sample(Some('a'), Some('a')));
        assert!(!repeat.sample(Some('a'), None));
        assert!(!repeat.sample(Some('a'), None));
        assert!(repeat.sample(Some('a'), None));
        assert!(!repeat.sample(Some('a'), None));
        assert!(repeat.sample(Some('a'), None));
        assert!(!repeat.sample(None, None));
        assert!(!repeat.sample(Some('a'), None));
        assert!(repeat.sample(Some('a'), Some('a')));
    }

    #[test]
    fn a_changed_key_starts_a_new_sequence() {
        let mut repeat = KeyRepeat::new(4, 2);
        assert!(repeat.sample(Some('a'), Some('a')));
        assert!(repeat.sample(Some('b'), Some('b')));
        assert!(!repeat.sample(Some('b'), None));
    }

    #[test]
    fn reset_blocks_a_reconnected_held_key_until_release_and_new_press() {
        let mut repeat = KeyRepeat::new(3, 2);
        assert!(repeat.sample(Some('a'), Some('a')));
        repeat.reset();
        assert!(!repeat.sample(Some('a'), None));
        assert!(!repeat.sample(None, None));
        assert!(repeat.sample(Some('a'), Some('a')));
    }
}
