#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::mem::size_of;

pub use ogc_conf_sys as sys;
pub use ogc_ipc as ipc;
pub use sys::{
    CONF_AREA_ASI, CONF_AREA_AUS, CONF_AREA_BRA, CONF_AREA_CHN, CONF_AREA_EUR, CONF_AREA_HKG,
    CONF_AREA_JPN, CONF_AREA_KOR, CONF_AREA_LTN, CONF_AREA_ROC, CONF_AREA_SAF, CONF_AREA_TWN,
    CONF_AREA_USA, CONF_ASPECT_4_3, CONF_ASPECT_16_9, CONF_BIGARRAY, CONF_BOOL, CONF_BYTE,
    CONF_EBADFILE, CONF_EBADVALUE, CONF_ENOENT, CONF_ENOMEM, CONF_ENOTIMPL, CONF_ENOTINIT,
    CONF_ERR_OK, CONF_ETOOBIG, CONF_LANG_DUTCH, CONF_LANG_ENGLISH, CONF_LANG_FRENCH,
    CONF_LANG_GERMAN, CONF_LANG_ITALIAN, CONF_LANG_JAPANESE, CONF_LANG_KOREAN,
    CONF_LANG_SIMP_CHINESE, CONF_LANG_SPANISH, CONF_LANG_TRAD_CHINESE, CONF_LED_BRIGHT,
    CONF_LED_DIM, CONF_LED_OFF, CONF_LONG, CONF_PAD_ACTIVE_AND_OTHER, CONF_PAD_MAX_ACTIVE,
    CONF_PAD_MAX_REGISTERED, CONF_PAD_TOTAL, CONF_REGION_CN, CONF_REGION_EU, CONF_REGION_JP,
    CONF_REGION_KR, CONF_REGION_US, CONF_SENSORBAR_BOTTOM, CONF_SENSORBAR_TOP, CONF_SHORT,
    CONF_SHUTDOWN_IDLE, CONF_SHUTDOWN_STANDBY, CONF_SMALLARRAY, CONF_SOUND_MONO, CONF_SOUND_STEREO,
    CONF_SOUND_SURROUND, CONF_VIDEO_MPAL, CONF_VIDEO_NTSC, CONF_VIDEO_PAL, ConfPadDevice,
    ConfPadGuestDevice, ConfPadGuests, ConfPads,
};

const CONF_FILE_SIZE: usize = 0x4000;
const CONF_TXT_FILE_SIZE: usize = 0x100;

const CONF_FILE: &[u8] = b"/shared2/sys/SYSCONF\0";
const CONF_TXT_FILE: &[u8] = b"/title/00000001/00000002/data/setting.txt\0";

struct ConfState {
    inited: bool,
    buffer: [u8; CONF_FILE_SIZE],
    txt_buffer: [u8; CONF_TXT_FILE_SIZE + 1],
    buffer_dirty: bool,
}

impl ConfState {
    const fn new() -> Self {
        Self {
            inited: false,
            buffer: [0; CONF_FILE_SIZE],
            txt_buffer: [0; CONF_TXT_FILE_SIZE + 1],
            buffer_dirty: false,
        }
    }
}

struct StateCell(UnsafeCell<ConfState>);

unsafe impl Sync for StateCell {}

static STATE: StateCell = StateCell(UnsafeCell::new(ConfState::new()));

fn state() -> *mut ConfState {
    STATE.0.get()
}

fn open_file(path: &[u8], mode: u32) -> Result<ipc::FileDescriptor, i32> {
    let path = CStr::from_bytes_with_nul(path).map_err(|_| CONF_EBADFILE)?;
    ipc::open_file(path, mode)
}

fn close_file(fd: ipc::FileDescriptor) {
    let _ = fd.close();
}

fn read_exact(fd: &ipc::FileDescriptor, buf: &mut [u8]) -> Result<(), i32> {
    read_exact_with(|output| fd.read(output), buf)
}

fn write_exact(fd: &ipc::FileDescriptor, buf: &[u8]) -> Result<(), i32> {
    write_exact_with(|input| fd.write(input), buf)
}

fn read_exact_with(
    read: impl FnOnce(&mut [u8]) -> Result<usize, i32>,
    buf: &mut [u8],
) -> Result<(), i32> {
    match read(buf) {
        Ok(count) if count == buf.len() => Ok(()),
        _ => Err(CONF_EBADFILE),
    }
}

