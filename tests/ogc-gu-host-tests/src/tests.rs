use crate::math::*;

fn close<const R: usize, const C: usize>(actual: [[f32; C]; R], expected: [[f32; C]; R]) {
    for row in 0..R {
        for col in 0..C {
            assert!(
                (actual[row][col] - expected[row][col]).abs() < 0.00001,
                "[{row}][{col}]: {} != {}",
                actual[row][col],
                expected[row][col]
            );
        }
    }
}

const IDENTITY: Mtx = [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]];
const AFFINE: Mtx = [[2., 0., 0., 4.], [0., 4., 0., 8.], [0., 0., 8., 16.]];
const INVERSE: Mtx = [
    [0.5, 0., 0., -2.],
    [0., 0.25, 0., -2.],
    [0., 0., 0.125, -2.],
];

#[test]
fn projections_have_fixed_clip_coordinates() {
    let mut out = [[99.; 4]; 4];
    unsafe { frustum(&mut out, 3., -1., -2., 6., 2., 10.) };
    close(
        out,
        [
            [0.5, 0., 0.5, 0.],
            [0., 1., 0.5, 0.],
            [0., 0., -0.25, -2.5],
            [0., 0., -1., 0.],
        ],
    );
    unsafe { perspective(&mut out, 90., 2., 2., 10.) };
    close(
        out,
        [
            [0.5, 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., -0.25, -2.5],
            [0., 0., -1., 0.],
        ],
    );
    unsafe { ortho(&mut out, 3., -1., -2., 6., 2., 10.) };
    close(
        out,
        [
            [0.25, 0., 0., -0.5],
            [0., 0.5, 0., -0.5],
            [0., 0., -0.125, -1.25],
            [0., 0., 0., 1.],
        ],
    );
}

#[test]
fn light_projections_apply_scale_and_offset() {
    let mut out = [[99.; 4]; 3];
    let params = GuLightAdjust {
        scale_s: 2.,
        scale_t: 3.,
        trans_s: 4.,
        trans_t: 5.,
    };
    unsafe { light_frustum(&mut out, 3., -1., -2., 6., 2., params) };
    close(
        out,
        [[1., 0., -3., 0.], [0., 3., -3.5, 0.], [0., 0., -1., 0.]],
    );
    unsafe { light_ortho(&mut out, 3., -1., -2., 6., params) };
    close(
        out,
        [[0.5, 0., 0., 3.], [0., 1.5, 0., 3.5], [0., 0., 0., 1.]],
    );
    unsafe { light_perspective(&mut out, 90., 2., 2., 3., 4., 5.) };
    close(
        out,
        [[1., 0., -4., 0.], [0., 3., -5., 0.], [0., 0., -1., 0.]],
    );
}

#[test]
fn camera_and_reflection_include_plane_position() {
    let mut out = [[99.; 4]; 3];
    let position = GuVector {
        x: 1.,
        y: 2.,
        z: 3.,
    };
    let up = GuVector {
        x: 0.,
        y: 1.,
        z: 0.,
    };
    let target = GuVector {
        x: 1.,
        y: 2.,
        z: 2.,
    };
    unsafe { look_at(&mut out, &position, &up, &target) };
    close(
        out,
        [[1., 0., 0., -1.], [0., 1., 0., -2.], [0., 0., 1., -3.]],
    );
    unsafe { mtx_reflect(&mut out, &position, &up) };
    close(out, [[1., 0., 0., 0.], [0., -1., 0., 4.], [0., 0., 1., 0.]]);
}

#[test]
fn vector_norm_dot_and_opposite_half_angle() {
    let mut vector = GuVector {
        x: 0.,
        y: 3.,
        z: 4.,
    };
    unsafe { vec_normalize_api(&mut vector) };
    close([[vector.x, vector.y, vector.z]], [[0., 0.6, 0.8]]);
    let other = GuVector {
        x: 2.,
        y: 5.,
        z: -1.,
    };
    assert!((unsafe { vec_dot_product(&vector, &other) } - 2.2).abs() < 0.00001);
    let a = GuVector {
        x: 1.,
        y: 0.,
        z: 0.,
    };
    let b = GuVector {
        x: -1.,
        y: 0.,
        z: 0.,
    };
    unsafe { vec_half_angle(&a, &b, &mut vector) };
    assert_eq!(vector, GuVector::default());
}

