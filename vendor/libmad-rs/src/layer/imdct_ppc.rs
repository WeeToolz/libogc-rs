//! PowerPC long-block IMDCT operation graph for the bundled libmad profile.
// The C reference is libmad/layer3.c, with FPM_PPC and no OPT_ACCURACY.
use super::configured_dot;
use crate::fixed::Fixed;

pub(super) fn transform(input: &[Fixed; 18]) -> [Fixed; 36] {
    let mut x = [0i32; 36];
    let mut sum: Fixed;
    sum = configured_dot(&[(input[4], 0x0ec835e8), (input[13], 0x061f78aa)]);
    let t6 = sum;
    let t14 = input[1].wrapping_sub(input[10]);
    let t15 = input[7].wrapping_add(input[16]);
    sum = sum.wrapping_add(configured_dot(&[(t14, -0x061f78aa), (t15, -0x0ec835e8)]));
    let t0 = sum;
    let t8 = input[0].wrapping_sub(input[11]).wrapping_sub(input[12]);
    let t9 = input[2].wrapping_sub(input[9]).wrapping_sub(input[14]);
    let t10 = input[3].wrapping_sub(input[8]).wrapping_sub(input[15]);
    let t11 = input[5].wrapping_sub(input[6]).wrapping_sub(input[17]);
    sum = sum.wrapping_add(configured_dot(&[
        (t8, 0x0216a2a2),
        (t9, 0x09bd7ca0),
        (t10, -0x0cb19346),
        (t11, -0x0fdcf549),
    ]));
    x[7] = sum;
    x[10] = x[7].wrapping_neg();
    sum = configured_dot(&[
        (t8, -0x0cb19346),
        (t9, 0x0fdcf549),
        (t10, 0x0216a2a2),
        (t11, -0x09bd7ca0),
    ]);
    x[34] = sum.wrapping_sub(t0);
    x[19] = x[34];
    let t12 = input[0]
        .wrapping_sub(input[3])
        .wrapping_add(input[8])
        .wrapping_sub(input[11])
        .wrapping_sub(input[12])
        .wrapping_add(input[15]);
    let t13 = input[2]
        .wrapping_add(input[5])
        .wrapping_sub(input[6])
        .wrapping_sub(input[9])
        .wrapping_sub(input[14])
        .wrapping_sub(input[17]);
    sum = configured_dot(&[(t12, -0x0ec835e8), (t13, 0x061f78aa)]);
    x[31] = sum.wrapping_add(t0);
    x[22] = x[31];
    sum = configured_dot(&[
        (input[1], -0x09bd7ca0),
        (input[7], 0x0216a2a2),
        (input[10], -0x0fdcf549),
        (input[16], 0x0cb19346),
    ]);
    let t1 = sum.wrapping_add(t6);
    sum = configured_dot(&[
        (input[0], 0x03768962),
        (input[2], 0x0e313245),
        (input[3], -0x0ffc19fd),
        (input[5], -0x0acf37ad),
        (input[6], 0x04cfb0e2),
        (input[8], -0x0898c779),
        (input[9], 0x0d7e8807),
        (input[11], 0x0f426cb5),
        (input[12], -0x0bcbe352),
        (input[14], 0x00b2aa3e),
        (input[15], -0x07635284),
        (input[17], -0x0f9ee890),
    ]);
    x[6] = sum.wrapping_add(t1);
    x[11] = x[6].wrapping_neg();
    sum = configured_dot(&[
        (input[0], -0x0f426cb5),
        (input[2], -0x00b2aa3e),
        (input[3], 0x0898c779),
        (input[5], 0x0f9ee890),
        (input[6], 0x0acf37ad),
        (input[8], -0x07635284),
        (input[9], -0x0e313245),
        (input[11], -0x0bcbe352),
        (input[12], -0x03768962),
        (input[14], 0x0d7e8807),
        (input[15], 0x0ffc19fd),
        (input[17], 0x04cfb0e2),
    ]);
    x[30] = sum.wrapping_add(t1);
    x[23] = x[30];
    sum = configured_dot(&[
        (input[0], -0x0bcbe352),
        (input[2], 0x0d7e8807),
        (input[3], -0x07635284),
        (input[5], 0x04cfb0e2),
        (input[6], 0x0f9ee890),
        (input[8], -0x0ffc19fd),
        (input[9], -0x00b2aa3e),
        (input[11], 0x03768962),
        (input[12], -0x0f426cb5),
        (input[14], 0x0e313245),
        (input[15], 0x0898c779),
        (input[17], -0x0acf37ad),
    ]);
    x[35] = sum.wrapping_sub(t1);
    x[18] = x[35];
    sum = configured_dot(&[(input[4], 0x061f78aa), (input[13], -0x0ec835e8)]);
    let t7 = sum;
    sum = sum.wrapping_add(configured_dot(&[
        (input[1], -0x0cb19346),
        (input[7], 0x0fdcf549),
        (input[10], 0x0216a2a2),
        (input[16], -0x09bd7ca0),
    ]));
    let t2 = sum;
    sum = sum.wrapping_add(configured_dot(&[
        (input[0], 0x04cfb0e2),
        (input[2], 0x0ffc19fd),
        (input[3], -0x0d7e8807),
        (input[5], 0x03768962),
        (input[6], -0x0bcbe352),
        (input[8], -0x0e313245),
        (input[9], 0x07635284),
        (input[11], -0x0acf37ad),
        (input[12], 0x0f9ee890),
        (input[14], 0x0898c779),
        (input[15], 0x00b2aa3e),
        (input[17], 0x0f426cb5),
    ]));
    x[5] = sum;
    x[12] = x[5].wrapping_neg();
    sum = configured_dot(&[
        (input[0], 0x0acf37ad),
        (input[2], -0x0898c779),
        (input[3], 0x0e313245),
        (input[5], -0x0f426cb5),
        (input[6], -0x03768962),
        (input[8], 0x00b2aa3e),
        (input[9], -0x0ffc19fd),
        (input[11], 0x0f9ee890),
        (input[12], -0x04cfb0e2),
        (input[14], 0x07635284),
        (input[15], 0x0d7e8807),
        (input[17], -0x0bcbe352),
    ]);
    x[0] = sum.wrapping_add(t2);
    x[17] = x[0].wrapping_neg();
    sum = configured_dot(&[
        (input[0], -0x0f9ee890),
        (input[2], -0x07635284),
        (input[3], -0x00b2aa3e),
        (input[5], 0x0bcbe352),
        (input[6], 0x0f426cb5),
        (input[8], 0x0d7e8807),
        (input[9], 0x0898c779),
        (input[11], -0x04cfb0e2),
        (input[12], -0x0acf37ad),
        (input[14], -0x0ffc19fd),
        (input[15], -0x0e313245),
        (input[17], -0x03768962),
    ]);
    x[29] = sum.wrapping_add(t2);
    x[24] = x[29];
    sum = configured_dot(&[
        (input[1], -0x0216a2a2),
        (input[7], -0x09bd7ca0),
        (input[10], 0x0cb19346),
        (input[16], 0x0fdcf549),
    ]);
    let t3 = sum.wrapping_add(t7);
    sum = configured_dot(&[
        (input[0], 0x00b2aa3e),
        (input[2], 0x03768962),
        (input[3], -0x04cfb0e2),
        (input[5], -0x07635284),
        (input[6], 0x0898c779),
        (input[8], 0x0acf37ad),
        (input[9], -0x0bcbe352),
        (input[11], -0x0d7e8807),
        (input[12], 0x0e313245),
        (input[14], 0x0f426cb5),
        (input[15], -0x0f9ee890),
        (input[17], -0x0ffc19fd),
    ]);
    x[8] = sum.wrapping_add(t3);
    x[9] = x[8].wrapping_neg();
    sum = configured_dot(&[
        (input[0], -0x0e313245),
        (input[2], 0x0bcbe352),
        (input[3], 0x0f9ee890),
        (input[5], -0x0898c779),
        (input[6], -0x0ffc19fd),
        (input[8], 0x04cfb0e2),
        (input[9], 0x0f426cb5),
        (input[11], -0x00b2aa3e),
        (input[12], -0x0d7e8807),
        (input[14], -0x03768962),
        (input[15], 0x0acf37ad),
        (input[17], 0x07635284),
    ]);
    x[32] = sum.wrapping_add(t3);
    x[21] = x[32];
    sum = configured_dot(&[
        (input[0], -0x0d7e8807),
        (input[2], 0x0f426cb5),
        (input[3], 0x0acf37ad),
        (input[5], -0x0ffc19fd),
        (input[6], -0x07635284),
        (input[8], 0x0f9ee890),
        (input[9], 0x03768962),
        (input[11], -0x0e313245),
        (input[12], 0x00b2aa3e),
        (input[14], 0x0bcbe352),
        (input[15], -0x04cfb0e2),
        (input[17], -0x0898c779),
    ]);
    x[33] = sum.wrapping_sub(t3);
    x[20] = x[33];
    sum = configured_dot(&[(t14, -0x0ec835e8), (t15, 0x061f78aa)]);
    let t4 = sum.wrapping_sub(t7);
    sum = configured_dot(&[(t12, 0x061f78aa), (t13, 0x0ec835e8)]);
    x[4] = sum.wrapping_add(t4);
    x[13] = x[4].wrapping_neg();
    sum = configured_dot(&[
        (t8, 0x09bd7ca0),
        (t9, -0x0216a2a2),
        (t10, 0x0fdcf549),
        (t11, -0x0cb19346),
    ]);
    x[1] = sum.wrapping_add(t4);
    x[16] = x[1].wrapping_neg();
    sum = configured_dot(&[
        (t8, -0x0fdcf549),
        (t9, -0x0cb19346),
        (t10, -0x09bd7ca0),
        (t11, -0x0216a2a2),
    ]);
    x[28] = sum.wrapping_add(t4);
    x[25] = x[28];
    sum = configured_dot(&[
        (input[1], -0x0fdcf549),
        (input[7], -0x0cb19346),
        (input[10], -0x09bd7ca0),
        (input[16], -0x0216a2a2),
    ]);
    let t5 = sum.wrapping_sub(t6);
    sum = configured_dot(&[
        (input[0], 0x0898c779),
        (input[2], 0x04cfb0e2),
        (input[3], 0x0bcbe352),
        (input[5], 0x00b2aa3e),
        (input[6], 0x0e313245),
        (input[8], -0x03768962),
        (input[9], 0x0f9ee890),
        (input[11], -0x07635284),
        (input[12], 0x0ffc19fd),
        (input[14], -0x0acf37ad),
        (input[15], 0x0f426cb5),
        (input[17], -0x0d7e8807),
    ]);
    x[2] = sum.wrapping_add(t5);
    x[15] = x[2].wrapping_neg();
    sum = configured_dot(&[
        (input[0], 0x07635284),
        (input[2], 0x0acf37ad),
        (input[3], 0x03768962),
        (input[5], 0x0d7e8807),
        (input[6], -0x00b2aa3e),
        (input[8], 0x0f426cb5),
        (input[9], -0x04cfb0e2),
        (input[11], 0x0ffc19fd),
        (input[12], -0x0898c779),
        (input[14], 0x0f9ee890),
        (input[15], -0x0bcbe352),
        (input[17], 0x0e313245),
    ]);
    x[3] = sum.wrapping_add(t5);
    x[14] = x[3].wrapping_neg();
    sum = configured_dot(&[
        (input[0], -0x0ffc19fd),
        (input[2], -0x0f9ee890),
        (input[3], -0x0f426cb5),
        (input[5], -0x0e313245),
        (input[6], -0x0d7e8807),
        (input[8], -0x0bcbe352),
        (input[9], -0x0acf37ad),
        (input[11], -0x0898c779),
        (input[12], -0x07635284),
        (input[14], -0x04cfb0e2),
        (input[15], -0x03768962),
        (input[17], -0x00b2aa3e),
    ]);
    x[27] = sum.wrapping_add(t5);
    x[26] = x[27];
    x
}