fn write_exact_with(
    write: impl FnOnce(&[u8]) -> Result<usize, i32>,
    buf: &[u8],
) -> Result<(), i32> {
    match write(buf) {
        Ok(count) if count == buf.len() => Ok(()),
        _ => Err(CONF_EBADFILE),
    }
}

fn read_be_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let chunk = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_be_bytes([chunk[0], chunk[1]]))
}

fn conf_buffer_for_write(buf: &[u8; CONF_FILE_SIZE]) -> &[u8; CONF_FILE_SIZE] {
    // The first four bytes are the SCv0 format signature. C writes the
    // buffer unchanged; this format has no checksum field at that offset.
    buf
}

fn decrypt_text_buffer(buf: &mut [u8; CONF_TXT_FILE_SIZE + 1]) {
    let mut key = 0x73B5_DBFAu32;
    for byte in buf.iter_mut().take(CONF_TXT_FILE_SIZE) {
        *byte ^= (key & 0xff) as u8;
        key = key.rotate_left(1);
    }
}

fn is_inited() -> bool {
    unsafe { (*state()).inited }
}

fn find_entry<'a>(name: &str, buffer: &'a [u8; CONF_FILE_SIZE]) -> Option<&'a [u8]> {
    let count = read_be_u16(buffer, 4)? as usize;
    let offsets = &buffer[6..];

    for index in 0usize..count {
        let offset_index = index.checked_mul(2)?;
        let offset = read_be_u16(offsets, offset_index)? as usize;
        let entry = buffer.get(offset..)?;
        let header = *entry.first()?;
        let name_len = ((header & 0x0f) as usize) + 1;
        if name.len() == name_len && entry.get(1..1 + name_len) == Some(name.as_bytes()) {
            return Some(entry);
        }
    }

    None
}

fn get_txt(name: &str, buf: &mut [u8]) -> Result<usize, i32> {
    if !is_inited() {
        return Err(CONF_ENOTINIT);
    }

    let state = unsafe { &*state() };
    get_txt_from_data(name, &state.txt_buffer[..CONF_TXT_FILE_SIZE], buf)
}

mod text_settings;
use text_settings::get_txt_from_data;

fn get_length_inner(name: &str) -> Result<usize, i32> {
    if !is_inited() {
        return Err(CONF_ENOTINIT);
    }

    let state = unsafe { &*state() };
    let Some(entry) = find_entry(name, &state.buffer) else {
        return Err(CONF_ENOENT);
    };

    let ty = entry[0] >> 5;
    let name_len = name.len();
    match ty {
        CONF_BIGARRAY => {
            let len = read_be_u16(entry, name_len + 1).ok_or(CONF_EBADFILE)? as usize;
            Ok(len + 1)
        }
        CONF_SMALLARRAY => Ok(entry[name_len + 1] as usize + 1),
        CONF_BYTE | CONF_BOOL => Ok(1),
        CONF_SHORT => Ok(2),
        CONF_LONG => Ok(4),
        _ => Err(CONF_ENOTIMPL),
    }
}

fn get_inner(name: &str, buffer: &mut [u8]) -> Result<usize, i32> {
    if !is_inited() {
        return Err(CONF_ENOTINIT);
    }

    let state = unsafe { &*state() };
    let Some(entry) = find_entry(name, &state.buffer) else {
        return Err(CONF_ENOENT);
    };

    let len = get_length_inner(name)?;
    if len > buffer.len() {
        return Err(CONF_ETOOBIG);
    }

    let ty = entry[0] >> 5;
    let name_len = name.len();
    match ty {
        CONF_BIGARRAY => {
            buffer[..len].copy_from_slice(&entry[name_len + 3..name_len + 3 + len]);
        }
        CONF_SMALLARRAY => {
            buffer[..len].copy_from_slice(&entry[name_len + 2..name_len + 2 + len]);
        }
        CONF_BYTE | CONF_SHORT | CONF_LONG | CONF_BOOL => {
            buffer.fill(0);
            let src = match ty {
                CONF_BYTE | CONF_BOOL => &entry[name_len + 1..name_len + 2],
                CONF_SHORT => &entry[name_len + 1..name_len + 3],
                CONF_LONG => &entry[name_len + 1..name_len + 5],
                _ => unreachable!(),
            };
            buffer[..len].copy_from_slice(src);
        }
        _ => return Err(CONF_ENOTIMPL),
    }

    Ok(len)
}