#[test]
fn quaternion_zero_and_matrix_conversion() {
    let zero = GuQuaternion::default();
    let mut out = GuQuaternion {
        x: 9.,
        y: 9.,
        z: 9.,
        w: 9.,
    };
    unsafe { quat_normalize(&zero, &mut out) };
    assert_eq!(out, zero);
    unsafe { quat_inverse(&zero, &mut out) };
    assert_eq!(out, zero);
    let rotation: Mtx = [[0., -1., 0., 0.], [1., 0., 0., 0.], [0., 0., 1., 0.]];
    unsafe { quat_mtx(&mut out, &rotation) };
    close(
        [[out.x, out.y, out.z, out.w]],
        [[0., 0., 0.70710677, 0.70710677]],
    );
    let mut matrix = [[9.; 4]; 3];
    unsafe { mtx_quat(&mut matrix, &out) };
    // The C contract writes the rotation block only and keeps translation.
    close(
        matrix,
        [[0., 1., 0., 9.], [-1., 0., 0., 9.], [0., 0., 1., 9.]],
    );
}

#[test]
fn affine_builders_and_distinct_copy_write_all_elements() {
    let mut out = [[99.; 4]; 3];
    unsafe { mtx_identity(&mut out) };
    assert_eq!(out, IDENTITY);
    unsafe { mtx_scale(&mut out, 2., 4., 8.) };
    assert_eq!(out, [[2., 0., 0., 0.], [0., 4., 0., 0.], [0., 0., 8., 0.]]);
    unsafe { mtx_trans(&mut out, 4., 8., 16.) };
    assert_eq!(out, [[1., 0., 0., 4.], [0., 1., 0., 8.], [0., 0., 1., 16.]]);
    unsafe { mtx_copy(&AFFINE, &mut out) };
    assert_eq!(out, AFFINE);
}

#[test]
fn affine_concat_and_inverse_support_both_aliases() {
    let translation = [[1., 0., 0., 1.], [0., 1., 0., 2.], [0., 0., 1., 3.]];
    let expected = [[2., 0., 0., 6.], [0., 4., 0., 16.], [0., 0., 8., 40.]];
    let mut out = [[99.; 4]; 3];
    unsafe { mtx_concat(&AFFINE, &translation, &mut out) };
    assert_eq!(out, expected);
    out = AFFINE;
    let pointer = &mut out as *mut Mtx;
    unsafe { mtx_concat(pointer, &translation, pointer) };
    assert_eq!(out, expected);
    out = translation;
    unsafe { mtx_concat(&AFFINE, pointer, pointer) };
    assert_eq!(out, expected);
    unsafe { assert_eq!(mtx_inverse(&AFFINE, &mut out), 1) };
    assert_eq!(out, INVERSE);
    out = AFFINE;
    unsafe { assert_eq!(mtx_inverse(pointer, pointer), 1) };
    assert_eq!(out, INVERSE);
    let singular = [[0.; 4]; 3];
    unsafe { assert_eq!(mtx_inverse(&singular, &mut out), 0) };
    assert_eq!(out, INVERSE);
}

#[test]
fn affine_transpose_and_inverse_transpose_clear_translation() {
    let source = [[1., 2., 3., 4.], [0., 1., 4., 5.], [0., 0., 1., 6.]];
    let mut out = [[99.; 4]; 3];
    unsafe { mtx_transpose(&source, &mut out) };
    assert_eq!(out, [[1., 0., 0., 0.], [2., 1., 0., 0.], [3., 4., 1., 0.]]);
    unsafe { assert_eq!(mtx_inv_xpose(&source, &mut out), 1) };
    let expected = [[1., 0., 0., 0.], [-2., 1., 0., 0.], [5., -4., 1., 0.]];
    assert_eq!(out, expected);
    out = source;
    let pointer = &mut out as *mut Mtx;
    unsafe { assert_eq!(mtx_inv_xpose(pointer, pointer), 1) };
    assert_eq!(out, expected);
    unsafe { assert_eq!(mtx_inv_xpose(&[[0.; 4]; 3], &mut out), 0) };
    assert_eq!(out, expected);
    out = source;
    unsafe { mtx_transpose(pointer, pointer) };
    assert_eq!(out, [[1., 0., 0., 0.], [2., 1., 0., 0.], [3., 4., 1., 0.]]);
}

