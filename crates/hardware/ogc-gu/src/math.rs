use core::ffi::c_char;
use core::ptr;
use libm::{cosf, sinf, sqrtf, tanf};

pub use ogc_gu_sys::{
    GuQuaternion, GuVector, M_DTOR, M_PI, Mtx, Mtx33, Mtx44, deg_to_rad, fix32tof, ftofix32,
    fttofrac8, max, min, rad_to_deg,
};

#[inline]
fn vec_normalize(v: &mut GuVector) {
    let mag = (v.x * v.x) + (v.y * v.y) + (v.z * v.z);
    let scale = 1.0 / sqrtf(mag);
    v.x *= scale;
    v.y *= scale;
    v.z *= scale;
}

#[inline]
fn vec_dot(a: &GuVector, b: &GuVector) -> f32 {
    (a.x * b.x) + (a.y * b.y) + (a.z * b.z)
}

#[inline]
fn vec_cross_inner(a: GuVector, b: GuVector) -> GuVector {
    GuVector {
        x: (a.y * b.z) - (a.z * b.y),
        y: (a.z * b.x) - (a.x * b.z),
        z: (a.x * b.y) - (a.y * b.x),
    }
}

#[inline]
fn axis_index(axis: c_char) -> Option<u8> {
    match axis.to_ne_bytes()[0] {
        b'x' | b'X' => Some(0),
        b'y' | b'Y' => Some(1),
        b'z' | b'Z' => Some(2),
        _ => None,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct GuLightAdjust {
    pub scale_s: f32,
    pub scale_t: f32,
    pub trans_s: f32,
    pub trans_t: f32,
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn frustum(mt: *mut Mtx44, t: f32, b: f32, l: f32, r: f32, n: f32, f: f32) {
    unsafe {
        let mt = &mut *mt;

        let mut tmp = 1.0f32 / (r - l);
        mt[0][0] = (2.0 * n) * tmp;
        mt[0][1] = 0.0;
        mt[0][2] = (r + l) * tmp;
        mt[0][3] = 0.0;

        tmp = 1.0 / (t - b);
        mt[1][0] = 0.0;
        mt[1][1] = (2.0 * n) * tmp;
        mt[1][2] = (t + b) * tmp;
        mt[1][3] = 0.0;

        tmp = 1.0 / (f - n);
        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = -n * tmp;
        mt[2][3] = -(f * n) * tmp;

        mt[3][0] = 0.0;
        mt[3][1] = 0.0;
        mt[3][2] = -1.0;
        mt[3][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn perspective(mt: *mut Mtx44, fovy: f32, aspect: f32, n: f32, f: f32) {
    unsafe {
        let mt = &mut *mt;
        let angle = deg_to_rad(fovy * 0.5);
        let cot = 1.0 / tanf(angle);
        let tmp = 1.0 / (f - n);

        mt[0][0] = cot / aspect;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = 0.0;

        mt[1][0] = 0.0;
        mt[1][1] = cot;
        mt[1][2] = 0.0;
        mt[1][3] = 0.0;

        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = -n * tmp;
        mt[2][3] = -(f * n) * tmp;

        mt[3][0] = 0.0;
        mt[3][1] = 0.0;
        mt[3][2] = -1.0;
        mt[3][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn ortho(mt: *mut Mtx44, t: f32, b: f32, l: f32, r: f32, n: f32, f: f32) {
    unsafe {
        let mt = &mut *mt;

        let mut tmp = 1.0f32 / (r - l);
        mt[0][0] = 2.0 * tmp;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = -(r + l) * tmp;

        tmp = 1.0 / (t - b);
        mt[1][0] = 0.0;
        mt[1][1] = 2.0 * tmp;
        mt[1][2] = 0.0;
        mt[1][3] = -(t + b) * tmp;

        tmp = 1.0 / (f - n);
        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = -tmp;
        mt[2][3] = -f * tmp;

        mt[3][0] = 0.0;
        mt[3][1] = 0.0;
        mt[3][2] = 0.0;
        mt[3][3] = 1.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn light_perspective(
    mt: *mut Mtx,
    fov_y: f32,
    aspect: f32,
    scale_s: f32,
    scale_t: f32,
    trans_s: f32,
    trans_t: f32,
) {
    unsafe {
        let mt = &mut *mt;
        let cot = 1.0 / tanf(deg_to_rad(fov_y * 0.5));

        mt[0][0] = (cot / aspect) * scale_s;
        mt[0][1] = 0.0;
        mt[0][2] = -trans_s;
        mt[0][3] = 0.0;

        mt[1][0] = 0.0;
        mt[1][1] = cot * scale_t;
        mt[1][2] = -trans_t;
        mt[1][3] = 0.0;

        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = -1.0;
        mt[2][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn light_ortho(mt: *mut Mtx, t: f32, b: f32, l: f32, r: f32, params: GuLightAdjust) {
    unsafe {
        let mt = &mut *mt;

        let mut tmp = 1.0 / (r - l);
        mt[0][0] = (2.0 * tmp) * params.scale_s;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = ((-(r + l) * tmp) * params.scale_s) + params.trans_s;

        tmp = 1.0 / (t - b);
        mt[1][0] = 0.0;
        mt[1][1] = (2.0 * tmp) * params.scale_t;
        mt[1][2] = 0.0;
        mt[1][3] = ((-(t + b) * tmp) * params.scale_t) + params.trans_t;

        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = 0.0;
        mt[2][3] = 1.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn light_frustum(
    mt: *mut Mtx,
    t: f32,
    b: f32,
    l: f32,
    r: f32,
    n: f32,
    params: GuLightAdjust,
) {
    unsafe {
        let mt = &mut *mt;

        let mut tmp = 1.0 / (r - l);
        mt[0][0] = ((2.0 * n) * tmp) * params.scale_s;
        mt[0][1] = 0.0;
        mt[0][2] = (((r + l) * tmp) * params.scale_s) - params.trans_s;
        mt[0][3] = 0.0;

        tmp = 1.0 / (t - b);
        mt[1][0] = 0.0;
        mt[1][1] = ((2.0 * n) * tmp) * params.scale_t;
        mt[1][2] = (((t + b) * tmp) * params.scale_t) - params.trans_t;
        mt[1][3] = 0.0;

        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = -1.0;
        mt[2][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn look_at(
    mt: *mut Mtx,
    cam_pos: *const GuVector,
    cam_up: *const GuVector,
    target: *const GuVector,
) {
    unsafe {
        let mt = &mut *mt;
        let cam_pos = &*cam_pos;
        let cam_up = &*cam_up;
        let target = &*target;

        let mut v_look = GuVector {
            x: cam_pos.x - target.x,
            y: cam_pos.y - target.y,
            z: cam_pos.z - target.z,
        };
        vec_normalize(&mut v_look);

        let mut v_right = GuVector::default();
        vec_cross(cam_up, &v_look, &mut v_right);
        vec_normalize(&mut v_right);

        let mut v_up = GuVector::default();
        vec_cross(&v_look, &v_right, &mut v_up);

        mt[0][0] = v_right.x;
        mt[0][1] = v_right.y;
        mt[0][2] = v_right.z;
        mt[0][3] = -(cam_pos.x * v_right.x + cam_pos.y * v_right.y + cam_pos.z * v_right.z);

        mt[1][0] = v_up.x;
        mt[1][1] = v_up.y;
        mt[1][2] = v_up.z;
        mt[1][3] = -(cam_pos.x * v_up.x + cam_pos.y * v_up.y + cam_pos.z * v_up.z);

        mt[2][0] = v_look.x;
        mt[2][1] = v_look.y;
        mt[2][2] = v_look.z;
        mt[2][3] = -(cam_pos.x * v_look.x + cam_pos.y * v_look.y + cam_pos.z * v_look.z);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input vector.
pub unsafe fn vec_half_angle(a: *const GuVector, b: *const GuVector, half: *mut GuVector) {
    unsafe {
        let a = *a;
        let b = *b;
        let half = &mut *half;

        let mut tmp1 = GuVector {
            x: -a.x,
            y: -a.y,
            z: -a.z,
        };
        let mut tmp2 = GuVector {
            x: -b.x,
            y: -b.y,
            z: -b.z,
        };
        vec_normalize(&mut tmp1);
        vec_normalize(&mut tmp2);

        let mut tmp3 = GuVector::default();
        vec_add(&tmp1, &tmp2, &mut tmp3);
        if vec_dot(&tmp3, &tmp3) > 0.0 {
            vec_normalize(&mut tmp3);
        }

        *half = tmp3;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input vector.
pub unsafe fn vec_add(a: *const GuVector, b: *const GuVector, ab: *mut GuVector) {
    unsafe {
        let a = *a;
        let b = *b;
        *ab = GuVector {
            x: a.x + b.x,
            y: a.y + b.y,
            z: a.z + b.z,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input vector.
pub unsafe fn vec_sub(a: *const GuVector, b: *const GuVector, ab: *mut GuVector) {
    unsafe {
        let a = *a;
        let b = *b;
        *ab = GuVector {
            x: a.x - b.x,
            y: a.y - b.y,
            z: a.z - b.z,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The destination may point to the source vector.
pub unsafe fn vec_scale(src: *const GuVector, dst: *mut GuVector, scale: f32) {
    unsafe {
        let src = *src;
        *dst = GuVector {
            x: src.x * scale,
            y: src.y * scale,
            z: src.z * scale,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn vec_normalize_api(v: *mut GuVector) {
    unsafe {
        vec_normalize(&mut *v);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination vectors may be the same. The matrix must not
/// overlap the destination.
pub unsafe fn vec_multiply(mt: *const Mtx, src: *const GuVector, dst: *mut GuVector) {
    unsafe {
        let mt = &*mt;
        let src = *src;
        let dst = &mut *dst;
        dst.x = mt[0][0] * src.x + mt[0][1] * src.y + mt[0][2] * src.z + mt[0][3];
        dst.y = mt[1][0] * src.x + mt[1][1] * src.y + mt[1][2] * src.z + mt[1][3];
        dst.z = mt[2][0] * src.x + mt[2][1] * src.y + mt[2][2] * src.z + mt[2][3];
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input vector.
pub unsafe fn vec_cross(a: *const GuVector, b: *const GuVector, axb: *mut GuVector) {
    unsafe {
        let a = *a;
        let b = *b;
        *axb = vec_cross_inner(a, b);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination vectors may be the same. The matrix must not
/// overlap the destination.
pub unsafe fn vec_multiply_sr(mt: *const Mtx, src: *const GuVector, dst: *mut GuVector) {
    unsafe {
        let mt = &*mt;
        let src = *src;
        let dst = &mut *dst;
        dst.x = mt[0][0] * src.x + mt[0][1] * src.y + mt[0][2] * src.z;
        dst.y = mt[1][0] * src.x + mt[1][1] * src.y + mt[1][2] * src.z;
        dst.z = mt[2][0] * src.x + mt[2][1] * src.y + mt[2][2] * src.z;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn vec_dot_product(a: *const GuVector, b: *const GuVector) -> f32 {
    unsafe { vec_dot(&*a, &*b) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input quaternion.
pub unsafe fn quat_add(a: *const GuQuaternion, b: *const GuQuaternion, ab: *mut GuQuaternion) {
    unsafe {
        let a = *a;
        let b = *b;
        *ab = GuQuaternion {
            x: a.x + b.x,
            y: a.y + b.y,
            z: a.z + b.z,
            w: a.w + b.w,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input quaternion.
pub unsafe fn quat_sub(a: *const GuQuaternion, b: *const GuQuaternion, ab: *mut GuQuaternion) {
    unsafe {
        let a = *a;
        let b = *b;
        *ab = GuQuaternion {
            x: a.x - b.x,
            y: a.y - b.y,
            z: a.z - b.z,
            w: a.w - b.w,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The output may point to either input quaternion.
pub unsafe fn quat_multiply(a: *const GuQuaternion, b: *const GuQuaternion, ab: *mut GuQuaternion) {
    unsafe {
        let a = *a;
        let b = *b;
        *ab = GuQuaternion {
            w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
            x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            y: a.w * b.y + a.y * b.w + a.z * b.x - a.x * b.z,
            z: a.w * b.z + a.z * b.w + a.x * b.y - a.y * b.x,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The destination may point to the source quaternion.
pub unsafe fn quat_normalize(a: *const GuQuaternion, d: *mut GuQuaternion) {
    unsafe {
        let a = *a;
        let dot = (a.x * a.x) + (a.y * a.y) + (a.z * a.z) + (a.w * a.w);
        if dot == 0.0 {
            *d = GuQuaternion::default();
            return;
        }

        let scale = 1.0 / sqrtf(dot);
        *d = GuQuaternion {
            x: a.x * scale,
            y: a.y * scale,
            z: a.z * scale,
            w: a.w * scale,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The destination may point to the source quaternion.
pub unsafe fn quat_inverse(a: *const GuQuaternion, d: *mut GuQuaternion) {
    unsafe {
        let a = *a;
        let mut mag = (a.x * a.x) + (a.y * a.y) + (a.z * a.z) + (a.w * a.w);
        if mag == 0.0 {
            mag = 1.0;
        }
        let nrminv = 1.0 / mag;
        *d = GuQuaternion {
            x: -a.x * nrminv,
            y: -a.y * nrminv,
            z: -a.z * nrminv,
            w: a.w * nrminv,
        };
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn quat_mtx(a: *mut GuQuaternion, m: *const Mtx) {
    unsafe {
        let a = &mut *a;
        let m = &*m;
        let diag = m[0][0] + m[1][1] + m[2][2] + 1.0;

        if diag > 0.0 {
            let scale = sqrtf(diag) * 2.0;
            a.x = (m[2][1] - m[1][2]) / scale;
            a.y = (m[0][2] - m[2][0]) / scale;
            a.z = (m[1][0] - m[0][1]) / scale;
            a.w = 0.25 * scale;
        } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
            let scale = sqrtf(1.0 + m[0][0] + m[1][1] + m[2][2]) * 2.0;
            a.x = 0.25 * scale;
            a.y = (m[0][1] + m[1][0]) / scale;
            a.z = (m[2][0] + m[0][2]) / scale;
            a.w = (m[2][1] - m[1][2]) / scale;
        } else if m[1][1] > m[2][2] {
            let scale = sqrtf(1.0 + m[0][0] + m[1][1] + m[2][2]) * 2.0;
            a.x = (m[0][1] + m[1][0]) / scale;
            a.y = 0.25 * scale;
            a.z = (m[1][2] + m[2][1]) / scale;
            a.w = (m[0][2] - m[2][0]) / scale;
        } else {
            let scale = sqrtf(1.0 + m[0][0] + m[1][1] + m[2][2]) * 2.0;
            a.x = (m[0][2] + m[2][0]) / scale;
            a.y = (m[1][2] + m[2][1]) / scale;
            a.z = 0.25 * scale;
            a.w = (m[1][0] - m[0][1]) / scale;
        }

        quat_normalize(a as *const GuQuaternion, a as *mut GuQuaternion);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_identity(mt: *mut Mtx) {
    unsafe {
        let mt = &mut *mt;
        mt[0][0] = 1.0;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = 0.0;
        mt[1][0] = 0.0;
        mt[1][1] = 1.0;
        mt[1][2] = 0.0;
        mt[1][3] = 0.0;
        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = 1.0;
        mt[2][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination may be the same matrix.
pub unsafe fn mtx_copy(src: *const Mtx, dst: *mut Mtx) {
    unsafe {
        if !ptr::eq(src, dst.cast_const()) {
            ptr::copy_nonoverlapping(src, dst, 1);
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_concat(a: *const Mtx, b: *const Mtx, ab: *mut Mtx) {
    unsafe {
        let a = &*a;
        let b = &*b;
        let out = ab;
        let mut tmp = [[0.0f32; 4]; 3];
        let tmp_ptr = &mut tmp as *mut Mtx;
        let m = if ptr::eq(out as *const Mtx, a as *const Mtx)
            || ptr::eq(out as *const Mtx, b as *const Mtx)
        {
            tmp_ptr
        } else {
            out
        };
        let m = &mut *m;

        for row in 0..3 {
            for col in 0..3 {
                m[row][col] = a[row][0] * b[0][col] + a[row][1] * b[1][col] + a[row][2] * b[2][col];
            }
            m[row][3] = a[row][0] * b[0][3] + a[row][1] * b[1][3] + a[row][2] * b[2][3] + a[row][3];
        }

        if ptr::eq(m as *const Mtx, tmp_ptr as *const Mtx) {
            *out = tmp;
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_scale(mt: *mut Mtx, x_s: f32, y_s: f32, z_s: f32) {
    unsafe {
        let mt = &mut *mt;
        mt[0][0] = x_s;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = 0.0;
        mt[1][0] = 0.0;
        mt[1][1] = y_s;
        mt[1][2] = 0.0;
        mt[1][3] = 0.0;
        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = z_s;
        mt[2][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination may be the same matrix.
pub unsafe fn mtx_scale_apply(src: *const Mtx, dst: *mut Mtx, x_s: f32, y_s: f32, z_s: f32) {
    unsafe {
        let src = *src;
        let dst = &mut *dst;
        for col in 0..4 {
            dst[0][col] = src[0][col] * x_s;
            dst[1][col] = src[1][col] * y_s;
            dst[2][col] = src[2][col] * z_s;
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination may be the same matrix.
pub unsafe fn mtx_apply_scale(src: *const Mtx, dst: *mut Mtx, x_s: f32, y_s: f32, z_s: f32) {
    unsafe {
        let src = *src;
        let dst = &mut *dst;
        dst[0][0] = src[0][0] * x_s;
        dst[0][1] = src[0][1] * y_s;
        dst[0][2] = src[0][2] * z_s;
        dst[0][3] = src[0][3];
        dst[1][0] = src[1][0] * x_s;
        dst[1][1] = src[1][1] * y_s;
        dst[1][2] = src[1][2] * z_s;
        dst[1][3] = src[1][3];
        dst[2][0] = src[2][0] * x_s;
        dst[2][1] = src[2][1] * y_s;
        dst[2][2] = src[2][2] * z_s;
        dst[2][3] = src[2][3];
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_trans(mt: *mut Mtx, x_t: f32, y_t: f32, z_t: f32) {
    unsafe {
        let mt = &mut *mt;
        mt[0][0] = 1.0;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = x_t;
        mt[1][0] = 0.0;
        mt[1][1] = 1.0;
        mt[1][2] = 0.0;
        mt[1][3] = y_t;
        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = 1.0;
        mt[2][3] = z_t;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination may be the same matrix.
pub unsafe fn mtx_trans_apply(src: *const Mtx, dst: *mut Mtx, x_t: f32, y_t: f32, z_t: f32) {
    unsafe {
        let same = ptr::eq(src, dst.cast_const());
        let src = *src;
        let dst = &mut *dst;
        if !same {
            dst[0][0] = src[0][0];
            dst[0][1] = src[0][1];
            dst[0][2] = src[0][2];
            dst[1][0] = src[1][0];
            dst[1][1] = src[1][1];
            dst[1][2] = src[1][2];
            dst[2][0] = src[2][0];
            dst[2][1] = src[2][1];
            dst[2][2] = src[2][2];
        }

        dst[0][3] = src[0][3] + x_t;
        dst[1][3] = src[1][3] + y_t;
        dst[2][3] = src[2][3] + z_t;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination may be the same matrix.
pub unsafe fn mtx_apply_trans(src: *const Mtx, dst: *mut Mtx, x_t: f32, y_t: f32, z_t: f32) {
    unsafe {
        let same = ptr::eq(src, dst.cast_const());
        let src = *src;
        let dst = &mut *dst;
        if !same {
            dst[0][0] = src[0][0];
            dst[0][1] = src[0][1];
            dst[0][2] = src[0][2];
            dst[1][0] = src[1][0];
            dst[1][1] = src[1][1];
            dst[1][2] = src[1][2];
            dst[2][0] = src[2][0];
            dst[2][1] = src[2][1];
            dst[2][2] = src[2][2];
        }

        dst[0][3] = src[0][0] * x_t + src[0][1] * y_t + src[0][2] * z_t + src[0][3];
        dst[1][3] = src[1][0] * x_t + src[1][1] * y_t + src[1][2] * z_t + src[1][3];
        dst[2][3] = src[2][0] * x_t + src[2][1] * y_t + src[2][2] * z_t + src[2][3];
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_inverse(src: *const Mtx, inv: *mut Mtx) -> u32 {
    unsafe {
        let src = &*src;
        let out = inv;
        let mut tmp = [[0.0f32; 4]; 3];
        let tmp_ptr = &mut tmp as *mut Mtx;
        let m = if ptr::eq(src, out as *const Mtx) {
            tmp_ptr
        } else {
            out
        };
        let m = &mut *m;

        let det = src[0][0] * src[1][1] * src[2][2]
            + src[0][1] * src[1][2] * src[2][0]
            + src[0][2] * src[1][0] * src[2][1]
            - src[2][0] * src[1][1] * src[0][2]
            - src[1][0] * src[0][1] * src[2][2]
            - src[0][0] * src[2][1] * src[1][2];

        if det == 0.0 {
            return 0;
        }

        let det = 1.0 / det;

        m[0][0] = (src[1][1] * src[2][2] - src[2][1] * src[1][2]) * det;
        m[0][1] = -(src[0][1] * src[2][2] - src[2][1] * src[0][2]) * det;
        m[0][2] = (src[0][1] * src[1][2] - src[1][1] * src[0][2]) * det;

        m[1][0] = -(src[1][0] * src[2][2] - src[2][0] * src[1][2]) * det;
        m[1][1] = (src[0][0] * src[2][2] - src[2][0] * src[0][2]) * det;
        m[1][2] = -(src[0][0] * src[1][2] - src[1][0] * src[0][2]) * det;

        m[2][0] = (src[1][0] * src[2][1] - src[2][0] * src[1][1]) * det;
        m[2][1] = -(src[0][0] * src[2][1] - src[2][0] * src[0][1]) * det;
        m[2][2] = (src[0][0] * src[1][1] - src[1][0] * src[0][1]) * det;

        m[0][3] = -m[0][0] * src[0][3] - m[0][1] * src[1][3] - m[0][2] * src[2][3];
        m[1][3] = -m[1][0] * src[0][3] - m[1][1] * src[1][3] - m[1][2] * src[2][3];
        m[2][3] = -m[2][0] * src[0][3] - m[2][1] * src[1][3] - m[2][2] * src[2][3];

        if ptr::eq(m as *const Mtx, tmp_ptr as *const Mtx) {
            *out = tmp;
        }

        1
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_transpose(src: *const Mtx, x_pose: *mut Mtx) {
    unsafe {
        let src = &*src;
        let out = x_pose;
        let mut tmp = [[0.0f32; 4]; 3];
        let tmp_ptr = &mut tmp as *mut Mtx;
        let m = if ptr::eq(src, out as *const Mtx) {
            tmp_ptr
        } else {
            out
        };
        let m = &mut *m;

        m[0][0] = src[0][0];
        m[0][1] = src[1][0];
        m[0][2] = src[2][0];
        m[0][3] = 0.0;
        m[1][0] = src[0][1];
        m[1][1] = src[1][1];
        m[1][2] = src[2][1];
        m[1][3] = 0.0;
        m[2][0] = src[0][2];
        m[2][1] = src[1][2];
        m[2][2] = src[2][2];
        m[2][3] = 0.0;

        if ptr::eq(m as *const Mtx, tmp_ptr as *const Mtx) {
            *out = tmp;
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_inv_xpose(src: *const Mtx, x_pose: *mut Mtx) -> u32 {
    unsafe {
        let src = &*src;
        let out = x_pose;
        let mut tmp = [[0.0f32; 4]; 3];
        let tmp_ptr = &mut tmp as *mut Mtx;
        let m = if ptr::eq(src, out as *const Mtx) {
            tmp_ptr
        } else {
            out
        };
        let m = &mut *m;

        let det = src[0][0] * src[1][1] * src[2][2]
            + src[0][1] * src[1][2] * src[2][0]
            + src[0][2] * src[1][0] * src[2][1]
            - src[2][0] * src[1][1] * src[0][2]
            - src[1][0] * src[0][1] * src[2][2]
            - src[0][0] * src[2][1] * src[1][2];

        if det == 0.0 {
            return 0;
        }

        let det = 1.0 / det;
        m[0][0] = (src[1][1] * src[2][2] - src[2][1] * src[1][2]) * det;
        m[0][1] = -(src[1][0] * src[2][2] - src[2][0] * src[1][2]) * det;
        m[0][2] = (src[1][0] * src[2][1] - src[2][0] * src[1][1]) * det;

        m[1][0] = -(src[0][1] * src[2][2] - src[2][1] * src[0][2]) * det;
        m[1][1] = (src[0][0] * src[2][2] - src[2][0] * src[0][2]) * det;
        m[1][2] = -(src[0][0] * src[2][1] - src[2][0] * src[0][1]) * det;

        m[2][0] = (src[0][1] * src[1][2] - src[1][1] * src[0][2]) * det;
        m[2][1] = -(src[0][0] * src[1][2] - src[1][0] * src[0][2]) * det;
        m[2][2] = (src[0][0] * src[1][1] - src[1][0] * src[0][1]) * det;

        m[0][3] = 0.0;
        m[1][3] = 0.0;
        m[2][3] = 0.0;

        if ptr::eq(m as *const Mtx, tmp_ptr as *const Mtx) {
            *out = tmp;
        }

        1
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_rot_trig(mt: *mut Mtx, axis: c_char, sin_a: f32, cos_a: f32) {
    unsafe {
        let mt = &mut *mt;
        match axis_index(axis) {
            Some(0) => {
                mt[0][0] = 1.0;
                mt[0][1] = 0.0;
                mt[0][2] = 0.0;
                mt[0][3] = 0.0;
                mt[1][0] = 0.0;
                mt[1][1] = cos_a;
                mt[1][2] = -sin_a;
                mt[1][3] = 0.0;
                mt[2][0] = 0.0;
                mt[2][1] = sin_a;
                mt[2][2] = cos_a;
                mt[2][3] = 0.0;
            }
            Some(1) => {
                mt[0][0] = cos_a;
                mt[0][1] = 0.0;
                mt[0][2] = sin_a;
                mt[0][3] = 0.0;
                mt[1][0] = 0.0;
                mt[1][1] = 1.0;
                mt[1][2] = 0.0;
                mt[1][3] = 0.0;
                mt[2][0] = -sin_a;
                mt[2][1] = 0.0;
                mt[2][2] = cos_a;
                mt[2][3] = 0.0;
            }
            Some(2) => {
                mt[0][0] = cos_a;
                mt[0][1] = -sin_a;
                mt[0][2] = 0.0;
                mt[0][3] = 0.0;
                mt[1][0] = sin_a;
                mt[1][1] = cos_a;
                mt[1][2] = 0.0;
                mt[1][3] = 0.0;
                mt[2][0] = 0.0;
                mt[2][1] = 0.0;
                mt[2][2] = 1.0;
                mt[2][3] = 0.0;
            }
            Some(_) | None => {}
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_rot_rad(mt: *mut Mtx, axis: c_char, rad: f32) {
    unsafe {
        let sin_a = sinf(rad);
        let cos_a = cosf(rad);
        mtx_rot_trig(mt, axis, sin_a, cos_a);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_rot_deg(mt: *mut Mtx, axis: c_char, deg: f32) {
    unsafe {
        mtx_rot_rad(mt, axis, deg_to_rad(deg));
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_rot_axis_deg(mt: *mut Mtx, axis: *mut GuVector, deg: f32) {
    unsafe {
        mtx_rot_axis_rad(mt, axis, deg_to_rad(deg));
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_rot_axis_rad(mt: *mut Mtx, axis: *mut GuVector, rad: f32) {
    unsafe {
        let mt = &mut *mt;
        let axis = &mut *axis;
        let s = sinf(rad);
        let c = cosf(rad);
        let t = 1.0 - c;

        vec_normalize(axis);

        let x = axis.x;
        let y = axis.y;
        let z = axis.z;
        let x_sq = x * x;
        let y_sq = y * y;
        let z_sq = z * z;

        mt[0][0] = (t * x_sq) + c;
        mt[0][1] = (t * x * y) - (s * z);
        mt[0][2] = (t * x * z) + (s * y);
        mt[0][3] = 0.0;

        mt[1][0] = (t * x * y) + (s * z);
        mt[1][1] = (t * y_sq) + c;
        mt[1][2] = (t * y * z) - (s * x);
        mt[1][3] = 0.0;

        mt[2][0] = (t * x * z) - (s * y);
        mt[2][1] = (t * y * z) + (s * x);
        mt[2][2] = (t * z_sq) + c;
        mt[2][3] = 0.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_reflect(m: *mut Mtx, p: *const GuVector, n: *const GuVector) {
    unsafe {
        let m = &mut *m;
        let p = &*p;
        let n = &*n;
        let vxy = -2.0 * n.x * n.y;
        let vxz = -2.0 * n.x * n.z;
        let vyz = -2.0 * n.y * n.z;
        let pdotn = 2.0 * vec_dot(p, n);

        m[0][0] = 1.0 - 2.0 * n.x * n.x;
        m[0][1] = vxy;
        m[0][2] = vxz;
        m[0][3] = pdotn * n.x;

        m[1][0] = vxy;
        m[1][1] = 1.0 - 2.0 * n.y * n.y;
        m[1][2] = vyz;
        m[1][3] = pdotn * n.y;

        m[2][0] = vxz;
        m[2][1] = vyz;
        m[2][2] = 1.0 - 2.0 * n.z * n.z;
        m[2][3] = pdotn * n.z;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx_quat(m: *mut Mtx, a: *const GuQuaternion) {
    unsafe {
        let m = &mut *m;
        let a = &*a;
        m[0][0] = 1.0 - 2.0 * a.y * a.y - 2.0 * a.z * a.z;
        m[1][0] = 2.0 * a.x * a.y - 2.0 * a.z * a.w;
        m[2][0] = 2.0 * a.x * a.z + 2.0 * a.y * a.w;
        m[0][1] = 2.0 * a.x * a.y + 2.0 * a.z * a.w;
        m[1][1] = 1.0 - 2.0 * a.x * a.x - 2.0 * a.z * a.z;
        m[2][1] = 2.0 * a.z * a.y - 2.0 * a.x * a.w;
        m[0][2] = 2.0 * a.x * a.z - 2.0 * a.y * a.w;
        m[1][2] = 2.0 * a.z * a.y + 2.0 * a.x * a.w;
        m[2][2] = 1.0 - 2.0 * a.x * a.x - 2.0 * a.y * a.y;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx44_identity(mt: *mut Mtx44) {
    unsafe {
        let mt = &mut *mt;
        mt[0][0] = 1.0;
        mt[0][1] = 0.0;
        mt[0][2] = 0.0;
        mt[0][3] = 0.0;
        mt[1][0] = 0.0;
        mt[1][1] = 1.0;
        mt[1][2] = 0.0;
        mt[1][3] = 0.0;
        mt[2][0] = 0.0;
        mt[2][1] = 0.0;
        mt[2][2] = 1.0;
        mt[2][3] = 0.0;
        mt[3][0] = 0.0;
        mt[3][1] = 0.0;
        mt[3][2] = 0.0;
        mt[3][3] = 1.0;
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// The source and destination may be the same matrix.
pub unsafe fn mtx44_copy(src: *const Mtx44, dst: *mut Mtx44) {
    unsafe {
        if !ptr::eq(src, dst.cast_const()) {
            ptr::copy_nonoverlapping(src, dst, 1);
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx44_concat(a: *const Mtx44, b: *const Mtx44, ab: *mut Mtx44) {
    unsafe {
        let a = &*a;
        let b = &*b;
        let out = ab;
        let mut tmp = [[0.0f32; 4]; 4];
        let tmp_ptr = &mut tmp as *mut Mtx44;
        let m = if ptr::eq(out as *const Mtx44, a as *const Mtx44)
            || ptr::eq(out as *const Mtx44, b as *const Mtx44)
        {
            tmp_ptr
        } else {
            out
        };
        let m = &mut *m;

        for row in 0..4 {
            for col in 0..4 {
                m[row][col] = a[row][0] * b[0][col]
                    + a[row][1] * b[1][col]
                    + a[row][2] * b[2][col]
                    + a[row][3] * b[3][col];
            }
        }

        if ptr::eq(m as *const Mtx44, tmp_ptr as *const Mtx44) {
            {
                *out = tmp;
            }
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn mtx44_inverse(src: *const Mtx44, inv: *mut Mtx44) -> u32 {
    unsafe {
        let src = &*src;
        let out = inv;
        let mut tmp = [[0.0f32; 4]; 4];
        let tmp_ptr = &mut tmp as *mut Mtx44;
        let m = if ptr::eq(src, out as *const Mtx44) {
            tmp_ptr
        } else {
            out
        };
        let m = &mut *m;

        m[0][0] = (src[1][1] * src[2][2] * src[3][3])
            - (src[1][1] * src[2][3] * src[3][2])
            - (src[2][1] * src[1][2] * src[3][3])
            + (src[2][1] * src[1][3] * src[3][2])
            + (src[3][1] * src[1][2] * src[2][3])
            - (src[3][1] * src[1][3] * src[2][2]);

        m[0][1] = -(src[0][1] * src[2][2] * src[3][3])
            + (src[0][1] * src[2][3] * src[3][2])
            + (src[2][1] * src[0][2] * src[3][3])
            - (src[2][1] * src[0][3] * src[3][2])
            - (src[3][1] * src[0][2] * src[2][3])
            + (src[3][1] * src[0][3] * src[2][2]);

        m[0][2] = (src[0][1] * src[1][2] * src[3][3])
            - (src[0][1] * src[1][3] * src[3][2])
            - (src[1][1] * src[0][2] * src[3][3])
            + (src[1][1] * src[0][3] * src[3][2])
            + (src[3][1] * src[0][2] * src[1][3])
            - (src[3][1] * src[0][3] * src[1][2]);

        m[0][3] = -(src[0][1] * src[1][2] * src[2][3])
            + (src[0][1] * src[1][3] * src[2][2])
            + (src[1][1] * src[0][2] * src[2][3])
            - (src[1][1] * src[0][3] * src[2][2])
            - (src[2][1] * src[0][2] * src[1][3])
            + (src[2][1] * src[0][3] * src[1][2]);

        m[1][0] = -(src[1][0] * src[2][2] * src[3][3])
            + (src[1][0] * src[2][3] * src[3][2])
            + (src[2][0] * src[1][2] * src[3][3])
            - (src[2][0] * src[1][3] * src[3][2])
            - (src[3][0] * src[1][2] * src[2][3])
            + (src[3][0] * src[1][3] * src[2][2]);

        m[1][1] = (src[0][0] * src[2][2] * src[3][3])
            - (src[0][0] * src[2][3] * src[3][2])
            - (src[2][0] * src[0][2] * src[3][3])
            + (src[2][0] * src[0][3] * src[3][2])
            + (src[3][0] * src[0][2] * src[2][3])
            - (src[3][0] * src[0][3] * src[2][2]);

        m[1][2] = -(src[0][0] * src[1][2] * src[3][3])
            + (src[0][0] * src[1][3] * src[3][2])
            + (src[1][0] * src[0][2] * src[3][3])
            - (src[1][0] * src[0][3] * src[3][2])
            - (src[3][0] * src[0][2] * src[1][3])
            + (src[3][0] * src[0][3] * src[1][2]);

        m[1][3] = (src[0][0] * src[1][2] * src[2][3])
            - (src[0][0] * src[1][3] * src[2][2])
            - (src[1][0] * src[0][2] * src[2][3])
            + (src[1][0] * src[0][3] * src[2][2])
            + (src[2][0] * src[0][2] * src[1][3])
            - (src[2][0] * src[0][3] * src[1][2]);

        m[2][0] = (src[1][0] * src[2][1] * src[3][3])
            - (src[1][0] * src[2][3] * src[3][1])
            - (src[2][0] * src[1][1] * src[3][3])
            + (src[2][0] * src[1][3] * src[3][1])
            + (src[3][0] * src[1][1] * src[2][3])
            - (src[3][0] * src[1][3] * src[2][1]);

        m[2][1] = -(src[0][0] * src[2][1] * src[3][3])
            + (src[0][0] * src[2][3] * src[3][1])
            + (src[2][0] * src[0][1] * src[3][3])
            - (src[2][0] * src[0][3] * src[3][1])
            - (src[3][0] * src[0][1] * src[2][3])
            + (src[3][0] * src[0][3] * src[2][1]);

        m[2][2] = (src[0][0] * src[1][1] * src[3][3])
            - (src[0][0] * src[1][3] * src[3][1])
            - (src[1][0] * src[0][1] * src[3][3])
            + (src[1][0] * src[0][3] * src[3][1])
            + (src[3][0] * src[0][1] * src[1][3])
            - (src[3][0] * src[0][3] * src[1][1]);

        m[2][3] = -(src[0][0] * src[1][1] * src[2][3])
            + (src[0][0] * src[1][3] * src[2][1])
            + (src[1][0] * src[0][1] * src[2][3])
            - (src[1][0] * src[0][3] * src[2][1])
            - (src[2][0] * src[0][1] * src[1][3])
            + (src[2][0] * src[0][3] * src[1][1]);

        m[3][0] = -(src[1][0] * src[2][1] * src[3][2])
            + (src[1][0] * src[2][2] * src[3][1])
            + (src[2][0] * src[1][1] * src[3][2])
            - (src[2][0] * src[1][2] * src[3][1])
            - (src[3][0] * src[1][1] * src[2][2])
            + (src[3][0] * src[1][2] * src[2][1]);

        m[3][1] = (src[0][0] * src[2][1] * src[3][2])
            - (src[0][0] * src[2][2] * src[3][1])
            - (src[2][0] * src[0][1] * src[3][2])
            + (src[2][0] * src[0][2] * src[3][1])
            + (src[3][0] * src[0][1] * src[2][2])
            - (src[3][0] * src[0][2] * src[2][1]);

        m[3][2] = -(src[0][0] * src[1][1] * src[3][2])
            + (src[0][0] * src[1][2] * src[3][1])
            + (src[1][0] * src[0][1] * src[3][2])
            - (src[1][0] * src[0][2] * src[3][1])
            - (src[3][0] * src[0][1] * src[1][2])
            + (src[3][0] * src[0][2] * src[1][1]);

        m[3][3] = (src[0][0] * src[1][1] * src[2][2])
            - (src[0][0] * src[1][2] * src[2][1])
            - (src[1][0] * src[0][1] * src[2][2])
            + (src[1][0] * src[0][2] * src[2][1])
            + (src[2][0] * src[0][1] * src[1][2])
            - (src[2][0] * src[0][2] * src[1][1]);

        let det =
            src[0][0] * m[0][0] + src[0][1] * m[1][0] + src[0][2] * m[2][0] + src[0][3] * m[3][0];
        if det == 0.0 {
            return 0;
        }

        let det = 1.0 / det;
        for row in m.iter_mut() {
            for col in row.iter_mut() {
                *col *= det;
            }
        }

        if ptr::eq(m as *const Mtx44, tmp_ptr as *const Mtx44) {
            {
                *out = tmp;
            }
        }

        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(x: f32, y: f32, z: f32) -> GuVector {
        GuVector { x, y, z }
    }

    fn quaternion(x: f32, y: f32, z: f32, w: f32) -> GuQuaternion {
        GuQuaternion { x, y, z, w }
    }

    #[test]
    fn matrix_helpers_work() {
        let mut m = [[0.0f32; 4]; 3];
        unsafe { mtx_identity(&mut m) };
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[1][1], 1.0);
        assert_eq!(m[2][2], 1.0);
    }

    #[test]
    fn matrix_operations_support_in_place_outputs() {
        let original: Mtx = [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
        ];
        let mut matrix = original;
        let pointer = &mut matrix as *mut Mtx;

        unsafe { mtx_copy(pointer.cast_const(), pointer) };
        assert_eq!(matrix, original);

        unsafe { mtx_scale_apply(pointer.cast_const(), pointer, 2.0, 3.0, 4.0) };
        assert_eq!(
            matrix,
            [
                [2.0, 4.0, 6.0, 8.0],
                [15.0, 18.0, 21.0, 24.0],
                [36.0, 40.0, 44.0, 48.0],
            ]
        );

        matrix = original;
        unsafe { mtx_apply_scale(pointer.cast_const(), pointer, 2.0, 3.0, 4.0) };
        assert_eq!(
            matrix,
            [
                [2.0, 6.0, 12.0, 4.0],
                [10.0, 18.0, 28.0, 8.0],
                [18.0, 30.0, 44.0, 12.0],
            ]
        );

        matrix = original;
        unsafe { mtx_trans_apply(pointer.cast_const(), pointer, 1.0, 2.0, 3.0) };
        assert_eq!(
            matrix,
            [
                [1.0, 2.0, 3.0, 5.0],
                [5.0, 6.0, 7.0, 10.0],
                [9.0, 10.0, 11.0, 15.0],
            ]
        );

        matrix = original;
        unsafe { mtx_apply_trans(pointer.cast_const(), pointer, 1.0, 2.0, 3.0) };
        assert_eq!(matrix[0][3], 18.0);
        assert_eq!(matrix[1][3], 46.0);
        assert_eq!(matrix[2][3], 74.0);

        let original_44: Mtx44 = [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ];
        let mut matrix_44 = original_44;
        let pointer_44 = &mut matrix_44 as *mut Mtx44;
        unsafe { mtx44_copy(pointer_44.cast_const(), pointer_44) };
        assert_eq!(matrix_44, original_44);
    }

    #[test]
    fn vector_helpers_work() {
        let a = GuVector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let b = GuVector {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };
        let mut out = GuVector::default();
        unsafe { vec_add(&a, &b, &mut out) };
        assert_eq!(
            out,
            GuVector {
                x: 5.0,
                y: 7.0,
                z: 9.0
            }
        );
    }

    #[test]
    fn vector_operations_support_in_place_outputs() {
        let mut a = vector(1.0, 2.0, 3.0);
        let b = vector(4.0, 5.0, 6.0);
        let a_ptr = &mut a as *mut GuVector;

        unsafe {
            vec_add(a_ptr.cast_const(), &b, a_ptr);
        }
        assert_eq!(a, vector(5.0, 7.0, 9.0));

        unsafe {
            vec_sub(a_ptr.cast_const(), &b, a_ptr);
        }
        assert_eq!(a, vector(1.0, 2.0, 3.0));

        unsafe {
            vec_scale(a_ptr.cast_const(), a_ptr, 2.0);
        }
        assert_eq!(a, vector(2.0, 4.0, 6.0));

        unsafe {
            vec_cross(a_ptr.cast_const(), &b, a_ptr);
        }
        assert_eq!(a, vector(-6.0, 12.0, -6.0));

        let matrix: Mtx = [
            [2.0, 0.0, 0.0, 1.0],
            [0.0, 3.0, 0.0, 2.0],
            [0.0, 0.0, 4.0, 3.0],
        ];
        a = vector(1.0, 2.0, 3.0);
        unsafe { vec_multiply(&matrix, a_ptr.cast_const(), a_ptr) };
        assert_eq!(a, vector(3.0, 8.0, 15.0));

        a = vector(1.0, 2.0, 3.0);
        unsafe { vec_multiply_sr(&matrix, a_ptr.cast_const(), a_ptr) };
        assert_eq!(a, vector(2.0, 6.0, 12.0));
    }

    #[test]
    fn half_angle_supports_in_place_output() {
        let mut a = vector(1.0, 0.0, 0.0);
        let pointer = &mut a as *mut GuVector;

        unsafe { vec_half_angle(pointer.cast_const(), pointer.cast_const(), pointer) };

        assert_eq!(a, vector(-1.0, 0.0, 0.0));
    }

    #[test]
    fn quaternion_operations_support_in_place_outputs() {
        let mut a = quaternion(1.0, 2.0, 3.0, 4.0);
        let b = quaternion(4.0, 3.0, 2.0, 1.0);
        let pointer = &mut a as *mut GuQuaternion;

        unsafe { quat_add(pointer.cast_const(), &b, pointer) };
        assert_eq!(a, quaternion(5.0, 5.0, 5.0, 5.0));

        unsafe { quat_sub(pointer.cast_const(), &b, pointer) };
        assert_eq!(a, quaternion(1.0, 2.0, 3.0, 4.0));

        let b = quaternion(5.0, 6.0, 7.0, 8.0);
        unsafe { quat_multiply(pointer.cast_const(), &b, pointer) };
        assert_eq!(a, quaternion(24.0, 48.0, 48.0, -6.0));

        a = quaternion(0.0, 0.0, 0.0, 2.0);
        unsafe { quat_normalize(pointer.cast_const(), pointer) };
        assert_eq!(a, quaternion(0.0, 0.0, 0.0, 1.0));

        a = quaternion(0.0, 0.0, 0.0, 2.0);
        unsafe { quat_inverse(pointer.cast_const(), pointer) };
        assert_eq!(a, quaternion(0.0, 0.0, 0.0, 0.5));
    }

    #[test]
    fn projection_helpers_work() {
        let mut m = [[0.0f32; 4]; 4];
        unsafe { frustum(&mut m, 1.0, -1.0, -1.0, 1.0, 1.0, 2.0) };
        assert_eq!(m[3][2], -1.0);
    }
}
