use libm::{atan2f, atanf, cosf, hypotf, sinf, sqrtf};
use wiiuse_rs::{
    AccelCalibration, BalanceBoardCalibration, BalanceBoardReport, ButtonState, ClassicCalibration,
    ClassicReport, GuitarHero3Report, MotionPlusReport, NunchukCalibration, NunchukReport,
};
pub(crate) use wiiuse_rs::{ReadAssembly, ReadFragmentResult, decode_ack_report};

use crate::{
    Accel, ClassicCtrl, Expansion, FDot, GForce, GuitarHero3, Ir, IrDot, Joystick, MotionPlus,
    Nunchuk, Orient, SensorBar, Vec3w, WPAD_DATA_ACCEL, WPAD_DATA_BUTTONS, WPAD_DATA_EXPANSION,
    WPAD_DATA_IR, WPAD_EXP_CLASSIC, WPAD_EXP_GUITARHERO3, WPAD_EXP_NONE, WPAD_EXP_NUNCHUK,
    WPAD_EXP_UNKNOWN, WPAD_EXP_WIIBOARD, WPAD_FMT_BTNS, WPAD_FMT_BTNS_ACC, WPAD_FMT_BTNS_ACC_IR,
    WiiBoard, WpadData,
};

pub(crate) const DEFAULT_SMOOTH_ALPHA: f32 = 0.3;

const WM_ASPECT_16_9_X: f32 = 660.0;
const WM_ASPECT_16_9_Y: f32 = 370.0;
const WM_ASPECT_4_3_X: f32 = 560.0;
const WM_ASPECT_4_3_Y: f32 = 420.0;

const WM_IR_BLOCK1_LEVEL1: [u8; 9] = [0x02, 0x00, 0x00, 0x71, 0x01, 0x00, 0x64, 0x00, 0xfe];
const WM_IR_BLOCK2_LEVEL1: [u8; 2] = [0xfd, 0x05];
const WM_IR_BLOCK1_LEVEL2: [u8; 9] = [0x02, 0x00, 0x00, 0x71, 0x01, 0x00, 0x96, 0x00, 0xb4];
const WM_IR_BLOCK2_LEVEL2: [u8; 2] = [0xb3, 0x04];
const WM_IR_BLOCK1_LEVEL3: [u8; 9] = [0x02, 0x00, 0x00, 0x71, 0x01, 0x00, 0xaa, 0x00, 0x64];
const WM_IR_BLOCK2_LEVEL3: [u8; 2] = [0x63, 0x03];
const WM_IR_BLOCK1_LEVEL4: [u8; 9] = [0x02, 0x00, 0x00, 0x71, 0x01, 0x00, 0xc8, 0x00, 0x36];
const WM_IR_BLOCK2_LEVEL4: [u8; 2] = [0x35, 0x03];
const WM_IR_BLOCK1_LEVEL5: [u8; 9] = [0x07, 0x00, 0x00, 0x71, 0x01, 0x00, 0x72, 0x00, 0x20];
const WM_IR_BLOCK2_LEVEL5: [u8; 2] = [0x1f, 0x03];

const SB_WIDTH: f32 = 19.5;
const SB_DOT_WIDTH: f32 = 2.25;
const SB_DOT_HEIGHT: f32 = 1.0;
const SB_DOT_WIDTH_RATIO: f32 = SB_DOT_WIDTH / SB_WIDTH;
const SB_DOT_HEIGHT_RATIO: f32 = SB_DOT_HEIGHT / SB_WIDTH;
const SB_Z_COEFFICIENT: f32 = 256.0;
const WIIMOTE_FOV_COEFFICIENT: f32 = 0.39;
const ERROR_MAX_COUNT: u32 = 8;
const GLITCH_MAX_COUNT: u32 = 5;
const GLITCH_DIST: f32 = 150.0 * 150.0;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct IrSensitivity {
    pub block1: [u8; 9],
    pub block2: [u8; 2],
}

#[cfg(test)]
pub(crate) fn is_wiimote_name(name: &[u8]) -> bool {
    name.starts_with(b"Nintendo RVL-CNT-")
}

pub(crate) fn report_type_for_format(fmt: i32, has_expansion: bool) -> Option<(bool, u8)> {
    let (continuous, report) = match fmt {
        WPAD_FMT_BTNS => (false, if has_expansion { 0x34 } else { 0x30 }),
        WPAD_FMT_BTNS_ACC => (true, if has_expansion { 0x35 } else { 0x31 }),
        WPAD_FMT_BTNS_ACC_IR => (true, if has_expansion { 0x37 } else { 0x33 }),
        _ => return None,
    };
    Some((continuous, report))
}

pub(crate) fn ir_mode_for_expansion(has_expansion: bool) -> u8 {
    if has_expansion { 0x01 } else { 0x03 }
}

pub(crate) fn ir_sensitivity_blocks(level: i32) -> IrSensitivity {
    match level.clamp(1, 5) {
        1 => IrSensitivity {
            block1: WM_IR_BLOCK1_LEVEL1,
            block2: WM_IR_BLOCK2_LEVEL1,
        },
        2 => IrSensitivity {
            block1: WM_IR_BLOCK1_LEVEL2,
            block2: WM_IR_BLOCK2_LEVEL2,
        },
        3 => IrSensitivity {
            block1: WM_IR_BLOCK1_LEVEL3,
            block2: WM_IR_BLOCK2_LEVEL3,
        },
        4 => IrSensitivity {
            block1: WM_IR_BLOCK1_LEVEL4,
            block2: WM_IR_BLOCK2_LEVEL4,
        },
        _ => IrSensitivity {
            block1: WM_IR_BLOCK1_LEVEL5,
            block2: WM_IR_BLOCK2_LEVEL5,
        },
    }
}

pub(crate) fn ir_aspect_vres(aspect: i32) -> (i32, [u32; 2]) {
    if aspect == 1 {
        (1, [660, 370])
    } else {
        (0, [560, 420])
    }
}

pub(crate) fn ir_position_offset(aspect: i32, sensor_bar_bottom: bool) -> [i32; 2] {
    let y = if aspect == 1 {
        if sensor_bar_bottom { -70 } else { 70 }
    } else if sensor_bar_bottom {
        -100
    } else {
        100
    };
    [0, y]
}

