#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct GuVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct GuQuaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

pub type Mtx = [[f32; 4]; 3];
pub type RoMtx = [[f32; 3]; 4];
pub type Mtx33 = [[f32; 3]; 3];
pub type Mtx44 = [[f32; 4]; 4];

pub const M_PI: f64 = core::f64::consts::PI;
pub const M_DTOR: f64 = core::f64::consts::PI / 180.0;

pub const fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

pub const fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

pub const fn deg_to_rad(a: f32) -> f32 {
    a * 0.017_453_292
}

pub const fn rad_to_deg(a: f32) -> f32 {
    a * 57.295_78
}

pub const fn ftofix32(x: f32) -> i32 {
    (x * 65_536.0) as i32
}

pub const fn fix32tof(x: i32) -> f32 {
    (x as f32) * (1.0 / 65_536.0)
}

pub const fn fttofrac8(x: f32) -> i32 {
    let scaled = x * 128.0;
    let clamped = if scaled < 127.0 { scaled } else { 127.0 };
    (clamped as i32) & 0xff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_types_have_expected_sizes() {
        assert_eq!(core::mem::size_of::<Mtx>(), 48);
        assert_eq!(core::mem::size_of::<Mtx44>(), 64);
        assert_eq!(core::mem::size_of::<GuVector>(), 12);
        assert_eq!(core::mem::size_of::<GuQuaternion>(), 16);
    }

    #[test]
    fn math_constants_match_c_double_precision() {
        let pi: f64 = M_PI;
        let degrees_to_radians: f64 = M_DTOR;

        assert_eq!(pi, core::f64::consts::PI);
        assert_eq!(degrees_to_radians, core::f64::consts::PI / 180.0);
    }

    #[test]
    fn scalar_helpers_match_c_macros() {
        assert_eq!(max(3.0, 5.0), 5.0);
        assert_eq!(min(3.0, 5.0), 3.0);
        assert_eq!(deg_to_rad(180.0), 180.0 * 0.017_453_292_52_f32);
        assert_eq!(rad_to_deg(core::f32::consts::PI), 180.0);
        assert_eq!(ftofix32(1.5), 98_304);
        assert_eq!(fix32tof(98_304), 1.5);
        assert_eq!(fttofrac8(0.5), 64);
        assert_eq!(fttofrac8(2.0), 127);
        assert_eq!(fttofrac8(-1.0), 128);
    }
}