fn set_inner(name: &str, buffer: &[u8]) -> Result<usize, i32> {
    if !is_inited() {
        return Err(CONF_ENOTINIT);
    }

    let state = unsafe { &mut *state() };
    let (offset, ty) = {
        let Some(entry) = find_entry(name, &state.buffer) else {
            return Err(CONF_ENOENT);
        };
        (
            entry.as_ptr() as usize - state.buffer.as_ptr() as usize,
            entry[0] >> 5,
        )
    };

    let len = get_length_inner(name)?;
    if len != buffer.len() {
        return Err(CONF_EBADVALUE);
    }

    let name_len = name.len();

    match ty {
        CONF_BIGARRAY => {
            state.buffer[offset + name_len + 3..offset + name_len + 3 + len]
                .copy_from_slice(buffer);
        }
        CONF_SMALLARRAY => {
            state.buffer[offset + name_len + 2..offset + name_len + 2 + len]
                .copy_from_slice(buffer);
        }
        CONF_BYTE | CONF_SHORT | CONF_LONG | CONF_BOOL => {
            state.buffer[offset + name_len + 1..offset + name_len + 1 + len]
                .copy_from_slice(buffer);
        }
        _ => return Err(CONF_ENOTIMPL),
    }

    state.buffer_dirty = true;
    Ok(len)
}

fn region_from_text(text: &[u8]) -> i32 {
    match text {
        b"JP" => CONF_REGION_JP as i32,
        b"US" => CONF_REGION_US as i32,
        b"EU" => CONF_REGION_EU as i32,
        b"KR" => CONF_REGION_KR as i32,
        b"CN" => CONF_REGION_CN as i32,
        _ => CONF_EBADVALUE,
    }
}

fn area_from_text(text: &[u8]) -> i32 {
    match text {
        b"JPN" => CONF_AREA_JPN as i32,
        b"USA" => CONF_AREA_USA as i32,
        b"EUR" => CONF_AREA_EUR as i32,
        b"AUS" => CONF_AREA_AUS as i32,
        b"BRA" => CONF_AREA_BRA as i32,
        b"TWN" => CONF_AREA_TWN as i32,
        b"ROC" => CONF_AREA_ROC as i32,
        b"KOR" => CONF_AREA_KOR as i32,
        b"HKG" => CONF_AREA_HKG as i32,
        b"ASI" => CONF_AREA_ASI as i32,
        b"LTN" => CONF_AREA_LTN as i32,
        b"SAF" => CONF_AREA_SAF as i32,
        b"CHN" => CONF_AREA_CHN as i32,
        _ => CONF_EBADVALUE,
    }
}

fn video_from_text(text: &[u8]) -> i32 {
    match text {
        b"NTSC" => CONF_VIDEO_NTSC as i32,
        b"PAL" => CONF_VIDEO_PAL as i32,
        b"MPAL" => CONF_VIDEO_MPAL as i32,
        _ => CONF_EBADVALUE,
    }
}

pub fn init() -> i32 {
    if is_inited() {
        return 0;
    }

    let state = unsafe { &mut *state() };

    let conf_fd = match open_file(CONF_FILE, ipc::IPC_OPEN_READ) {
        Ok(fd) => fd,
        Err(err) => return err,
    };
    state.buffer.fill(0);
    state.txt_buffer.fill(0);

    if let Err(err) = read_exact(&conf_fd, &mut state.buffer) {
        close_file(conf_fd);
        return err;
    }
    close_file(conf_fd);

    let txt_fd = match open_file(CONF_TXT_FILE, ipc::IPC_OPEN_READ) {
        Ok(fd) => fd,
        Err(err) => return err,
    };
    if let Err(err) = read_exact(&txt_fd, &mut state.txt_buffer[..CONF_TXT_FILE_SIZE]) {
        close_file(txt_fd);
        return err;
    }
    close_file(txt_fd);

    if &state.buffer[..4] != b"SCv0" {
        return CONF_EBADFILE;
    }

    decrypt_text_buffer(&mut state.txt_buffer);
    state.inited = true;
    0
}