#[test]
fn rotations_accept_axis_case_and_normalize_arbitrary_axis() {
    let x = [[1., 0., 0., 0.], [0., 0., -1., 0.], [0., 1., 0., 0.]];
    let y = [[0., 0., 1., 0.], [0., 1., 0., 0.], [-1., 0., 0., 0.]];
    let z = [[0., -1., 0., 0.], [1., 0., 0., 0.], [0., 0., 1., 0.]];
    let mut out = [[99.; 4]; 3];
    for (axis, expected) in [
        (b'x', x),
        (b'X', x),
        (b'y', y),
        (b'Y', y),
        (b'z', z),
        (b'Z', z),
    ] {
        unsafe { mtx_rot_trig(&mut out, axis as core::ffi::c_char, 1., 0.) };
        assert_eq!(out, expected);
    }
    unsafe {
        mtx_rot_rad(
            &mut out,
            b'x' as core::ffi::c_char,
            core::f32::consts::FRAC_PI_2,
        )
    };
    close(out, x);
    unsafe { mtx_rot_deg(&mut out, b'y' as core::ffi::c_char, 90.) };
    close(out, y);
    let mut axis = GuVector {
        x: 0.,
        y: 0.,
        z: 5.,
    };
    unsafe { mtx_rot_axis_rad(&mut out, &mut axis, core::f32::consts::FRAC_PI_2) };
    close(out, z);
    assert_eq!(
        axis,
        GuVector {
            x: 0.,
            y: 0.,
            z: 1.
        }
    );
    axis = GuVector {
        x: 0.,
        y: 3.,
        z: 0.,
    };
    unsafe { mtx_rot_axis_deg(&mut out, &mut axis, 90.) };
    close(out, y);
    assert_eq!(
        axis,
        GuVector {
            x: 0.,
            y: 1.,
            z: 0.
        }
    );
}

#[test]
fn full_matrix_copy_concat_and_inverse() {
    let source: Mtx44 = [
        [2., 0., 0., 4.],
        [0., 4., 0., 8.],
        [0., 0., 8., 16.],
        [0., 0., 0., 1.],
    ];
    let inverse: Mtx44 = [
        [0.5, 0., 0., -2.],
        [0., 0.25, 0., -2.],
        [0., 0., 0.125, -2.],
        [0., 0., 0., 1.],
    ];
    let identity: Mtx44 = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    let mut out = [[99.; 4]; 4];
    unsafe { mtx44_identity(&mut out) };
    assert_eq!(out, identity);
    unsafe { mtx44_copy(&source, &mut out) };
    assert_eq!(out, source);
    unsafe { assert_eq!(mtx44_inverse(&source, &mut out), 1) };
    assert_eq!(out, inverse);
    unsafe { mtx44_concat(&source, &inverse, &mut out) };
    assert_eq!(out, identity);
    let pointer = &mut out as *mut Mtx44;
    out = source;
    unsafe { mtx44_concat(pointer, &inverse, pointer) };
    assert_eq!(out, identity);
    out = inverse;
    unsafe { mtx44_concat(&source, pointer, pointer) };
    assert_eq!(out, identity);
    out = source;
    unsafe { assert_eq!(mtx44_inverse(pointer, pointer), 1) };
    assert_eq!(out, inverse);
    out = [[0.; 4]; 4];
    unsafe { assert_eq!(mtx44_inverse(pointer, pointer), 0) };
    assert_eq!(out, [[0.; 4]; 4]);
    unsafe { assert_eq!(mtx44_inverse(&[[0.; 4]; 4], &mut out), 0) };
}

#[test]
fn full_matrix_inverse_checks_off_diagonal_cofactors() {
    let source = [
        [1., 2., 3., 4.],
        [0., 1., 4., 2.],
        [5., 6., 0., 1.],
        [2., 7., 1., 3.],
    ];
    // Exact rational inverse from independent row reduction; determinant is -198.
    let expected = [
        [1. / 6., -1. / 18., 5. / 18., -5. / 18.],
        [-7. / 33., 10. / 99., -5. / 99., 23. / 99.],
        [-1. / 6., 7. / 18., 1. / 18., -1. / 18.],
        [29. / 66., -65. / 198., -17. / 198., -1. / 198.],
    ];
    let mut out = [[99.; 4]; 4];
    unsafe { assert_eq!(mtx44_inverse(&source, &mut out), 1) };
    close(out, expected);
    out = source;
    let pointer = &mut out as *mut Mtx44;
    unsafe { assert_eq!(mtx44_inverse(pointer, pointer), 1) };
    close(out, expected);
}