pub(crate) fn apply_read_report_buttons(current: &mut WpadData, report: &[u8]) -> bool {
    let previous = current.btns_h;
    let Some(transition) = report
        .get(1..)
        .and_then(|button_bytes| ButtonState::decode(button_bytes, previous as u16))
    else {
        return false;
    };
    current.btns_l = previous;
    let expansion_buttons = previous & !u32::from(u16::MAX);
    current.btns_h = expansion_buttons | u32::from(transition.current);
    current.btns_d = u32::from(transition.pressed);
    current.btns_u = u32::from(transition.released);
    current.data_present |= WPAD_DATA_BUTTONS;
    true
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

fn rotate_dots(input: &[FDot], output: &mut [FDot], count: usize, ang: f32) {
    if ang == 0.0 {
        output[..count].copy_from_slice(&input[..count]);
        return;
    }

    let rad = ang.to_radians();
    let s = sinf(rad);
    let c = cosf(rad);
    for (src, dst) in input.iter().zip(output.iter_mut()).take(count) {
        dst.x = (c * src.x) + (-s * src.y);
        dst.y = (s * src.x) + (c * src.y);
    }
}

fn ir_correct_for_bounds(
    x: &mut f32,
    y: &mut f32,
    aspect: i32,
    offset_x: i32,
    offset_y: i32,
) -> bool {
    let (xs, ys) = if aspect == 1 {
        (WM_ASPECT_16_9_X, WM_ASPECT_16_9_Y)
    } else {
        (WM_ASPECT_4_3_X, WM_ASPECT_4_3_Y)
    };

    let x0 = ((1024.0 - xs) / 2.0) + offset_x as f32;
    let y0 = ((768.0 - ys) / 2.0) + offset_y as f32;

    if *x >= x0 && *x <= x0 + xs && *y >= y0 && *y <= y0 + ys {
        *x -= offset_x as f32;
        *y -= offset_y as f32;
        true
    } else {
        false
    }
}

fn ir_convert_to_vres(x: &mut f32, y: &mut f32, aspect: i32, vx: u32, vy: u32) {
    let (xs, ys) = if aspect == 1 {
        (WM_ASPECT_16_9_X, WM_ASPECT_16_9_Y)
    } else {
        (WM_ASPECT_4_3_X, WM_ASPECT_4_3_Y)
    };
    *x -= (1024.0 - xs) / 2.0;
    *y -= (768.0 - ys) / 2.0;
    *x = (*x / xs) * vx as f32;
    *y = (*y / ys) * vy as f32;
}

pub(crate) fn calculate_orientation(
    accel_calib: &Accel,
    accel: &Vec3w,
    orient: &mut Orient,
    smooth: bool,
) {
    let xg = if accel_calib.cal_g.x == 0 {
        1.0
    } else {
        accel_calib.cal_g.x as f32
    };
    let yg = if accel_calib.cal_g.y == 0 {
        1.0
    } else {
        accel_calib.cal_g.y as f32
    };
    let zg = if accel_calib.cal_g.z == 0 {
        1.0
    } else {
        accel_calib.cal_g.z as f32
    };

    orient.yaw = 0.0;

    let mut x = (accel.x as f32 - accel_calib.cal_zero.x as f32) / xg;
    let mut y = (accel.y as f32 - accel_calib.cal_zero.y as f32) / yg;
    let mut z = (accel.z as f32 - accel_calib.cal_zero.z as f32) / zg;

    x = clamp_f32(x, -1.0, 1.0);
    y = clamp_f32(y, -1.0, 1.0);
    z = clamp_f32(z, -1.0, 1.0);

    if (accel.x as i32 - accel_calib.cal_zero.x as i32).abs() <= accel_calib.cal_g.x as i32 + 10 {
        let roll = atan2f(x, z).to_degrees();
        if roll.is_finite() {
            orient.roll = roll;
            orient.a_roll = roll;
        }
    }

    if (accel.y as i32 - accel_calib.cal_zero.y as i32).abs() <= accel_calib.cal_g.y as i32 + 10 {
        let pitch = atan2f(y, z).to_degrees();
        if pitch.is_finite() {
            orient.pitch = pitch;
            orient.a_pitch = pitch;
        }
    }

    if smooth {
        apply_smoothing(accel_calib, orient, 1);
        apply_smoothing(accel_calib, orient, 2);
    }
}

pub(crate) fn calculate_gforce(accel_calib: &Accel, accel: &Vec3w, gforce: &mut GForce) {
    let xg = if accel_calib.cal_g.x == 0 {
        1.0
    } else {
        accel_calib.cal_g.x as f32
    };
    let yg = if accel_calib.cal_g.y == 0 {
        1.0
    } else {
        accel_calib.cal_g.y as f32
    };
    let zg = if accel_calib.cal_g.z == 0 {
        1.0
    } else {
        accel_calib.cal_g.z as f32
    };
    gforce.x = (accel.x as f32 - accel_calib.cal_zero.x as f32) / xg;
    gforce.y = (accel.y as f32 - accel_calib.cal_zero.y as f32) / yg;
    gforce.z = (accel.z as f32 - accel_calib.cal_zero.z as f32) / zg;
}

pub(crate) fn calc_joystick_state(js: &mut Joystick, x: f32, y: f32) {
    let rx = if x == js.center.x as f32 {
        0.0
    } else if x >= js.center.x as f32 {
        (x - js.center.x as f32) / (js.max.x as f32 - js.center.x as f32)
    } else {
        (x - js.min.x as f32) / (js.center.x as f32 - js.min.x as f32) - 1.0
    };
    let ry = if y == js.center.y as f32 {
        0.0
    } else if y >= js.center.y as f32 {
        (y - js.center.y as f32) / (js.max.y as f32 - js.center.y as f32)
    } else {
        (y - js.min.y as f32) / (js.center.y as f32 - js.min.y as f32) - 1.0
    };

    js.ang = atan2f(rx, ry).to_degrees();
    js.mag = hypotf(rx, ry);
}

pub(crate) fn apply_smoothing(accel_calib: &Accel, orient: &mut Orient, kind: i32) {
    let alpha = accel_calib.st_alpha;
    match kind {
        1 => {
            if orient.roll.is_nan() || orient.roll.is_infinite() {
                orient.roll = 0.0;
            }
            if (orient.roll < 0.0 && orient.a_roll > 0.0)
                || (orient.roll > 0.0 && orient.a_roll < 0.0)
            {
                orient.roll = orient.a_roll;
            } else {
                orient.roll = orient.roll + (alpha * (orient.a_roll - orient.roll));
            }
        }
        2 => {
            if orient.pitch.is_nan() || orient.pitch.is_infinite() {
                orient.pitch = 0.0;
            }
            if (orient.pitch < 0.0 && orient.a_pitch > 0.0)
                || (orient.pitch > 0.0 && orient.a_pitch < 0.0)
            {
                orient.pitch = orient.a_pitch;
            } else {
                orient.pitch = orient.pitch + (alpha * (orient.a_pitch - orient.pitch));
            }
        }
        _ => {}
    }
}

fn decode_ir_dot_basic(data: &[u8], idx: usize) -> IrDot {
    let (rx, ry) = match idx {
        0 => (
            1023 - (u16::from(data[0]) | (u16::from(data[2] & 0x30) << 4)),
            u16::from(data[1]) | (u16::from(data[2] & 0xc0) << 2),
        ),
        1 => (
            1023 - (u16::from(data[3]) | (u16::from(data[2] & 0x03) << 8)),
            u16::from(data[4]) | (u16::from(data[2] & 0x0c) << 6),
        ),
        2 => (
            1023 - (u16::from(data[5]) | (u16::from(data[7] & 0x30) << 4)),
            u16::from(data[6]) | (u16::from(data[7] & 0xc0) << 2),
        ),
        _ => (
            1023 - (u16::from(data[8]) | (u16::from(data[7] & 0x03) << 8)),
            u16::from(data[9]) | (u16::from(data[7] & 0x0c) << 6),
        ),
    };
    IrDot {
        visible: if ry == 1023 { 0 } else { 1 },
        rx: rx as i16,
        ry: ry as i16,
        size: 0,
    }
}

fn decode_ir_dot_extended(data: &[u8], idx: usize) -> IrDot {
    let base = idx * 3;
    let rx = 1023 - (u16::from(data[base]) | (u16::from(data[base + 2] & 0x30) << 4));
    let ry = u16::from(data[base + 1]) | (u16::from(data[base + 2] & 0xc0) << 2);
    IrDot {
        visible: if ry == 1023 { 0 } else { 1 },
        rx: rx as i16,
        ry: ry as i16,
        size: data[base + 2] & 0x0f,
    }
}

fn find_sensorbar(ir: &mut Ir, orient: &Orient) {
    let mut dots = [FDot::default(); 4];
    let mut acc_dots = [FDot::default(); 4];
    let mut candidates = [SensorBar::default(); 6];
    let mut num_dots = 0usize;
    let mut num_candidates = 0usize;

    for dot in ir.dot.iter() {
        if dot.visible != 0 {
            dots[num_dots].x = (dot.rx as f32 - 512.0) / 512.0;
            dots[num_dots].y = (dot.ry as f32 - 384.0) / 512.0;
            num_dots += 1;
        }
    }
    ir.num_dots = num_dots as u8;

    if num_dots == 0 {
        if ir.state != 0 {
            ir.state = 3;
        }
        ir.ax = 0.0;
        ir.ay = 0.0;
        ir.distance = 0.0;
        ir.raw_valid = 0;
        return;
    }

    rotate_dots(&dots, &mut acc_dots, num_dots, orient.roll);

    if num_dots > 1 {
        for first in 0..(num_dots - 1) {
            for second in (first + 1)..num_dots {
                let mut cand = SensorBar::default();
                if acc_dots[first].x > acc_dots[second].x {
                    cand.dots[0] = dots[second];
                    cand.dots[1] = dots[first];
                    cand.acc_dots[0] = acc_dots[second];
                    cand.acc_dots[1] = acc_dots[first];
                } else {
                    cand.dots[0] = dots[first];
                    cand.dots[1] = dots[second];
                    cand.acc_dots[0] = acc_dots[first];
                    cand.acc_dots[1] = acc_dots[second];
                }
                let diff_x = cand.acc_dots[1].x - cand.acc_dots[0].x;
                let diff_y = cand.acc_dots[1].y - cand.acc_dots[0].y;
                if (diff_y / diff_x).abs() > 0.7 {
                    continue;
                }
                cand.off_angle = -atan2f(diff_y, diff_x).to_degrees();
                cand.angle = cand.off_angle + orient.roll;
                rotate_dots(&cand.dots, &mut cand.rot_dots, 2, cand.angle);
                let diff_x = cand.rot_dots[1].x - cand.rot_dots[0].x;
                if diff_x < 0.1 {
                    continue;
                }

                let mut rejected = false;
                for (i, dot) in dots.iter().enumerate().take(num_dots) {
                    if i == first || i == second {
                        continue;
                    }
                    let hadj = SB_DOT_HEIGHT_RATIO * diff_x;
                    let wadj = SB_DOT_WIDTH_RATIO * diff_x;
                    let mut tdot = FDot::default();
                    rotate_dots(
                        core::slice::from_ref(dot),
                        core::slice::from_mut(&mut tdot),
                        1,
                        cand.angle,
                    );
                    if (cand.rot_dots[0].x + wadj) < tdot.x
                        && (cand.rot_dots[1].x - wadj) > tdot.x
                        && (cand.rot_dots[0].y + hadj) > tdot.y
                        && (cand.rot_dots[0].y - hadj) < tdot.y
                    {
                        rejected = true;
                        break;
                    }
                }
                if rejected {
                    continue;
                }

                cand.score = 1.0 / (cand.rot_dots[1].x - cand.rot_dots[0].x);
                candidates[num_candidates] = cand;
                num_candidates += 1;
            }
        }
    }

    let mut sb = SensorBar::default();
    if num_candidates == 0 {
        if ir.state == 0 {
            ir.ax = 0.0;
            ir.ay = 0.0;
            ir.distance = 0.0;
            ir.raw_valid = 0;
            return;
        }

        let mut closest = 0usize;
        let mut closest_to = 0usize;
        let mut best = f32::MAX;
        for (i, acc_dot) in acc_dots.iter().enumerate().take(num_dots) {
            for (j, sensor_dot) in ir.sensorbar.acc_dots.iter().enumerate() {
                let dx = acc_dot.x - sensor_dot.x;
                let dy = acc_dot.y - sensor_dot.y;
                let d = dx * dx + dy * dy;
                if d < best {
                    best = d;
                    closest = i;
                    closest_to = j;
                }
            }
        }

        sb.acc_dots[closest_to] = acc_dots[closest];
        sb.acc_dots[closest_to ^ 1].x = ir.sensorbar.acc_dots[closest_to ^ 1].x
            - ir.sensorbar.acc_dots[closest_to].x
            + acc_dots[closest].x;
        sb.acc_dots[closest_to ^ 1].y = ir.sensorbar.acc_dots[closest_to ^ 1].y
            - ir.sensorbar.acc_dots[closest_to].y
            + acc_dots[closest].y;
        rotate_dots(&sb.acc_dots, &mut sb.dots, 2, -orient.roll);
        sb.off_angle = -atan2f(
            sb.acc_dots[1].y - sb.acc_dots[0].y,
            sb.acc_dots[1].x - sb.acc_dots[0].x,
        )
        .to_degrees();
        sb.angle = ir.sensorbar.off_angle + orient.roll;
        rotate_dots(&sb.acc_dots, &mut sb.rot_dots, 2, ir.sensorbar.off_angle);
        sb.score = 0.0;
        ir.state = 2;
    } else {
        let mut best = f32::MIN;
        for cand in candidates.iter().take(num_candidates) {
            if cand.score > best {
                best = cand.score;
                sb = *cand;
            }
        }
        ir.state = 1;
    }

    ir.raw_valid = 1;
    ir.ax = ((sb.rot_dots[0].x + sb.rot_dots[1].x) / 2.0) * 512.0 + 512.0;
    ir.ay = ((sb.rot_dots[0].y + sb.rot_dots[1].y) / 2.0) * 512.0 + 384.0;
    ir.sensorbar = sb;
    ir.distance = (sb.rot_dots[1].x - sb.rot_dots[0].x) * 512.0;
}

fn apply_ir_smoothing(ir: &mut Ir) {
    let dx = ir.ax - ir.sx;
    let dy = ir.ay - ir.sy;
    let d = sqrtf(dx * dx + dy * dy);
    if d > 2.5 {
        if d < 8.0 {
            ir.sx += dx * 0.25;
            ir.sy += dy * 0.25;
        } else {
            let theta = atan2f(dy, dx);
            ir.sx = ir.ax - cosf(theta) * 8.0;
            ir.sy = ir.ay - sinf(theta) * 8.0;
        }
    }
}

pub(crate) fn interpret_ir_data(ir: &mut Ir, orient: &mut Orient) {
    find_sensorbar(ir, orient);
    if ir.raw_valid != 0 {
        ir.angle = ir.sensorbar.angle;
        ir.z = SB_Z_COEFFICIENT / ir.distance.max(1.0);
        orient.yaw = calc_yaw(ir);
        if ir.error_cnt >= ERROR_MAX_COUNT as f32 {
            ir.sx = ir.ax;
            ir.sy = ir.ay;
            ir.glitch_cnt = 0.0;
        } else {
            let dx = ir.ax - ir.sx;
            let dy = ir.ay - ir.sy;
            let d = dx * dx + dy * dy;
            if d > GLITCH_DIST {
                if ir.glitch_cnt > GLITCH_MAX_COUNT as f32 {
                    apply_ir_smoothing(ir);
                    ir.glitch_cnt = 0.0;
                } else {
                    ir.glitch_cnt += 1.0;
                }
            } else {
                ir.glitch_cnt = 0.0;
                apply_ir_smoothing(ir);
            }
        }
        ir.smooth_valid = 1;
        ir.error_cnt = 0.0;
    } else if ir.error_cnt >= ERROR_MAX_COUNT as f32 {
        ir.smooth_valid = 0;
    } else {
        ir.smooth_valid = 1;
        ir.error_cnt += 1.0;
    }

    if ir.smooth_valid != 0 {
        let mut x = ir.sx;
        let mut y = ir.sy;
        if ir_correct_for_bounds(&mut x, &mut y, ir.aspect, ir.offset[0], ir.offset[1]) {
            ir_convert_to_vres(&mut x, &mut y, ir.aspect, ir.vres[0], ir.vres[1]);
            ir.x = x;
            ir.y = y;
            ir.valid = 1;
        } else {
            ir.valid = 0;
        }
    } else {
        ir.valid = 0;
    }
}

pub(crate) fn calc_yaw(ir: &Ir) -> f32 {
    let mut x = ir.ax - 512.0;
    x *= WIIMOTE_FOV_COEFFICIENT / 512.0;
    atanf(x).to_degrees()
}

fn decode_base_buttons(report: &[u8]) -> Option<u32> {
    report
        .get(1..)
        .and_then(|button_bytes| ButtonState::decode(button_bytes, 0))
        .map(|state| u32::from(state.current))
}

fn decode_accel(report: &[u8]) -> Vec3w {
    Vec3w {
        x: (u16::from(report[3]) << 2) | u16::from((report[1] >> 5) & 3),
        y: (u16::from(report[4]) << 2) | u16::from((report[2] >> 4) & 2),
        z: (u16::from(report[5]) << 2) | u16::from((report[2] >> 5) & 2),
    }
}

fn previous_expansion_buttons(prev: &WpadData) -> u32 {
    match prev.exp.type_ {
        x if x == WPAD_EXP_NONE as i32 => 0,
        x if x == WPAD_EXP_NUNCHUK as i32 => unsafe { prev.exp.u.nunchuk.btns as u32 },
        x if x == WPAD_EXP_CLASSIC as i32 => unsafe { prev.exp.u.classic.btns },
        x if x == WPAD_EXP_GUITARHERO3 as i32 => unsafe { prev.exp.u.gh3.btns as u32 },
        _ => 0,
    }
}

fn update_nunchuk_state(prev: &WpadData, current: &mut WpadData, report: &[u8]) -> u32 {
    let Some(report) = NunchukReport::decode(report) else {
        return 0;
    };
    unsafe {
        let nunchuk = &mut current.exp.u.nunchuk;
        let prev_buttons = previous_expansion_buttons(prev) as u8 & 0x03;
        let button_state = ButtonState::decode_active_low(report.buttons, 0x03, prev_buttons);
        let buttons = button_state.current as u8;
        nunchuk.btns_last = prev_buttons;
        nunchuk.btns_held = button_state.held as u8;
        nunchuk.btns_released = button_state.released as u8;
        nunchuk.btns = buttons;
        nunchuk.js.pos.x = report.stick_x;
        nunchuk.js.pos.y = report.stick_y;
        if nunchuk.js.center.x != 0 {
            nunchuk.js.min.x = nunchuk.js.min.x.min(nunchuk.js.pos.x);
            nunchuk.js.max.x = nunchuk.js.max.x.max(nunchuk.js.pos.x);
        }
        if nunchuk.js.center.y != 0 {
            nunchuk.js.min.y = nunchuk.js.min.y.min(nunchuk.js.pos.y);
            nunchuk.js.max.y = nunchuk.js.max.y.max(nunchuk.js.pos.y);
        }
        let x = nunchuk.js.pos.x;
        let y = nunchuk.js.pos.y;
        calc_joystick_state(&mut nunchuk.js, x as f32, y as f32);
        nunchuk.accel.x = report.accel_x;
        nunchuk.accel.y = report.accel_y;
        nunchuk.accel.z = report.accel_z;
        calculate_orientation(
            &nunchuk.accel_calib,
            &nunchuk.accel,
            &mut nunchuk.orient,
            false,
        );
        calculate_gforce(&nunchuk.accel_calib, &nunchuk.accel, &mut nunchuk.gforce);
        current.exp.type_ = WPAD_EXP_NUNCHUK as i32;
        u32::from(buttons) << 16
    }
}

fn update_classic_state(prev: &WpadData, current: &mut WpadData, report: &[u8]) -> u32 {
    let wii_u_pro = unsafe { current.exp.u.classic.type_ == 2 };
    let previous_buttons = previous_expansion_buttons(prev);
    let Some(report) = ClassicReport::decode(report, wii_u_pro, previous_buttons) else {
        return 0;
    };
    unsafe {
        let classic = &mut current.exp.u.classic;
        classic.btns_last = report.buttons_last;
        classic.btns_held = report.buttons_held;
        classic.btns_released = report.buttons_released;
        classic.btns = report.buttons;
        classic.ls_raw = report.left_trigger;
        classic.rs_raw = report.right_trigger;
        classic.ljs.pos.x = report.left_stick[0];
        classic.ljs.pos.y = report.left_stick[1];
        classic.rjs.pos.x = report.right_stick[0];
        classic.rjs.pos.y = report.right_stick[1];
        classic.charging = u8::from(report.charging);
        classic.wired = u8::from(report.wired);
        classic.battery = report.battery;
        classic.r_shoulder = classic.rs_raw as f32 / 0x1f as f32;
        classic.l_shoulder = classic.ls_raw as f32 / 0x1f as f32;
        let lx = classic.ljs.pos.x;
        let ly = classic.ljs.pos.y;
        let rx = classic.rjs.pos.x;
        let ry = classic.rjs.pos.y;
        calc_joystick_state(&mut classic.ljs, lx as f32, ly as f32);
        calc_joystick_state(&mut classic.rjs, rx as f32, ry as f32);
        current.exp.type_ = WPAD_EXP_CLASSIC as i32;
        report.wpad_buttons(wii_u_pro)
    }
}

fn update_gh3_state(prev: &WpadData, current: &mut WpadData, report: &[u8]) -> u32 {
    let Some(report) = GuitarHero3Report::decode(report, previous_expansion_buttons(prev) as u16)
    else {
        return 0;
    };
    unsafe {
        let gh3 = &mut current.exp.u.gh3;
        let buttons = report.buttons.current;
        gh3.btns_last = report.buttons.previous as i16;
        gh3.btns_held = report.buttons.held as i16;
        gh3.btns_released = report.buttons.released as i16;
        gh3.btns = buttons as i16;
        gh3.js.pos.x = report.stick[0];
        gh3.js.pos.y = report.stick[1];
        gh3.tb_raw = report.touch_raw;
        gh3.wb_raw = report.whammy_raw;
        gh3.touch_bar = i32::from(report.analog.touch_bar);
        gh3.whammy_bar = report.analog.whammy_bar;
        let x = gh3.js.pos.x;
        let y = gh3.js.pos.y;
        calc_joystick_state(&mut gh3.js, x as f32, y as f32);
        current.exp.type_ = WPAD_EXP_GUITARHERO3 as i32;
        u32::from(buttons) << 16
    }
}

fn update_wiiboard_state(current: &mut WpadData, report: &[u8]) -> u32 {
    unsafe {
        let wb = &mut current.exp.u.wb;
        let calibration = BalanceBoardCalibration {
            top_right: wb.ctr.map(|value| value as u16),
            bottom_right: wb.cbr.map(|value| value as u16),
            top_left: wb.ctl.map(|value| value as u16),
            bottom_left: wb.cbl.map(|value| value as u16),
        };
        if let Some(board_report) = BalanceBoardReport::decode(report, &calibration) {
            wb.rtr = board_report.raw_top_right as i16;
            wb.rbr = board_report.raw_bottom_right as i16;
            wb.rtl = board_report.raw_top_left as i16;
            wb.rbl = board_report.raw_bottom_left as i16;
            wb.tr = board_report.top_right;
            wb.br = board_report.bottom_right;
            wb.tl = board_report.top_left;
            wb.bl = board_report.bottom_left;
        }
        current.exp.type_ = WPAD_EXP_WIIBOARD as i32;
    }
    0
}

fn update_motion_plus_state(current: &mut WpadData, report: &[u8]) -> u32 {
    let Some(report) = MotionPlusReport::decode(report) else {
        return 0;
    };
    unsafe {
        let mp = &mut current.exp.u.mp;
        mp.rx = report.rx;
        mp.ry = report.ry;
        mp.rz = report.rz;
        mp.ext = report.ext;
        mp.status = report.status;
        current.exp.type_ = WPAD_EXP_UNKNOWN as i32;
    }
    0
}

fn expansion_report_payload<'a>(
    report: &'a [u8],
    offset: usize,
    current: &WpadData,
) -> Option<&'a [u8]> {
    let payload_len = match current.exp.type_ {
        x if x == WPAD_EXP_WIIBOARD as i32 => 8,
        x if x == WPAD_EXP_CLASSIC as i32 && unsafe { current.exp.u.classic.type_ == 2 } => 11,
        _ => 6,
    };
    report.get(offset..offset + payload_len)
}