pub fn save_changes() -> i32 {
    if !is_inited() {
        return CONF_ENOTINIT;
    }

    let state = unsafe { &mut *state() };
    if !state.buffer_dirty {
        return 0;
    }

    let fd = match open_file(CONF_FILE, ipc::IPC_OPEN_RW) {
        Ok(fd) => fd,
        Err(err) => return err,
    };

    let result = match write_exact(&fd, conf_buffer_for_write(&state.buffer)) {
        Ok(()) => {
            state.buffer_dirty = false;
            CONF_ERR_OK
        }
        Err(err) => err,
    };
    close_file(fd);
    result
}

pub fn get_length(name: &str) -> i32 {
    get_length_inner(name)
        .map(|len| len as i32)
        .unwrap_or_else(|err| err)
}

pub fn get_type(name: &str) -> i32 {
    if !is_inited() {
        return CONF_ENOTINIT;
    }

    let state = unsafe { &*state() };
    let Some(entry) = find_entry(name, &state.buffer) else {
        return CONF_ENOENT;
    };

    (entry[0] >> 5) as i32
}

pub fn get(name: &str, buffer: &mut [u8]) -> i32 {
    get_inner(name, buffer)
        .map(|len| len as i32)
        .unwrap_or_else(|err| err)
}

pub fn set(name: &str, buffer: &[u8]) -> i32 {
    set_inner(name, buffer)
        .map(|len| len as i32)
        .unwrap_or_else(|err| err)
}

pub fn get_shutdown_mode() -> i32 {
    let mut buf = [0u8; 2];
    let res = get("IPL.IDL", &mut buf);
    if res < 0 {
        return res;
    }
    if res != 2 {
        return CONF_EBADVALUE;
    }
    i32::from(buf[0])
}

pub fn get_idle_led_mode() -> i32 {
    let mut buf = [0u8; 2];
    let res = get("IPL.IDL", &mut buf);
    if res < 0 {
        return res;
    }
    if res != 2 {
        return CONF_EBADVALUE;
    }
    i32::from(buf[1])
}

pub fn get_progressive_scan() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.PGS", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_eu_rgb60() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.E60", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_ir_sensitivity() -> i32 {
    let mut val = [0u8; 4];
    let res = get("BT.SENS", &mut val);
    if res < 0 {
        return res;
    }
    if res != 4 {
        return CONF_EBADVALUE;
    }
    i32::from_be_bytes(val)
}

