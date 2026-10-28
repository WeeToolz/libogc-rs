//! Record actual callback PCM without changing the probe's expected samples.
extern crate alloc;
use core::sync::atomic::{AtomicU32, Ordering};

// The native DSP output is right, then left (dspmixer.s, dspmixer_loop_end).
// +4096 with left volume 0x80 and right volume 0x40 gives +1024, +2048.
pub(crate) const INITIAL_FRAME: [u8; 4] = [0x04, 0x00, 0x08, 0x00];
pub(crate) const REPLACEMENT_FRAME: [u8; 4] = [0xfc, 0x00, 0xf8, 0x00];

pub(crate) fn matches_pcm(bytes: &[u8], frame: &[u8; 4]) -> bool {
    !bytes.is_empty()
        && bytes.len().is_multiple_of(4)
        && bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|sample| sample == frame)
}

pub(crate) struct OutputEvidence {
    first_nonzero: AtomicU32,
    nonzero_frames: AtomicU32,
    silent_buffers: AtomicU32,
}

impl OutputEvidence {
    pub const fn new() -> Self {
        Self {
            first_nonzero: AtomicU32::new(0),
            nonzero_frames: AtomicU32::new(0),
            silent_buffers: AtomicU32::new(0),
        }
    }

    pub fn reset(&self) {
        self.first_nonzero.store(0, Ordering::Release);
        self.nonzero_frames.store(0, Ordering::Release);
        self.silent_buffers.store(0, Ordering::Release);
    }

    pub fn observe(&self, bytes: &[u8]) {
        let mut nonzero = 0;
        for sample in bytes.as_chunks::<4>().0 {
            let word = u32::from_be_bytes(*sample);
            if word != 0 {
                nonzero += 1;
                // Keep the first actual nonzero frame. A later frame must not
                // replace this evidence, including after a stream replacement.
                // Audio callbacks are serialized by the AI interrupt handler.
                if self.first_nonzero.load(Ordering::Relaxed) == 0 {
                    self.first_nonzero.store(word, Ordering::Relaxed);
                }
            }
        }
        self.nonzero_frames.fetch_add(nonzero, Ordering::Relaxed);
        if nonzero == 0 {
            self.silent_buffers.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn detail(&self) -> alloc::string::String {
        alloc::format!(
            "PCM first_nonzero={:08x} nonzero_frames={} silent_buffers={}",
            self.first_nonzero.load(Ordering::Acquire),
            self.nonzero_frames.load(Ordering::Acquire),
            self.silent_buffers.load(Ordering::Acquire),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_pcm_checks_right_then_left_and_every_sample() {
        assert!(matches_pcm(&[4, 0, 8, 0, 4, 0, 8, 0], &INITIAL_FRAME));
        assert!(matches_pcm(&[0xfc, 0, 0xf8, 0], &REPLACEMENT_FRAME));
        assert!(!matches_pcm(&[8, 0, 4, 0], &INITIAL_FRAME));
        assert!(!matches_pcm(&[4, 0, 8, 0, 4, 0, 7, 0], &INITIAL_FRAME));
        assert!(!matches_pcm(&[0; 8], &INITIAL_FRAME));
        assert!(!matches_pcm(&[4, 0, 8], &INITIAL_FRAME));
        assert!(!matches_pcm(&[], &INITIAL_FRAME));
    }

    #[test]
    fn retains_actual_pcm_and_distinguishes_silence() {
        let evidence = OutputEvidence::new();
        evidence.observe(&[0; 8]);
        evidence.observe(&[0, 0, 0, 0, 4, 0, 8, 0]);
        evidence.observe(&[0xfc, 0, 0xf8, 0]);
        assert_eq!(
            evidence.detail(),
            "PCM first_nonzero=04000800 nonzero_frames=2 silent_buffers=1"
        );
        evidence.reset();
        assert_eq!(
            evidence.detail(),
            "PCM first_nonzero=00000000 nonzero_frames=0 silent_buffers=0"
        );
    }
}