fn merge_expansion_buttons(base_buttons: u32, expansion_buttons: u32, current: &WpadData) -> u32 {
    let is_wii_u_pro =
        current.exp.type_ == WPAD_EXP_CLASSIC as i32 && unsafe { current.exp.u.classic.type_ == 2 };
    if is_wii_u_pro {
        expansion_buttons
    } else {
        base_buttons | expansion_buttons
    }
}

fn expansion_buttons_and_state(prev: &WpadData, current: &mut WpadData, report: &[u8]) -> u32 {
    match current.exp.type_ {
        x if x == WPAD_EXP_NUNCHUK as i32 => update_nunchuk_state(prev, current, report),
        x if x == WPAD_EXP_CLASSIC as i32 => update_classic_state(prev, current, report),
        x if x == WPAD_EXP_GUITARHERO3 as i32 => update_gh3_state(prev, current, report),
        x if x == WPAD_EXP_WIIBOARD as i32 => update_wiiboard_state(current, report),
        x if x == WPAD_EXP_UNKNOWN as i32 => update_motion_plus_state(current, report),
        _ => 0,
    }
}

pub(crate) fn wiimote_calibration_or_default(data: &[u8]) -> Accel {
    let calibration = AccelCalibration::decode(data).unwrap_or_else(AccelCalibration::fallback);
    accel_from_calibration(calibration)
}