pub fn get_sensor_bar_position() -> i32 {
    let mut val = [0u8; 1];
    let res = get("BT.BAR", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_pad_speaker_volume() -> i32 {
    let mut val = [0u8; 1];
    let res = get("BT.SPKV", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_pad_motor_mode() -> i32 {
    let mut val = [0u8; 1];
    let res = get("BT.MOT", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_sound_mode() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.SND", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_language() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.LNG", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_counter_bias(bias: &mut u32) -> i32 {
    let res = get("IPL.CB", unsafe {
        core::slice::from_raw_parts_mut((bias as *mut u32).cast(), 4)
    });
    if res < 0 {
        return res;
    }
    if res != 4 {
        return CONF_EBADVALUE;
    }
    CONF_ERR_OK
}

pub fn get_screen_saver_mode() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.SSV", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_display_offset_h(offset: &mut i8) -> i32 {
    let res = get("IPL.DH", unsafe {
        core::slice::from_raw_parts_mut((offset as *mut i8).cast(), 1)
    });
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    0
}

pub fn get_pad_devices(pads: &mut ConfPads) -> i32 {
    let res = get("BT.DINF", unsafe {
        core::slice::from_raw_parts_mut((pads as *mut ConfPads).cast(), size_of::<ConfPads>())
    });
    if res < 0 {
        return res;
    }
    if res < size_of::<ConfPads>() as i32 {
        return CONF_EBADVALUE;
    }
    0
}

pub fn set_pad_devices(pads: &ConfPads) -> i32 {
    if pads.num_registered > CONF_PAD_MAX_REGISTERED as u8 {
        return CONF_EBADVALUE;
    }
    set("BT.DINF", unsafe {
        core::slice::from_raw_parts((pads as *const ConfPads).cast(), size_of::<ConfPads>())
    })
}

pub fn get_pad_guest_devices(pads: &mut ConfPadGuests) -> i32 {
    let res = get("BT.CDIF", unsafe {
        core::slice::from_raw_parts_mut(
            (pads as *mut ConfPadGuests).cast(),
            size_of::<ConfPadGuests>(),
        )
    });
    if res < 0 {
        return res;
    }
    if res < size_of::<ConfPadGuests>() as i32 {
        return CONF_EBADVALUE;
    }
    0
}

pub fn set_pad_guest_devices(pads: Option<&ConfPadGuests>) -> i32 {
    let empty = ConfPadGuests::default();
    let pads = pads.unwrap_or(&empty);
    if pads.num_guests > CONF_PAD_MAX_ACTIVE as u8 {
        return CONF_EBADVALUE;
    }
    set("BT.CDIF", unsafe {
        core::slice::from_raw_parts(
            (pads as *const ConfPadGuests).cast(),
            size_of::<ConfPadGuests>(),
        )
    })
}

pub fn get_nick_name(nickname: &mut [u8; 11]) -> i32 {
    let mut buf = [0u8; 0x16];
    let res = get("IPL.NIK", &mut buf);
    if res < 0 {
        return res;
    }
    if res != 0x16 || buf[..2] == [0, 0] {
        return CONF_EBADVALUE;
    }

    for (index, chunk) in buf.as_chunks::<2>().0.iter().take(10).enumerate() {
        nickname[index] = u16::from_be_bytes([chunk[0], chunk[1]]) as u8;
    }
    nickname[10] = 0;
    res
}

pub fn get_aspect_ratio() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.AR", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_eula() -> i32 {
    let mut val = [0u8; 1];
    let res = get("IPL.EULA", &mut val);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    i32::from(val[0])
}

pub fn get_parental_password(password: &mut [i8; 5]) -> i32 {
    let mut buf = [0u8; 0x4a];
    let res = get("IPL.PC", &mut buf);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    for (dst, src) in password.iter_mut().zip(buf[3..7].iter()) {
        *dst = *src as i8;
    }
    password[4] = 0;
    res
}

pub fn get_parental_answer(answer: &mut [i8; 33]) -> i32 {
    let mut buf = [0u8; 0x4a];
    let res = get("IPL.PC", &mut buf);
    if res < 0 {
        return res;
    }
    if res != 1 {
        return CONF_EBADVALUE;
    }
    for (dst, src) in answer.iter_mut().take(32).zip(buf[8..40].iter()) {
        *dst = *src as i8;
    }
    answer[32] = 0;
    res
}

pub fn get_wii_connect24() -> i32 {
    let mut val = [0u8; 4];
    let res = get("NET.WCFG", &mut val);
    if res < 0 {
        return res;
    }
    if res != 4 {
        return CONF_EBADVALUE;
    }
    i32::from_be_bytes(val)
}

/// Read MODEL from the initialized console setting.txt cache. The returned
/// length excludes the NUL terminator. No firmware or configuration is changed.
pub fn model_name(output: &mut [u8]) -> Result<usize, i32> {
    get_txt("MODEL", output)
}

pub fn get_region() -> i32 {
    let mut buf = [0u8; 3];
    let len = match get_txt("GAME", &mut buf) {
        Ok(len) => len,
        Err(err) => return err,
    };
    region_from_text(&buf[..len])
}

pub fn get_area() -> i32 {
    let mut buf = [0u8; 4];
    let len = match get_txt("AREA", &mut buf) {
        Ok(len) => len,
        Err(err) => return err,
    };
    area_from_text(&buf[..len])
}

pub fn get_video() -> i32 {
    let mut buf = [0u8; 5];
    let len = match get_txt("VIDEO", &mut buf) {
        Ok(len) => len,
        Err(err) => return err,
    };
    video_from_text(&buf[..len])
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn model_field_uses_exact_setting_bytes_and_checks_output_extent() {
        let mut output = [0u8; 32];
        assert_eq!(
            get_txt_from_data("MODEL", b"AREA=USA\r\nMODEL=RVL-101(USA)\r\n", &mut output),
            Ok(12)
        );
        assert_eq!(&output[..13], b"RVL-101(USA)\0");
        assert_eq!(
            get_txt_from_data("MODEL", b"MODEL=RVL-101(USA)\r\n", &mut [0; 12]),
            Err(CONF_ETOOBIG)
        );
        assert_eq!(
            get_txt_from_data("MODEL", b"AREA=USA\r\n", &mut output),
            Err(CONF_ENOENT)
        );
    }

    #[test]
    fn save_buffer_preserves_sc_v0_signature() {
        let mut buffer = [0u8; CONF_FILE_SIZE];
        buffer[..4].copy_from_slice(b"SCv0");
        buffer[4..8].copy_from_slice(&0x1234_5678u32.to_be_bytes());

        let written = conf_buffer_for_write(&buffer);

        assert_eq!(&written[..4], b"SCv0");
        assert_eq!(&written[4..8], &0x1234_5678u32.to_be_bytes());
    }

    #[test]
    fn setting_values_require_an_exact_match() {
        assert_eq!(region_from_text(b"JP"), CONF_REGION_JP as i32);
        assert_eq!(region_from_text(b"JPN"), CONF_EBADVALUE);
        assert_eq!(area_from_text(b"USA"), CONF_AREA_USA as i32);
        assert_eq!(area_from_text(b"USAX"), CONF_EBADVALUE);
        assert_eq!(video_from_text(b"PAL"), CONF_VIDEO_PAL as i32);
        assert_eq!(video_from_text(b"PAL60"), CONF_EBADVALUE);
    }

    #[test]
    fn setting_text_prefers_cr_over_an_earlier_lf() {
        let data = b"GAME=JP\nOTHER=x\r";
        let mut output = [0u8; 11];

        assert_eq!(get_txt_from_data("GAME", data, &mut output), Ok(10));
        assert_eq!(&output[..10], b"JP\nOTHER=x");
        assert_eq!(output[10], 0);
    }

    #[test]
    fn negative_conf_read_result_maps_to_bad_file() {
        let mut buffer = [0u8; 8];
        assert_eq!(
            read_exact_with(|_| Err(-6), &mut buffer),
            Err(CONF_EBADFILE)
        );
    }

    #[test]
    fn negative_conf_write_result_maps_to_bad_file() {
        let buffer = [0u8; 8];
        assert_eq!(write_exact_with(|_| Err(-6), &buffer), Err(CONF_EBADFILE));
    }

    #[test]
    fn layout_matches_c() {
        assert_eq!(core::mem::size_of::<ConfPadDevice>(), 70);
        assert_eq!(core::mem::align_of::<ConfPadDevice>(), 1);
        assert_eq!(core::mem::size_of::<ConfPads>(), 1121);
        assert_eq!(core::mem::align_of::<ConfPads>(), 1);
        assert_eq!(core::mem::size_of::<ConfPadGuestDevice>(), 86);
        assert_eq!(core::mem::align_of::<ConfPadGuestDevice>(), 1);
        assert_eq!(core::mem::size_of::<ConfPadGuests>(), 517);
        assert_eq!(core::mem::align_of::<ConfPadGuests>(), 1);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn() -> i32;
        let _ = save_changes as fn() -> i32;
        let _ = get_length as fn(&str) -> i32;
        let _ = get_type as fn(&str) -> i32;
        let _ = get as fn(&str, &mut [u8]) -> i32;
        let _ = set as fn(&str, &[u8]) -> i32;
        let _ = get_shutdown_mode as fn() -> i32;
        let _ = get_idle_led_mode as fn() -> i32;
        let _ = get_progressive_scan as fn() -> i32;
        let _ = get_eu_rgb60 as fn() -> i32;
        let _ = get_ir_sensitivity as fn() -> i32;
        let _ = get_sensor_bar_position as fn() -> i32;
        let _ = get_pad_speaker_volume as fn() -> i32;
        let _ = get_pad_motor_mode as fn() -> i32;
        let _ = get_sound_mode as fn() -> i32;
        let _ = get_language as fn() -> i32;
        let _ = get_counter_bias as fn(&mut u32) -> i32;
        let _ = get_screen_saver_mode as fn() -> i32;
        let _ = get_display_offset_h as fn(&mut i8) -> i32;
        let _ = get_pad_devices as fn(&mut ConfPads) -> i32;
        let _ = set_pad_devices as fn(&ConfPads) -> i32;
        let _ = get_pad_guest_devices as fn(&mut ConfPadGuests) -> i32;
        let _ = set_pad_guest_devices as fn(Option<&ConfPadGuests>) -> i32;
        let _ = get_nick_name as fn(&mut [u8; 11]) -> i32;
        let _ = get_aspect_ratio as fn() -> i32;
        let _ = get_eula as fn() -> i32;
        let _ = get_parental_password as fn(&mut [i8; 5]) -> i32;
        let _ = get_parental_answer as fn(&mut [i8; 33]) -> i32;
        let _ = get_wii_connect24 as fn() -> i32;
        let _ = get_region as fn() -> i32;
        let _ = get_area as fn() -> i32;
        let _ = get_video as fn() -> i32;
    }
}
