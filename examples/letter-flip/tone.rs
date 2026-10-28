//! Finite effect samples shared by ASND playback and host checks.
pub const SAMPLE_RATE: u32 = 44_100;
pub const SAMPLE_COUNT: usize = SAMPLE_RATE as usize * 120 / 1000;
pub fn fill(samples: &mut [u8; SAMPLE_COUNT], correct: bool) {
    let frequency: u32 = if correct { 660 } else { 330 };
    let step = ((u64::from(frequency) << 32) / u64::from(SAMPLE_RATE)) as u32;
    let mut phase = 0_u32;
    for (i, sample) in samples.iter_mut().enumerate() {
        phase = phase.wrapping_add(step);
        let ramp = (phase >> 16) as i32;
        let triangle = if ramp < 32768 {
            ramp * 2 - 32768
        } else {
            98303 - ramp * 2
        };
        let envelope = i.min(SAMPLE_COUNT - 1 - i).min(441) as i32;
        *sample = (128 + (triangle >> 10) * envelope / 441) as u8;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_soft_tones_have_fixed_samples_and_silent_endpoints() {
        assert_eq!(SAMPLE_COUNT, 5292);
        for (correct, expected) in [
            (false, [128, 128, 128, 133, 135, 158, 128]),
            (true, [128, 128, 134, 146, 145, 98, 128]),
        ] {
            let mut samples = [0; SAMPLE_COUNT];
            fill(&mut samples, correct);
            let actual = [0, 1, 100, 440, 441, 1000, 5291].map(|i| samples[i]);
            assert_eq!(actual, expected);
            assert!(samples.iter().all(|&v| (96..=159).contains(&v)));
        }
    }
}
