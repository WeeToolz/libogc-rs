//! Layer III IMDCT overlap-add and subband frequency inversion from `layer3.c`.

use crate::fixed::Fixed;

/// Add an 18-sample MDCT output to the saved overlap, then store the next
/// 18-sample overlap. `sample` uses C's `[time][subband]` layout.
pub fn overlap_add(
    output: &[Fixed; 36],
    overlap: &mut [Fixed; 18],
    sample: &mut [[Fixed; 32]; 18],
    subband: usize,
) {
    assert!(subband < 32, "subband index must be less than 32");
    for i in 0..18 {
        sample[i][subband] = output[i].wrapping_add(overlap[i]);
        overlap[i] = output[i + 18];
    }
}

/// Emit a zero IMDCT subband: copy saved overlap into the current samples and
/// clear the saved overlap, as `III_overlap_z()` does.
pub fn overlap_zero(overlap: &mut [Fixed; 18], sample: &mut [[Fixed; 32]; 18], subband: usize) {
    assert!(subband < 32, "subband index must be less than 32");
    for i in 0..18 {
        sample[i][subband] = overlap[i];
        overlap[i] = 0;
    }
}

/// Invert odd time lines in one subband, as `III_freqinver()` does.
/// Call this for odd subband indexes, matching the C decoder call sites.
pub fn frequency_invert(sample: &mut [[Fixed; 32]; 18], subband: usize) {
    assert!(subband < 32, "subband index must be less than 32");
    for row in (1..18).step_by(2) {
        sample[row][subband] = sample[row][subband].wrapping_neg();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_add_combines_current_and_previous_samples_then_saves_tail() {
        let mut output = [0; 36];
        let mut overlap = [0; 18];
        let mut sample = [[0; 32]; 18];
        for i in 0..18 {
            output[i] = i as i32 + 10;
            output[i + 18] = i as i32 + 100;
            overlap[i] = i as i32 + 1;
        }

        overlap_add(&output, &mut overlap, &mut sample, 7);
        for i in 0..18 {
            assert_eq!(sample[i][7], output[i] + i as i32 + 1);
            assert_eq!(overlap[i], output[i + 18]);
        }
        assert_eq!(sample[0][6], 0);
    }

    #[test]
    fn zero_overlap_emits_tail_once_and_clears_it() {
        let mut overlap = [0; 18];
        let mut sample = [[0; 32]; 18];
        for (i, value) in overlap.iter_mut().enumerate() {
            *value = 40 + i as i32;
        }
        overlap_zero(&mut overlap, &mut sample, 3);
        for i in 0..18 {
            assert_eq!(sample[i][3], 40 + i as i32);
            assert_eq!(overlap[i], 0);
        }
    }

    #[test]
    fn frequency_inversion_negates_odd_lines_only_in_selected_subband() {
        let mut sample = [[0; 32]; 18];
        for (i, row) in sample.iter_mut().enumerate() {
            row[5] = i as i32 + 1;
            row[6] = 99;
        }
        frequency_invert(&mut sample, 5);
        for (i, row) in sample.iter().enumerate() {
            assert_eq!(
                row[5],
                if i & 1 == 0 {
                    i as i32 + 1
                } else {
                    -(i as i32 + 1)
                }
            );
            assert_eq!(row[6], 99);
        }
    }

    #[test]
    fn zero_and_impulse_overlap_inputs_match_expected_samples() {
        let mut output = [0; 36];
        let mut overlap = [0; 18];
        let mut sample = [[0; 32]; 18];
        overlap_add(&output, &mut overlap, &mut sample, 1);
        assert!(sample.iter().all(|row| row[1] == 0));
        assert!(overlap.iter().all(|&value| value == 0));

        output[0] = 7;
        output[18] = -9;
        overlap[1] = 4;
        overlap_add(&output, &mut overlap, &mut sample, 1);
        assert_eq!(sample[0][1], 7);
        assert_eq!(sample[1][1], 4);
        assert_eq!(overlap[0], -9);
    }
}