fn accel_from_calibration(calibration: AccelCalibration) -> Accel {
    Accel {
        cal_zero: Vec3w {
            x: calibration.zero[0],
            y: calibration.zero[1],
            z: calibration.zero[2],
        },
        cal_g: Vec3w {
            x: calibration.one_g[0],
            y: calibration.one_g[1],
            z: calibration.one_g[2],
        },
        st_roll: 0.0,
        st_pitch: 0.0,
        st_alpha: DEFAULT_SMOOTH_ALPHA,
    }
}

pub(crate) fn shape_wiimote_input_report(
    prev: &WpadData,
    current: &mut WpadData,
    accel_calib: &Accel,
    report: &[u8],
) -> bool {
    let Some(report_type) = report.first().copied() else {
        return false;
    };
    if !(0x30..=0x37).contains(&report_type) {
        return false;
    }
    let Some(mut buttons) = decode_base_buttons(report) else {
        return false;
    };

    current.err = crate::WPAD_ERR_NONE;
    current.data_present = 0;

    match report_type {
        0x30 => {
            current.data_present |= WPAD_DATA_BUTTONS;
        }
        0x31 => {
            if report.len() < 6 {
                return false;
            }
            current.accel = decode_accel(report);
            calculate_orientation(accel_calib, &current.accel, &mut current.orient, false);
            calculate_gforce(accel_calib, &current.accel, &mut current.gforce);
            current.data_present |= WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL;
        }
        0x33 => {
            if report.len() < 18 {
                return false;
            }
            current.accel = decode_accel(report);
            calculate_orientation(accel_calib, &current.accel, &mut current.orient, false);
            calculate_gforce(accel_calib, &current.accel, &mut current.gforce);
            current.ir.dot = [
                decode_ir_dot_extended(&report[6..], 0),
                decode_ir_dot_extended(&report[6..], 1),
                decode_ir_dot_extended(&report[6..], 2),
                decode_ir_dot_extended(&report[6..], 3),
            ];
            interpret_ir_data(&mut current.ir, &mut current.orient);
            current.data_present |= WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL | WPAD_DATA_IR;
        }
        0x34 => {
            let Some(expansion_report) = expansion_report_payload(report, 3, current) else {
                return false;
            };
            let exp_buttons = expansion_buttons_and_state(prev, current, expansion_report);
            buttons = merge_expansion_buttons(buttons, exp_buttons, current);
            current.data_present |= WPAD_DATA_BUTTONS | WPAD_DATA_EXPANSION;
        }
        0x35 => {
            if report.len() < 6 {
                return false;
            }
            current.accel = decode_accel(report);
            calculate_orientation(accel_calib, &current.accel, &mut current.orient, false);
            calculate_gforce(accel_calib, &current.accel, &mut current.gforce);
            let Some(expansion_report) = expansion_report_payload(report, 6, current) else {
                return false;
            };
            let exp_buttons = expansion_buttons_and_state(prev, current, expansion_report);
            buttons = merge_expansion_buttons(buttons, exp_buttons, current);
            current.data_present |= WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL | WPAD_DATA_EXPANSION;
        }
        0x36 => {
            if report.len() < 13 {
                return false;
            }
            current.ir.dot = [
                decode_ir_dot_basic(&report[3..], 0),
                decode_ir_dot_basic(&report[3..], 1),
                decode_ir_dot_basic(&report[3..], 2),
                decode_ir_dot_basic(&report[3..], 3),
            ];
            interpret_ir_data(&mut current.ir, &mut current.orient);
            let Some(expansion_report) = expansion_report_payload(report, 13, current) else {
                return false;
            };
            let exp_buttons = expansion_buttons_and_state(prev, current, expansion_report);
            buttons = merge_expansion_buttons(buttons, exp_buttons, current);
            current.data_present |= WPAD_DATA_BUTTONS | WPAD_DATA_IR | WPAD_DATA_EXPANSION;
        }
        0x37 => {
            if report.len() < 16 {
                return false;
            }
            current.accel = decode_accel(report);
            calculate_orientation(accel_calib, &current.accel, &mut current.orient, false);
            calculate_gforce(accel_calib, &current.accel, &mut current.gforce);
            current.ir.dot = [
                decode_ir_dot_basic(&report[6..], 0),
                decode_ir_dot_basic(&report[6..], 1),
                decode_ir_dot_basic(&report[6..], 2),
                decode_ir_dot_basic(&report[6..], 3),
            ];
            interpret_ir_data(&mut current.ir, &mut current.orient);
            let Some(expansion_report) = expansion_report_payload(report, 16, current) else {
                return false;
            };
            let exp_buttons = expansion_buttons_and_state(prev, current, expansion_report);
            buttons = merge_expansion_buttons(buttons, exp_buttons, current);
            current.data_present |=
                WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL | WPAD_DATA_IR | WPAD_DATA_EXPANSION;
        }
        _ => return false,
    }

    current.btns_l = prev.btns_h;
    current.btns_h = buttons;
    current.btns_d = buttons & !prev.btns_h;
    current.btns_u = prev.btns_h & !buttons;
    true
}

pub(crate) fn decode_expansion_calibration(data: &[u8], exp: &mut Expansion) -> bool {
    if data.len() < 224 {
        return false;
    }

    let id = u32::from_be_bytes([data[220], data[221], data[222], data[223]]);
    match id {
        0xa4200000 => {
            let Some(calibration) = NunchukCalibration::decode(data) else {
                return false;
            };
            let mut nunchuk = Nunchuk {
                flags: core::ptr::null_mut(),
                ..Nunchuk::default()
            };
            nunchuk.accel_calib.cal_zero.x = calibration.accel_zero[0];
            nunchuk.accel_calib.cal_zero.y = calibration.accel_zero[1];
            nunchuk.accel_calib.cal_zero.z = calibration.accel_zero[2];
            nunchuk.accel_calib.cal_g.x = calibration.accel_g[0];
            nunchuk.accel_calib.cal_g.y = calibration.accel_g[1];
            nunchuk.accel_calib.cal_g.z = calibration.accel_g[2];
            nunchuk.js.max.x = calibration.stick_max[0];
            nunchuk.js.min.x = calibration.stick_min[0];
            nunchuk.js.center.x = calibration.stick_center[0];
            nunchuk.js.max.y = calibration.stick_max[1];
            nunchuk.js.min.y = calibration.stick_min[1];
            nunchuk.js.center.y = calibration.stick_center[1];
            nunchuk.accel_calib.st_alpha = DEFAULT_SMOOTH_ALPHA;
            exp.type_ = WPAD_EXP_NUNCHUK as i32;
            exp.u.nunchuk = nunchuk;
            true
        }
        0xa4200103 => {
            let mut gh3 = GuitarHero3 {
                touch_bar: -1,
                ..GuitarHero3::default()
            };
            gh3.js.max.x = 0x3c;
            gh3.js.min.x = 0x05;
            gh3.js.center.x = 0x20;
            gh3.js.max.y = 0x3a;
            gh3.js.min.y = 0x05;
            gh3.js.center.y = 0x20;
            exp.type_ = WPAD_EXP_GUITARHERO3 as i32;
            exp.u.gh3 = gh3;
            true
        }
        0xa4200402 => {
            let Some(calibration) = BalanceBoardCalibration::decode(data) else {
                return false;
            };
            let wb = WiiBoard {
                ctr: calibration.top_right.map(|value| value as i16),
                cbr: calibration.bottom_right.map(|value| value as i16),
                ctl: calibration.top_left.map(|value| value as i16),
                cbl: calibration.bottom_left.map(|value| value as i16),
                ..WiiBoard::default()
            };
            exp.type_ = WPAD_EXP_WIIBOARD as i32;
            exp.u.wb = wb;
            true
        }
        0xa4200405 => {
            exp.type_ = WPAD_EXP_UNKNOWN as i32;
            exp.u.mp = MotionPlus::default();
            true
        }
        _ => {
            let Some(calibration) = ClassicCalibration::decode(data) else {
                return false;
            };
            let mut classic = ClassicCtrl {
                type_: calibration.type_,
                ..ClassicCtrl::default()
            };
            classic.ljs.max.x = calibration.left_max[0];
            classic.ljs.max.y = calibration.left_max[1];
            classic.ljs.min.x = calibration.left_min[0];
            classic.ljs.min.y = calibration.left_min[1];
            classic.ljs.center.x = calibration.left_center[0];
            classic.ljs.center.y = calibration.left_center[1];
            classic.rjs.max.x = calibration.right_max[0];
            classic.rjs.max.y = calibration.right_max[1];
            classic.rjs.min.x = calibration.right_min[0];
            classic.rjs.min.y = calibration.right_min[1];
            classic.rjs.center.x = calibration.right_center[0];
            classic.rjs.center.y = calibration.right_center[1];
            exp.type_ = WPAD_EXP_CLASSIC as i32;
            exp.u.classic = classic;
            true
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use crate::WPAD_BUTTON_A;
    use crate::WPAD_MAX_IR_DOTS;

    #[test]
    fn constants_and_shapes_exist() {
        assert_eq!(WPAD_MAX_IR_DOTS, 4);
        assert_eq!(WPAD_BUTTON_A, 0x0008);
        assert_eq!(WPAD_EXP_NUNCHUK, 1);
        assert_eq!(WPAD_FMT_BTNS_ACC_IR, 2);
    }

    #[test]
    fn report_type_mapping_matches_expansion_variants() {
        assert_eq!(
            report_type_for_format(WPAD_FMT_BTNS, false),
            Some((false, 0x30))
        );
        assert_eq!(
            report_type_for_format(WPAD_FMT_BTNS, true),
            Some((false, 0x34))
        );
        assert_eq!(
            report_type_for_format(WPAD_FMT_BTNS_ACC, false),
            Some((true, 0x31))
        );
        assert_eq!(
            report_type_for_format(WPAD_FMT_BTNS_ACC, true),
            Some((true, 0x35))
        );
        assert_eq!(
            report_type_for_format(WPAD_FMT_BTNS_ACC_IR, false),
            Some((true, 0x33))
        );
        assert_eq!(
            report_type_for_format(WPAD_FMT_BTNS_ACC_IR, true),
            Some((true, 0x37))
        );
    }

    #[test]
    fn status_and_ack_parsing_work() {
        let ack = decode_ack_report(&[0x22, 0, 0, 0x17, 0]).unwrap();
        assert_eq!(ack.command, 0x17);
        assert_eq!(ack.status, 0);
    }

    #[test]
    fn wiimote_name_detection_uses_prefix_only() {
        assert!(is_wiimote_name(b"Nintendo RVL-CNT-01"));
        assert!(!is_wiimote_name(b"Nintendo RVL-XXX-01"));
    }

    #[test]
    fn wiimote_calibration_uses_wiiuse_decoder_and_handles_short_blocks() {
        let accel = wiimote_calibration_or_default(&[0; 8]);
        assert_eq!(accel.cal_zero.x, 499);
        assert_eq!(accel.cal_zero.y, 509);
        assert_eq!(accel.cal_zero.z, 507);
        assert_eq!(accel.cal_g.x, 703);
        assert_eq!(accel.cal_g.y, 709);
        assert_eq!(accel.cal_g.z, 709);
        let fallback = wiimote_calibration_or_default(&[0; 7]);
        assert_eq!(fallback.cal_zero.x, 499);
        assert_eq!(fallback.cal_g.z, 709);
    }

    #[test]
    fn read_reports_update_button_transitions_without_a_pending_read() {
        let mut current = WpadData::default();
        assert!(apply_read_report_buttons(
            &mut current,
            &[0x21, 0, 0x08, 0, 0, 0]
        ));
        assert_eq!(current.btns_h, WPAD_BUTTON_A);
        assert_eq!(current.btns_d, WPAD_BUTTON_A);
        assert_eq!(current.btns_u, 0);
        assert!(apply_read_report_buttons(
            &mut current,
            &[0x21, 0, 0, 0, 0, 0]
        ));
        assert_eq!(current.btns_h, 0);
        assert_eq!(current.btns_d, 0);
        assert_eq!(current.btns_u, WPAD_BUTTON_A);
        assert_ne!(current.data_present & WPAD_DATA_BUTTONS, 0);
    }

    #[test]
    fn read_reports_preserve_held_expansion_buttons() {
        let mut current = WpadData {
            btns_h: crate::WPAD_NUNCHUK_BUTTON_Z,
            ..WpadData::default()
        };
        assert!(apply_read_report_buttons(
            &mut current,
            &[0x21, 0, 0x08, 0, 0, 0]
        ));
        assert_eq!(current.btns_h, crate::WPAD_NUNCHUK_BUTTON_Z | WPAD_BUTTON_A);
        assert_eq!(current.btns_u, 0);
        assert_eq!(current.btns_d, WPAD_BUTTON_A);
    }

    #[test]
    fn nunchuk_report_uses_shared_stick_accel_and_button_decoder() {
        let prev = WpadData::default();
        let mut current = WpadData::default();
        current.exp.type_ = WPAD_EXP_NUNCHUK as i32;

        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &Accel::default(),
            &[0x34, 0, 0, 0x2a, 0x35, 0x12, 0x34, 0x56, 0xc9]
        ));
        let nunchuk = unsafe { current.exp.u.nunchuk };
        assert_eq!([nunchuk.js.pos.x, nunchuk.js.pos.y], [0x2a, 0x35]);
        assert_eq!(
            [nunchuk.accel.x, nunchuk.accel.y, nunchuk.accel.z],
            [74, 208, 347]
        );
        assert_eq!(nunchuk.btns, 0x02);
        assert_eq!(current.btns_h, 0x0002_0000);
    }

    #[test]
    fn expansion_calibration_classifies_common_extensions() {
        let mut exp = Expansion::default();
        let mut data = [0u8; 224];

        data[220..224].copy_from_slice(&0xa4200000u32.to_be_bytes());
        assert!(decode_expansion_calibration(&data, &mut exp));
        assert_eq!(exp.type_, WPAD_EXP_NUNCHUK as i32);

        data[220..224].copy_from_slice(&0xa4200103u32.to_be_bytes());
        assert!(decode_expansion_calibration(&data, &mut exp));
        assert_eq!(exp.type_, WPAD_EXP_GUITARHERO3 as i32);

        data[223] = 0x20;
        data[220..224].copy_from_slice(&0x12345678u32.to_be_bytes());
        assert!(decode_expansion_calibration(&data, &mut exp));
        assert_eq!(exp.type_, WPAD_EXP_CLASSIC as i32);
    }

    #[test]
    fn classic_expansion_uses_shared_calibration_decoder() {
        let mut data = [0u8; 224];
        data[..12].copy_from_slice(&[252, 24, 128, 252, 24, 128, 248, 8, 128, 248, 8, 128]);
        data[218] = 1;
        data[220..].copy_from_slice(&0x1234_5678u32.to_be_bytes());
        let mut expansion = Expansion::default();

        assert!(decode_expansion_calibration(&data, &mut expansion));
        assert_eq!(expansion.type_, WPAD_EXP_CLASSIC as i32);
        let classic = unsafe { expansion.u.classic };
        assert_eq!(classic.type_, 1);
        assert_eq!(
            [classic.ljs.max.x, classic.ljs.max.y, classic.ljs.center.x],
            [63, 63, 32]
        );
        assert_eq!(
            [classic.rjs.max.x, classic.rjs.max.y, classic.rjs.center.x],
            [31, 31, 16]
        );
    }

    #[test]
    fn nunchuk_calibration_populates_wpad_fields_from_real_layout() {
        let mut data = [0u8; 224];
        data[..14].copy_from_slice(&[
            10, 20, 30, 0b10_01_11, 40, 50, 60, 0b01_10_11, 223, 27, 126, 222, 30, 131,
        ]);
        data[220..].copy_from_slice(&0xa420_0000u32.to_be_bytes());
        let mut expansion = Expansion::default();

        assert!(decode_expansion_calibration(&data, &mut expansion));
        assert_eq!(expansion.type_, WPAD_EXP_NUNCHUK as i32);
        let nunchuk = unsafe { expansion.u.nunchuk };
        assert_eq!(
            [
                nunchuk.accel_calib.cal_zero.x,
                nunchuk.accel_calib.cal_zero.y,
                nunchuk.accel_calib.cal_zero.z,
            ],
            [42, 81, 123]
        );
        assert_eq!(
            [
                nunchuk.accel_calib.cal_g.x,
                nunchuk.accel_calib.cal_g.y,
                nunchuk.accel_calib.cal_g.z,
            ],
            [119, 121, 120]
        );
        assert_eq!([nunchuk.js.max.x, nunchuk.js.max.y], [223, 222]);
        assert_eq!([nunchuk.js.min.x, nunchuk.js.min.y], [27, 30]);
        assert_eq!([nunchuk.js.center.x, nunchuk.js.center.y], [126, 131]);
    }

    #[test]
    fn expansion_calibration_classifies_motion_plus() {
        let mut exp = Expansion::default();
        let mut data = [0u8; 224];
        data[220..224].copy_from_slice(&0xa4200405u32.to_be_bytes());
        assert!(decode_expansion_calibration(&data, &mut exp));
        assert_eq!(exp.type_, WPAD_EXP_UNKNOWN as i32);
    }

    #[test]
    fn motion_plus_report_uses_shared_axis_and_status_decoder() {
        let mut current = WpadData::default();
        current.exp.type_ = WPAD_EXP_UNKNOWN as i32;
        let report = [0x34, 0, 0, 0x12, 0x34, 0x56, 0xab, 0xcf, 0xef];

        assert!(shape_wiimote_input_report(
            &WpadData::default(),
            &mut current,
            &Accel::default(),
            &report
        ));
        let motion_plus = unsafe { current.exp.u.mp };
        assert_eq!(
            [motion_plus.rx, motion_plus.ry, motion_plus.rz],
            [0x3b56, 0x3334, 0x2a12]
        );
        assert_eq!(motion_plus.ext, 1);
        assert_eq!(motion_plus.status, 7);
        assert_ne!(current.data_present & WPAD_DATA_EXPANSION, 0);
    }

    #[test]
    fn wiiboard_calibration_and_report_fill_raw_and_calibrated_fields() {
        let mut calibration_data = [0u8; 224];
        calibration_data[220..224].copy_from_slice(&0xa4200402u32.to_be_bytes());
        let values = [
            100u16, 110, 120, 130, // zero calibration, TR BR TL BL
            200, 210, 220, 230, // middle calibration, TR BR TL BL
            300, 310, 320, 330, // full calibration, TR BR TL BL
        ];
        for (index, value) in values.into_iter().enumerate() {
            let offset = 4 + index * 2;
            calibration_data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        let mut exp = Expansion::default();
        assert!(decode_expansion_calibration(&calibration_data, &mut exp));
        assert_eq!(exp.type_, WPAD_EXP_WIIBOARD as i32);

        let mut current = WpadData::default();
        current.exp = exp;
        let readings = [150u16, 260, 220, 350];
        let mut report = [0u8; 11];
        report[0] = 0x34;
        for (index, value) in readings.into_iter().enumerate() {
            let offset = 3 + index * 2;
            report[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        assert!(shape_wiimote_input_report(
            &WpadData::default(),
            &mut current,
            &Accel::default(),
            &report
        ));
        let board = unsafe { current.exp.u.wb };
        assert_eq!(
            [
                board.rtr as u16,
                board.rbr as u16,
                board.rtl as u16,
                board.rbl as u16
            ],
            readings
        );
        assert_eq!(
            [board.tr, board.br, board.tl, board.bl],
            [8.5, 25.5, 17.0, 34.0]
        );
    }

    #[test]
    fn accelerator_orientation_is_stable() {
        let accel_calib = Accel {
            cal_zero: Vec3w {
                x: 500,
                y: 500,
                z: 500,
            },
            cal_g: Vec3w {
                x: 100,
                y: 100,
                z: 100,
            },
            ..Accel::default()
        };
        let mut orient = Orient::default();
        let mut gforce = GForce::default();
        let accel = Vec3w {
            x: 500,
            y: 600,
            z: 500,
        };
        calculate_orientation(&accel_calib, &accel, &mut orient, false);
        calculate_gforce(&accel_calib, &accel, &mut gforce);
        assert!(orient.pitch.is_finite());
        assert!(gforce.y > 0.9);
    }

    #[test]
    fn wiimote_button_report_shapes_state() {
        let prev = WpadData::default();
        let mut current = WpadData::default();
        let accel_calib = Accel {
            cal_zero: Vec3w { x: 0, y: 0, z: 0 },
            cal_g: Vec3w { x: 1, y: 1, z: 1 },
            ..Accel::default()
        };
        current.ir.vres = [640, 480];
        current.ir.aspect = 0;
        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &accel_calib,
            &[0x30, 0xff, 0xff]
        ));
        assert_eq!(current.btns_h, 8095);
        assert_eq!(current.data_present, WPAD_DATA_BUTTONS);
    }

    #[test]
    fn classic_report_keeps_expansion_fields_raw_and_publishes_high_word_buttons() {
        let prev = WpadData {
            btns_h: 0x0060_0000,
            ..WpadData::default()
        };
        let mut prev = prev;
        prev.exp.type_ = WPAD_EXP_CLASSIC as i32;
        prev.exp.u.classic.btns = 0x0060;

        let mut current = WpadData::default();
        current.exp.type_ = WPAD_EXP_CLASSIC as i32;
        let accel_calib = Accel::default();
        let mut report = [0u8; 9];
        report[0] = 0x34;
        report[7..9].copy_from_slice(&(!0x0021u16).to_be_bytes());

        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &accel_calib,
            &report
        ));
        let classic = unsafe { current.exp.u.classic };
        assert_eq!(classic.btns_last, 0x0060);
        assert_eq!(classic.btns, 0x0021);
        assert_eq!(classic.btns_held, 0x0020);
        assert_eq!(classic.btns_released, 0x0040);
        assert_eq!(current.btns_h, 0x0021_0000);
        assert_eq!(current.btns_d, 0x0001_0000);
        assert_eq!(current.btns_u, 0x0040_0000);
    }

    #[test]
    fn wii_u_pro_report_decodes_sticks_buttons_and_status() {
        let prev = WpadData::default();
        let mut current = WpadData::default();
        current.exp.type_ = WPAD_EXP_CLASSIC as i32;
        current.exp.u.classic.type_ = 2;

        let mut report = [0xff; 14];
        report[0] = 0x34;
        report[1] = 0x08; // Wii Remote A is replaced by Pro stick buttons.
        report[3..11].copy_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]);
        report[11..13].copy_from_slice(&(!0x2010u16).to_be_bytes());
        report[13] = 0x50; // charging and wired; battery level 5.

        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &Accel::default(),
            &report
        ));
        let classic = unsafe { current.exp.u.classic };
        assert_eq!([classic.ljs.pos.x, classic.ljs.pos.y], [0x41, 0xc9]);
        assert_eq!([classic.rjs.pos.x, classic.rjs.pos.y], [0x85, 0x0d]);
        assert_eq!(classic.btns, 0x0003_2010);
        assert_eq!(classic.ls_raw, 0x1f);
        assert_eq!(classic.rs_raw, 0);
        assert_eq!(classic.l_shoulder, 1.0);
        assert_eq!(classic.charging, 1);
        assert_eq!(classic.wired, 1);
        assert_eq!(classic.battery, 5);
        assert_eq!(current.btns_h, 0x2010_0003);
        assert_eq!(current.btns_d, 0x2010_0003);

        let mut short_report = [0u8; 13];
        short_report[0] = 0x34;
        assert!(!shape_wiimote_input_report(
            &prev,
            &mut current,
            &Accel::default(),
            &short_report
        ));
    }

    #[test]
    fn guitar_report_keeps_raw_expansion_button_transitions() {
        let mut prev = WpadData::default();
        prev.btns_h = 0x0060_0000;
        prev.exp.type_ = WPAD_EXP_GUITARHERO3 as i32;
        prev.exp.u.gh3.btns = 0x0060;

        let mut current = WpadData::default();
        current.exp.type_ = WPAD_EXP_GUITARHERO3 as i32;
        let accel_calib = Accel::default();
        let mut report = [0u8; 9];
        report[0] = 0x34;
        report[3] = 0xc5;
        report[4] = 0xd6;
        report[5] = 0xee;
        report[6] = 0xee;
        report[7..9].copy_from_slice(&(!0x0021u16).to_be_bytes());

        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &accel_calib,
            &report
        ));
        let guitar = unsafe { current.exp.u.gh3 };
        assert_eq!([guitar.js.pos.x, guitar.js.pos.y], [5, 22]);
        assert_eq!([guitar.tb_raw, guitar.wb_raw], [14, 14]);
        assert_eq!(guitar.btns_last, 0x0060);
        assert_eq!(guitar.btns, 0x0021);
        assert_eq!(guitar.btns_held, 0x0020);
        assert_eq!(guitar.btns_released, 0x0040);
        assert_eq!(guitar.touch_bar, 0x1000);
        assert_eq!(guitar.whammy_bar, -1.0 / 11.0);
        assert_eq!(current.btns_h, 0x0021_0000);
        assert_eq!(current.btns_d, 0x0001_0000);
        assert_eq!(current.btns_u, 0x0040_0000);
    }

    #[test]
    fn wiimote_accel_and_ir_reports_shape_state() {
        let prev = WpadData::default();
        let accel_calib = Accel {
            cal_zero: Vec3w { x: 0, y: 0, z: 0 },
            cal_g: Vec3w { x: 1, y: 1, z: 1 },
            ..Accel::default()
        };

        let mut current = WpadData::default();
        current.ir.vres = [640, 480];
        current.ir.aspect = 0;
        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &accel_calib,
            &[0x31, 0xff, 0xff, 1, 2, 3]
        ));
        assert_eq!(current.data_present, WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL);

        let mut current = WpadData::default();
        current.ir.vres = [640, 480];
        current.ir.aspect = 0;
        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &accel_calib,
            &[
                0x33, 0xff, 0xff, 1, 2, 3, 0, 1, 2, 0, 3, 4, 0, 5, 6, 0, 7, 8
            ]
        ));
        assert_eq!(
            current.data_present,
            WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL | WPAD_DATA_IR
        );

        let mut current = WpadData::default();
        current.ir.vres = [640, 480];
        current.ir.aspect = 0;
        assert!(shape_wiimote_input_report(
            &prev,
            &mut current,
            &accel_calib,
            &[
                0x37, 0xff, 0xff, 1, 2, 3, 0, 1, 2, 0, 3, 4, 0, 5, 6, 0, 7, 8, 0, 1, 2, 0, 3, 4
            ]
        ));
        assert_eq!(
            current.data_present,
            WPAD_DATA_BUTTONS | WPAD_DATA_ACCEL | WPAD_DATA_IR | WPAD_DATA_EXPANSION
        );
    }
}
