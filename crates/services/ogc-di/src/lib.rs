#![no_std]

//! Rust-owned Wii disc-interface implementation.
//!
//! This crate owns DI command submission and disc/status wrappers. It exports
//! the legacy `DI_*` C ABI names for source and linker compatibility. Async
//! submission and other transport behavior still need separate parity review.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

use ogc_filesystem::DiscInterface;
use ogc_ipc::{FileDescriptor, IPC_OPEN_RW, IoctlBuffers, IoctlvEntry, Ipccallback, open_file};
use ogc_lwp_sys as lwp;

#[path = "retry.rs"]
mod retry;
mod shutdown;

#[cfg(target_arch = "powerpc")]
use ogc_cache::dc_invalidate_range;

pub const DVD_IDENTIFY: u32 = 0x12;
pub const DVD_READ_DISCID: u32 = 0x70;
pub const DVD_LOW_READ: u32 = 0x71;
pub const DVD_WAITFORCOVERCLOSE: u32 = 0x79;
pub const DVD_READ_PHYSICAL: u32 = 0x80;
pub const DVD_READ_COPYRIGHT: u32 = 0x81;
pub const DVD_READ_DISCKEY: u32 = 0x82;
pub const DVD_GETCOVER: u32 = 0x88;
pub const DVD_RESET: u32 = 0x8A;
pub const DVD_OPEN_PARTITION: u32 = 0x8B;
pub const DVD_CLOSE_PARTITION: u32 = 0x8C;
pub const DVD_READ_UNENCRYPTED: u32 = 0x8D;
pub const DVD_REPORTKEY: u32 = 0xA4;
pub const DVD_READ: u32 = 0xD0;
pub const DVD_READ_CONFIG: u32 = 0xD1;
pub const DVD_READ_BCA: u32 = 0xDA;
pub const DVD_GET_ERROR: u32 = 0xE0;
pub const DVD_AUDIO_STREAM: u32 = 0xE1;
pub const DVD_SET_MOTOR: u32 = 0xE3;

pub const DVD_READY: u32 = 0x1;
pub const DVD_INIT: u32 = 0x2;
pub const DVD_UNKNOWN: u32 = 0x4;
pub const DVD_NO_DISC: u32 = 0x8;
pub const DVD_IOS_ERROR: u32 = 0x10;
pub const DVD_D0: u32 = 0x20;
pub const DVD_A8: u32 = 0x40;

pub const DVD_COVER_DISC_INSERTED: u32 = 0x02;
pub const LIBDI_MAX_RETRIES: u32 = retry::LIBDI_MAX_RETRIES;
pub const DEVICE_TYPE_WII_DVD: u32 = 0x5744_5644;
pub const FEATURE_MEDIUM_CANREAD: u32 = 0x0000_0001;
pub const FEATURE_WII_DVD: u32 = 0x0000_0400;

const DI_PATH: &CStr = c"/dev/di";
const BLOCK_SIZE: usize = 0x800;
const CACHEBLOCKS: usize = 26;
const CACHE_FREE: u32 = u32::MAX;
const EINVAL: i32 = -22;
const ENXIO: i32 = -6;
const EFAULT: i32 = -14;
const EIO: i32 = -5;
const MOUNT_TIMEOUT_TICKS: u64 = 15 * lwp::TICKS_PER_SECOND;
const MOUNT_POLL_TICKS: u64 = lwp::TICKS_PER_SECOND / 2_000;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct DiDriveId {
    pub rev: u16,
    pub dev_code: u16,
    pub rel_date: u32,
}

pub type DiCallback = Option<unsafe extern "C" fn(status: u32, error: u32) -> i32>;
pub type ReadFunc = Option<unsafe extern "C" fn(buf: *mut c_void, len: u32, lba: u32) -> i32>;
pub type ReadFuncAsync =
    Option<unsafe extern "C" fn(buf: *mut c_void, len: u32, lba: u32, cb: Ipccallback) -> i32>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ReadMode {
    Unknown,
    D0,
    A8,
}

struct DiState {
    fd: Option<FileDescriptor>,
    load_dvdx: bool,
    use_cache: bool,
    have_ahbprot: bool,
    status: u32,
    init_callback: DiCallback,
    mount_callback: DiCallback,
    read_mode: ReadMode,
    cache_block: u32,
    mount_stage: u8,
    mount_retries: u8,
    mount_inserted: bool,
}

impl DiState {
    const fn new() -> Self {
        Self {
            fd: None,
            load_dvdx: false,
            use_cache: true,
            have_ahbprot: false,
            status: DVD_INIT | DVD_NO_DISC,
            init_callback: None,
            mount_callback: None,
            read_mode: ReadMode::Unknown,
            cache_block: CACHE_FREE,
            mount_stage: 0,
            mount_retries: 0,
            mount_inserted: false,
        }
    }
}

static DI_MUTEX_READY: AtomicBool = AtomicBool::new(false);
static DI_MUTEX_STARTING: AtomicBool = AtomicBool::new(false);
static mut DI_MUTEX: lwp::Mutex = lwp::LWP_MUTEX_NULL;

fn di_mutex() -> lwp::Mutex {
    if DI_MUTEX_READY.load(Ordering::Acquire) {
        return unsafe { DI_MUTEX };
    }
    if DI_MUTEX_STARTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let mut handle = lwp::LWP_MUTEX_NULL;
        // SAFETY: `handle` is a valid unique output reference.
        let result = unsafe { lwp::mutex_init(&mut handle, 1) };
        if result != lwp::LWP_SUCCESSFUL {
            DI_MUTEX_STARTING.store(false, Ordering::Release);
            panic!("DI scheduler mutex allocation failed");
        }
        unsafe { DI_MUTEX = handle };
        DI_MUTEX_READY.store(true, Ordering::Release);
        DI_MUTEX_STARTING.store(false, Ordering::Release);
        handle
    } else {
        while !DI_MUTEX_READY.load(Ordering::Acquire) {
            lwp::yield_thread();
        }
        unsafe { DI_MUTEX }
    }
}

struct SpinLock<T> {
    value: core::cell::UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    const fn new(value: T) -> Self {
        Self {
            value: core::cell::UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> SpinGuard<'_, T> {
        let mutex = di_mutex();
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            panic!("DI scheduler mutex lock failed");
        }
        SpinGuard { lock: self, mutex }
    }
}

struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
    mutex: lwp::Mutex,
}

impl<T> core::ops::Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> core::ops::DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        let _ = lwp::mutex_unlock(self.mutex);
    }
}

#[repr(align(32))]
struct CommandWords([u32; 8]);

impl CommandWords {
    const fn zeroed() -> Self {
        Self([0; 8])
    }
}

#[repr(align(32))]
struct Output32([u8; 0x20]);

#[repr(C, align(32))]
struct MountIoBuffers {
    command: CommandWords,
    output: Output32,
}

#[repr(align(32))]
struct PartitionCerts([u8; 0x49e4]);

#[repr(align(32))]
struct CacheStorage([u8; BLOCK_SIZE * CACHEBLOCKS]);

static STATE: SpinLock<DiState> = SpinLock::new(DiState::new());
static mut CACHE_STORAGE: CacheStorage = CacheStorage([0; BLOCK_SIZE * CACHEBLOCKS]);
static mut PARTITION_CERTS: PartitionCerts = PartitionCerts([0; 0x49e4]);
static mut MOUNT_IO: MountIoBuffers = MountIoBuffers {
    command: CommandWords::zeroed(),
    output: Output32([0; 0x20]),
};
static mut ASYNC_READ_COMMAND: CommandWords = CommandWords::zeroed();

fn ioctl_words(
    fd: &FileDescriptor,
    ioctl: u32,
    words: &mut CommandWords,
    out: Option<&mut [u8]>,
) -> i32 {
    let in_bytes =
        unsafe { core::slice::from_raw_parts_mut(words.0.as_mut_ptr().cast::<u8>(), 0x20) };
    let buffers = IoctlBuffers::from_slices(Some(in_bytes), out);
    normalize_di_result(fd.ioctl(ioctl as i32, buffers))
}

fn normalize_di_result(result: i32) -> i32 {
    match result {
        1 => 0,
        2 => EIO,
        value if value < 0 => value,
        _ => EIO,
    }
}

fn read_ahbprot() -> bool {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::ptr::read_volatile(0xcd80_0064 as *const u32) == 0xFFFF_FFFF
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        false
    }
}

fn with_state<R>(f: impl FnOnce(&mut DiState) -> R) -> R {
    let mut guard = STATE.lock();
    f(&mut guard)
}

fn open_di(state: &mut DiState) -> Result<(), i32> {
    if state.fd.is_none() {
        state.fd = Some(open_file(DI_PATH, IPC_OPEN_RW)?);
    }
    Ok(())
}

fn set_state(state: &mut DiState, value: u32) {
    state.status = value;
}

fn current_fd(state: &DiState) -> Result<&FileDescriptor, i32> {
    state.fd.as_ref().ok_or(ENXIO)
}

// BUG FIX: preserve C's synchronous descriptor and mount-state errors before
// validating arguments or adding a request to the deferred callback queue.
fn async_read_precheck(state: &DiState, buf: *mut c_void, ipc_cb: Ipccallback) -> Result<(), i32> {
    // Match DI_ReadDVDAsync's error ordering: an unopened DI descriptor wins,
    // then the absent async-read hook before mount, before argument checks.
    if state.fd.is_none() {
        return Err(ENXIO);
    }
    if state.status & DVD_READY == 0 {
        return Err(-1);
    }
    if buf.is_null() || ipc_cb.is_none() {
        return Err(EINVAL);
    }
    if (buf as usize & 0x1f) != 0 {
        return Err(EFAULT);
    }
    Ok(())
}

fn build_dvd_command(words: &mut CommandWords, cmd: u32, len: u32, lba: u32, use_d0: bool) {
    words.0.fill(0);
    words.0[0] = cmd << 24;
    if use_d0 {
        words.0[3] = len;
        words.0[4] = lba;
    } else {
        words.0[1] = len << 11;
        words.0[2] = lba << 9;
    }
}

fn read_dvd_ios(state: &mut DiState, buf: *mut c_void, len: u32, lba: u32, use_d0: bool) -> i32 {
    let fd = match current_fd(state) {
        Ok(fd) => fd,
        Err(e) => return e,
    };

    if buf.is_null() {
        return EINVAL;
    }
    if (buf as usize) & 0x1f != 0 {
        return EFAULT;
    }

    let Some(bytes_u32) = len.checked_mul(BLOCK_SIZE as u32) else {
        return EINVAL;
    };
    if !use_d0 && lba.checked_mul(1 << 9).is_none() {
        return EINVAL;
    }
    let bytes = bytes_u32 as usize;
    let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, bytes) };

    let mut words = CommandWords::zeroed();
    build_dvd_command(
        &mut words,
        if use_d0 {
            DVD_READ
        } else {
            DVD_READ_UNENCRYPTED
        },
        len,
        lba,
        use_d0,
    );
    let ioctl = if use_d0 {
        DVD_READ
    } else {
        DVD_READ_UNENCRYPTED
    };
    retry::retry_di_read(
        || ioctl_words(fd, ioctl, &mut words, Some(&mut *out)),
        LIBDI_MAX_RETRIES,
    )
}

fn read_dvd_hw(buf: *mut c_void, len: u32, lba: u32, use_d0: bool) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let Some(bytes) = len.checked_mul(BLOCK_SIZE as u32) else {
            return EINVAL;
        };
        if !use_d0 && lba.checked_mul(1 << 9).is_none() {
            return EINVAL;
        }

        let reg = 0xcd80_6000 as *mut u32;
        let read_cmd = if use_d0 { 0xD000_0000 } else { 0xA800_0000 };
        if ((buf as usize) & 0xC000_0000) == 0x8000_0000 {
            core::ptr::write_volatile(reg.add(0), 0x2E);
        }
        core::ptr::write_volatile(reg.add(1), 0);
        core::ptr::write_volatile(reg.add(2), read_cmd);
        core::ptr::write_volatile(reg.add(3), if use_d0 { lba } else { lba << 9 });
        core::ptr::write_volatile(reg.add(4), if use_d0 { len } else { len << 11 });
        core::ptr::write_volatile(reg.add(5), buf as u32);
        core::ptr::write_volatile(reg.add(6), bytes);
        core::ptr::write_volatile(reg.add(7), 3);
        dc_invalidate_range(buf.cast_const(), bytes);
        while (core::ptr::read_volatile(reg.add(7)) & 1) != 0 {}
        if (core::ptr::read_volatile(reg.add(0)) & 0x4) != 0 {
            1
        } else {
            0
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = (buf, len, lba, use_d0);
        ENXIO
    }
}

fn read_dvd_selected(state: &mut DiState, buf: *mut c_void, len: u32, lba: u32) -> i32 {
    match state.read_mode {
        ReadMode::D0 => {
            if state.have_ahbprot {
                read_dvd_hw(buf, len, lba, true)
            } else {
                read_dvd_ios(state, buf, len, lba, true)
            }
        }
        ReadMode::A8 => {
            if state.have_ahbprot {
                read_dvd_hw(buf, len, lba, false)
            } else {
                read_dvd_ios(state, buf, len, lba, false)
            }
        }
        ReadMode::Unknown => {
            let first = if state.have_ahbprot {
                read_dvd_hw(buf, len, lba, true)
            } else {
                read_dvd_ios(state, buf, len, lba, true)
            };
            if first == 0 {
                state.read_mode = ReadMode::D0;
                set_state(state, state.status | DVD_D0);
                return 0;
            }
            let second = if state.have_ahbprot {
                read_dvd_hw(buf, len, lba, false)
            } else {
                read_dvd_ios(state, buf, len, lba, false)
            };
            if second == 0 {
                state.read_mode = ReadMode::A8;
                set_state(state, state.status | DVD_A8);
            }
            second
        }
    }
}

fn read_from_cache_or_drive(state: &mut DiState, buf: *mut c_void, len: u32, block: u32) -> i32 {
    if !state.use_cache {
        return read_dvd_selected(state, buf, len, block);
    }

    let cache_len = CACHEBLOCKS as u32;
    if len > cache_len {
        return read_dvd_selected(state, buf, len, block);
    }

    let cache_ptr = unsafe { core::ptr::addr_of_mut!(CACHE_STORAGE.0) as *mut u8 };
    let cached = block >= state.cache_block
        && block
            .checked_add(len)
            .zip(state.cache_block.checked_add(cache_len))
            .is_some_and(|(request_end, cache_end)| request_end <= cache_end);
    if cached {
        let offset = ((block - state.cache_block) as usize) * BLOCK_SIZE;
        let bytes = (len as usize) * BLOCK_SIZE;
        unsafe {
            core::ptr::copy_nonoverlapping(cache_ptr.add(offset), buf as *mut u8, bytes);
        }
        return 0;
    }

    let rc = read_dvd_selected(state, cache_ptr.cast(), CACHEBLOCKS as u32, block);
    if rc != 0 {
        state.cache_block = CACHE_FREE;
        return rc;
    }

    state.cache_block = block;
    unsafe {
        core::ptr::copy_nonoverlapping(cache_ptr, buf as *mut u8, (len as usize) * BLOCK_SIZE);
    }
    0
}

const MOUNT_GET_COVER: u8 = 0;
const MOUNT_WAIT_FOR_COVER: u8 = 1;
const MOUNT_RESET: u8 = 2;
const MOUNT_IDENTIFY: u8 = 3;
const MOUNT_READ_DISC_ID: u8 = 4;

fn mount_next_stage(stage: u8, inserted: bool) -> u8 {
    match stage {
        MOUNT_GET_COVER if inserted => MOUNT_RESET,
        MOUNT_GET_COVER => MOUNT_WAIT_FOR_COVER,
        MOUNT_WAIT_FOR_COVER if inserted => MOUNT_RESET,
        MOUNT_WAIT_FOR_COVER => MOUNT_WAIT_FOR_COVER,
        MOUNT_RESET => MOUNT_IDENTIFY,
        MOUNT_IDENTIFY => MOUNT_READ_DISC_ID,
        MOUNT_READ_DISC_ID => 5,
        _ => 5,
    }
}

fn mount_cover_precheck(has_fd: bool, cover_result: Result<u32, i32>) -> MountPrecheck {
    if !has_fd {
        MountPrecheck::NoDevice
    } else {
        match cover_result {
            Ok(status) if status & DVD_COVER_DISC_INSERTED != 0 => MountPrecheck::Start,
            Ok(_) => MountPrecheck::NoDisc,
            Err(error) => MountPrecheck::CoverError(error),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum MountPrecheck {
    NoDevice,
    NoDisc,
    CoverError(i32),
    Start,
}

fn wait_for_mount(
    timeout_ticks: u64,
    poll_ticks: u64,
    mut status: impl FnMut() -> i32,
    mut now_ticks: impl FnMut() -> u64,
    mut sleep_ticks: impl FnMut(u64),
) -> bool {
    let started = now_ticks();
    loop {
        let current = status();
        if current & DVD_INIT as i32 == 0 {
            return current & DVD_READY as i32 != 0;
        }
        if now_ticks().wrapping_sub(started) > timeout_ticks {
            return false;
        }
        sleep_ticks(poll_ticks);
    }
}

fn mount_expected_result(stage: u8) -> i32 {
    if stage == MOUNT_RESET { 4 } else { 1 }
}

fn submit_mount_ioctl() -> i32 {
    let (raw_fd, stage) = with_state(|state| {
        (
            state.fd.as_ref().map(FileDescriptor::raw),
            state.mount_stage,
        )
    });
    let Some(raw_fd) = raw_fd else { return ENXIO };
    let ioctl = match stage {
        MOUNT_GET_COVER => DVD_GETCOVER,
        MOUNT_WAIT_FOR_COVER => DVD_WAITFORCOVERCLOSE,
        MOUNT_RESET => DVD_RESET,
        MOUNT_IDENTIFY => DVD_IDENTIFY,
        MOUNT_READ_DISC_ID => DVD_READ_DISCID,
        _ => return EINVAL,
    };
    unsafe {
        let io = core::ptr::addr_of_mut!(MOUNT_IO);
        (*io).command.0 = [0; 8];
        (*io).command.0[0] = ioctl << 24;
        (*io).command.0[1] = u32::from(stage == MOUNT_RESET);
        let input =
            core::slice::from_raw_parts_mut((*io).command.0.as_mut_ptr().cast::<u8>(), 0x20);
        let output = &mut (*io).output.0;
        let buffers = IoctlBuffers::from_slices(Some(input), Some(output));
        FileDescriptor::new(raw_fd).ioctl_async(
            ioctl as i32,
            buffers,
            Some(mount_ioctl_callback),
            core::ptr::null_mut(),
        )
    }
}

unsafe extern "C" fn mount_ioctl_callback(result: i32, _userdata: *mut c_void) -> i32 {
    let (next_stage, exhausted) = with_state(|state| {
        let stage = state.mount_stage;
        let output_word;
        unsafe {
            let out = core::ptr::addr_of!(MOUNT_IO.output.0).cast::<u8>();
            output_word = u32::from_ne_bytes([*out, *out.add(1), *out.add(2), *out.add(3)]);
        }
        if stage == MOUNT_GET_COVER || stage == MOUNT_WAIT_FOR_COVER {
            state.mount_inserted = output_word & DVD_COVER_DISC_INSERTED != 0;
        }
        if result == mount_expected_result(stage) {
            state.mount_retries = 16;
            state.mount_stage = mount_next_stage(stage, state.mount_inserted);
        } else if state.mount_retries > 0 {
            state.mount_retries -= 1;
        } else {
            state.status = DVD_UNKNOWN;
            return (state.mount_stage, true);
        }
        (state.mount_stage, false)
    });

    if exhausted {
        let callback = with_state(|state| state.mount_callback.take());
        if let Some(callback) = callback {
            unsafe { callback(DVD_UNKNOWN, EIO as u32) };
        }
        return 0;
    }
    if next_stage == 5 {
        let (init_callback, mount_callback) = with_state(|state| {
            set_state(state, DVD_READY);
            state.read_mode = ReadMode::Unknown;
            (state.init_callback, state.mount_callback.take())
        });
        if let Some(callback) = init_callback {
            unsafe { callback(DVD_READY, 0) };
        }
        if let Some(callback) = mount_callback {
            unsafe { callback(DVD_READY, 0) };
        }
        return 0;
    }
    let rc = submit_mount_ioctl();
    if rc < 0 {
        let callback = with_state(|state| {
            state.status = DVD_UNKNOWN;
            state.mount_callback.take()
        });
        if let Some(callback) = callback {
            unsafe { callback(DVD_UNKNOWN, rc as u32) };
        }
    }
    0
}

fn get_cover_register_inner(state: &DiState, status: &mut u32) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    let rc = ioctl_words(fd, DVD_GETCOVER, &mut dic, Some(&mut outbuf.0));
    if rc == 0 {
        *status = u32::from_ne_bytes([outbuf.0[0], outbuf.0[1], outbuf.0[2], outbuf.0[3]]);
        Ok(())
    } else {
        Err(rc)
    }
}

fn identify_inner(state: &mut DiState) -> Result<DiDriveId, i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_IDENTIFY << 24;
    let rc = ioctl_words(fd, DVD_IDENTIFY, &mut dic, Some(&mut outbuf.0));
    if rc != 0 {
        return Err(rc);
    }
    Ok(DiDriveId {
        rev: u16::from_ne_bytes([outbuf.0[0], outbuf.0[1]]),
        dev_code: u16::from_ne_bytes([outbuf.0[2], outbuf.0[3]]),
        rel_date: u32::from_ne_bytes([outbuf.0[4], outbuf.0[5], outbuf.0[6], outbuf.0[7]]),
    })
}

fn get_error_inner(state: &DiState) -> Result<u32, i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_GET_ERROR << 24;
    let rc = ioctl_words(fd, DVD_GET_ERROR, &mut dic, Some(&mut outbuf.0));
    if rc == 0 {
        Ok(u32::from_ne_bytes([
            outbuf.0[0],
            outbuf.0[1],
            outbuf.0[2],
            outbuf.0[3],
        ]))
    } else {
        Err(rc)
    }
}

fn read_disc_id_bytes_inner(state: &mut DiState, output: &mut [u8; 0x20]) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_READ_DISCID << 24;
    let rc = ioctl_words(fd, DVD_READ_DISCID, &mut dic, Some(&mut outbuf.0));
    if rc != 0 {
        return Err(rc);
    }
    output.copy_from_slice(&outbuf.0);
    Ok(())
}

fn read_disc_id_inner(state: &mut DiState) -> Result<u64, i32> {
    let mut bytes = [0u8; 0x20];
    read_disc_id_bytes_inner(state, &mut bytes)?;
    Ok(u64::from_ne_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn reset_inner(state: &mut DiState) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_RESET << 24;
    dic.0[1] = 1;
    let rc = ioctl_words(fd, DVD_RESET, &mut dic, Some(&mut outbuf.0));
    if rc == 0 { Ok(()) } else { Err(rc) }
}

fn audio_stream_inner(
    state: &mut DiState,
    subcmd: u32,
    len: u32,
    offset_words: u32,
) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = (DVD_AUDIO_STREAM << 24) | subcmd;
    dic.0[1] = offset_words;
    dic.0[2] = len;
    let rc = ioctl_words(fd, DVD_AUDIO_STREAM, &mut dic, Some(&mut outbuf.0));
    if rc == 0 { Ok(()) } else { Err(rc) }
}

fn set_motor_inner(state: &mut DiState, flag: u32) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_SET_MOTOR << 24;
    dic.0[1] = flag & 0x1;
    dic.0[2] = (flag >> 1) & 0x1;
    let rc = ioctl_words(fd, DVD_SET_MOTOR, &mut dic, Some(&mut outbuf.0));
    if rc == 0 { Ok(()) } else { Err(rc) }
}

fn read_dvd_config_inner(state: &mut DiState, val: &mut u32, flag: u32) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_READ_CONFIG << 24;
    dic.0[1] = flag & 0x1;
    dic.0[3] = *val;
    let rc = ioctl_words(fd, DVD_READ_CONFIG, &mut dic, Some(&mut outbuf.0));
    if rc == 0 {
        *val = u32::from_ne_bytes([outbuf.0[0], outbuf.0[1], outbuf.0[2], outbuf.0[3]]);
        Ok(())
    } else {
        Err(rc)
    }
}

fn read_copy_inner(state: &mut DiState, out: &mut u32) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_READ_COPYRIGHT << 24;
    let rc = ioctl_words(fd, DVD_READ_COPYRIGHT, &mut dic, Some(&mut outbuf.0));
    if rc == 0 {
        *out = u32::from_ne_bytes([outbuf.0[0], outbuf.0[1], outbuf.0[2], outbuf.0[3]]);
        Ok(())
    } else {
        Err(rc)
    }
}

fn partition_ioctlv_inner(state: &mut DiState, offset: u32) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_OPEN_PARTITION << 24;
    dic.0[1] = offset;
    let dic_bytes =
        unsafe { core::slice::from_raw_parts_mut(dic.0.as_mut_ptr().cast::<u8>(), 0x20) };
    let certs = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(PARTITION_CERTS.0).cast::<u8>(),
            0x49e4,
        )
    };
    let vectors = [
        IoctlvEntry::In(dic_bytes),
        IoctlvEntry::InNull(0x2a4),
        IoctlvEntry::InNull(0),
        IoctlvEntry::Out(certs),
        IoctlvEntry::Out(&mut outbuf.0),
    ];
    let rc = normalize_di_result(fd.ioctlv(DVD_OPEN_PARTITION as i32, 3, vectors));
    if rc == 0 { Ok(()) } else { Err(rc) }
}

fn close_partition_inner(state: &DiState) -> Result<(), i32> {
    let fd = current_fd(state)?;
    let mut dic = CommandWords::zeroed();
    let mut outbuf = Output32([0; 0x20]);
    dic.0[0] = DVD_CLOSE_PARTITION << 24;
    let rc = ioctl_words(fd, DVD_CLOSE_PARTITION, &mut dic, Some(&mut outbuf.0));
    if rc == 0 { Ok(()) } else { Err(rc) }
}

pub fn init() -> i32 {
    let result = with_state(|state| {
        if state.fd.is_some() {
            return 1;
        }

        // AHBPROT is only required for direct DI register access. Normal Wii
        // applications must still be able to use the IOS `/dev/di` transport.
        state.have_ahbprot = read_ahbprot();

        match open_di(state) {
            Ok(()) => {
                state.status = DVD_INIT | DVD_NO_DISC;
                0
            }
            Err(rc) => rc,
        }
    });
    sync_legacy_fd();
    result
}

pub fn load_dvdx(load: bool) {
    with_state(|state| {
        state.load_dvdx = load;
    });
}

pub fn use_cache(use_cache: bool) {
    with_state(|state| {
        state.use_cache = use_cache;
        if use_cache {
            state.cache_block = CACHE_FREE;
        }
    });
}

pub fn set_init_callback(cb: DiCallback) {
    with_state(|state| {
        state.init_callback = cb;
    });
}

/// Sets a one-shot callback for the active mount request. This callback runs
/// after success or terminal failure. The legacy init callback keeps its
/// success-only behavior.
pub fn set_mount_callback(cb: DiCallback) {
    with_state(|state| state.mount_callback = cb);
}

pub fn mount() {
    // BUG FIX: C checks the cover register before it starts the async mount
    // chain. With no disc, it sets DVD_NO_DISC and does not wait for insertion.
    let (precheck, ready) = with_state(|state| {
        let mut cover_status = 0;
        let cover_result =
            get_cover_register_inner(state, &mut cover_status).map(|()| cover_status);
        let precheck = mount_cover_precheck(state.fd.is_some(), cover_result);
        if precheck != MountPrecheck::Start {
            match precheck {
                MountPrecheck::NoDevice => {}
                MountPrecheck::NoDisc | MountPrecheck::CoverError(_) => {
                    set_state(state, DVD_NO_DISC);
                }
                MountPrecheck::Start => unreachable!(),
            }
            return (precheck, false);
        }
        state.mount_stage = MOUNT_GET_COVER;
        state.mount_retries = 16;
        state.mount_inserted = false;
        state.read_mode = ReadMode::Unknown;
        state.cache_block = CACHE_FREE;
        set_state(state, DVD_INIT | DVD_NO_DISC);
        (precheck, true)
    });
    if ready {
        let rc = submit_mount_ioctl();
        if rc < 0 {
            let callback = with_state(|state| {
                set_state(state, DVD_UNKNOWN);
                state.mount_callback.take()
            });
            if let Some(callback) = callback {
                unsafe { callback(DVD_UNKNOWN, rc as u32) };
            }
        }
    } else {
        let (precheck, callback) = with_state(|state| (precheck, state.mount_callback.take()));
        if let Some(callback) = callback {
            match precheck {
                MountPrecheck::NoDevice => unsafe {
                    callback(DVD_UNKNOWN, ENXIO as u32);
                },
                MountPrecheck::NoDisc => unsafe {
                    callback(DVD_NO_DISC, 0);
                },
                MountPrecheck::CoverError(error) => unsafe {
                    callback(DVD_NO_DISC, error as u32);
                },
                MountPrecheck::Start => unreachable!(),
            }
        }
    }
}

/// Closes the DI transport and retains it for retry when IOS rejects the close.
pub fn close() -> i32 {
    let result = with_state(|state| {
        if let Err(error) =
            shutdown::close_transport(&mut state.fd, |fd| FileDescriptor::new(fd.raw()).close())
        {
            return error;
        }
        state.read_mode = ReadMode::Unknown;
        state.status = DVD_INIT | DVD_NO_DISC;
        state.cache_block = CACHE_FREE;
        0
    });
    sync_legacy_fd();
    result
}

pub fn get_status() -> i32 {
    with_state(|state| state.status as i32)
}

pub fn identify(id: &mut DiDriveId) -> i32 {
    with_state(|state| match identify_inner(state) {
        Ok(value) => {
            *id = value;
            0
        }
        Err(rc) => rc,
    })
}

pub fn check_dvd_support() -> i32 {
    let mut id = DiDriveId::default();
    if identify(&mut id) == 0 && id.rel_date <= 0x2008_0714 {
        1
    } else {
        0
    }
}

pub fn read_disc_id(id: &mut u64) -> i32 {
    with_state(|state| match read_disc_id_inner(state) {
        Ok(value) => {
            *id = value;
            0
        }
        Err(rc) => rc,
    })
}

/// Reads the complete 32-byte optical-disc identification record.
pub fn read_disc_id_bytes(output: &mut [u8; 0x20]) -> i32 {
    with_state(|state| match read_disc_id_bytes_inner(state, output) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

pub fn get_error(error: &mut u32) -> i32 {
    with_state(|state| match get_error_inner(state) {
        Ok(value) => {
            *error = value;
            0
        }
        Err(rc) => rc,
    })
}

pub fn get_cover_register(status: &mut u32) -> i32 {
    with_state(|state| match get_cover_register_inner(state, status) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

pub fn reset() -> i32 {
    with_state(|state| match reset_inner(state) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

pub fn stop_motor() -> i32 {
    with_state(|state| match set_motor_inner(state, 0) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

/// Issues the DI audio-stream command used by libogc DVD streaming APIs.
pub fn audio_stream(subcmd: u32, len: u32, offset_words: u32) -> i32 {
    with_state(
        |state| match audio_stream_inner(state, subcmd, len, offset_words) {
            Ok(()) => 0,
            Err(rc) => rc,
        },
    )
}

pub fn eject() -> i32 {
    with_state(|state| match set_motor_inner(state, 1) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

pub fn kill_drive() -> i32 {
    with_state(|state| match set_motor_inner(state, 2) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

/// # Safety
///
/// `buf` must point to a valid writable buffer large enough for `len << 11`
/// bytes and satisfy the DI alignment rules.
pub unsafe fn read_dvd(buf: *mut c_void, len: u32, lba: u32) -> i32 {
    with_state(|state| read_from_cache_or_drive(state, buf, len, lba))
}

fn submit_async_dvd_read(
    state: &mut DiState,
    buf: *mut c_void,
    len: u32,
    lba: u32,
    ipc_cb: Ipccallback,
    use_d0: bool,
) -> i32 {
    let fd = match current_fd(state) {
        Ok(fd) => fd,
        Err(error) => return error,
    };
    let Some(bytes_u32) = len.checked_mul(BLOCK_SIZE as u32) else {
        return EINVAL;
    };
    if !use_d0 && lba.checked_mul(1 << 9).is_none() {
        return EINVAL;
    }
    let bytes = bytes_u32 as usize;
    let ioctl = if use_d0 {
        DVD_READ
    } else {
        DVD_READ_UNENCRYPTED
    };

    unsafe {
        let command = core::ptr::addr_of_mut!(ASYNC_READ_COMMAND);
        build_dvd_command(&mut *command, ioctl, len, lba, use_d0);
        let input = core::slice::from_raw_parts_mut((*command).0.as_mut_ptr().cast::<u8>(), 0x20);
        let output = core::slice::from_raw_parts_mut(buf.cast::<u8>(), bytes);
        fd.ioctl_async(
            ioctl as i32,
            IoctlBuffers::from_slices(Some(input), Some(output)),
            ipc_cb,
            buf,
        )
    }
}

fn submit_async_dvd_read_selected(
    state: &mut DiState,
    buf: *mut c_void,
    len: u32,
    lba: u32,
    ipc_cb: Ipccallback,
) -> i32 {
    match state.read_mode {
        ReadMode::D0 => submit_async_dvd_read(state, buf, len, lba, ipc_cb, true),
        ReadMode::A8 => submit_async_dvd_read(state, buf, len, lba, ipc_cb, false),
        ReadMode::Unknown => {
            let first = submit_async_dvd_read(state, buf, len, lba, ipc_cb, true);
            if first == 0 {
                state.read_mode = ReadMode::D0;
                set_state(state, state.status | DVD_D0);
                return 0;
            }
            let second = submit_async_dvd_read(state, buf, len, lba, ipc_cb, false);
            if second == 0 {
                state.read_mode = ReadMode::A8;
                set_state(state, state.status | DVD_A8);
            }
            second
        }
    }
}

/// Submits the active DI read command directly to IOS asynchronously.
///
/// This intentionally bypasses the synchronous sector cache, matching
/// libogc's `DI_ReadDVDAsync` transport path. The caller callback receives the
/// final IOS reply and the original buffer as user data.
/// # Safety
///
/// `buf` must remain writable and 32-byte aligned until the callback runs.
pub unsafe fn read_dvd_async(buf: *mut c_void, len: u32, lba: u32, ipc_cb: Ipccallback) -> i32 {
    with_state(|state| {
        if let Err(rc) = async_read_precheck(state, buf, ipc_cb) {
            return rc;
        }
        submit_async_dvd_read_selected(state, buf, len, lba, ipc_cb)
    })
}

/// # Safety
///
/// `buf` must point to a valid writable buffer of `size` bytes and be
/// 32-byte aligned.
pub unsafe fn read(buf: *mut c_void, size: u32, offset: u32) -> i32 {
    with_state(|state| {
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let mut dic = CommandWords::zeroed();
        dic.0[0] = DVD_LOW_READ << 24;
        dic.0[1] = size;
        dic.0[2] = offset;
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, size as usize) };
        ioctl_words(fd, DVD_LOW_READ, &mut dic, Some(out))
    })
}

/// # Safety
///
/// `buf` must point to a valid writable buffer of `size` bytes and be
/// 32-byte aligned.
pub unsafe fn unencrypted_read(buf: *mut c_void, size: u32, offset: u32) -> i32 {
    with_state(|state| {
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let mut dic = CommandWords::zeroed();
        dic.0[0] = DVD_READ_UNENCRYPTED << 24;
        dic.0[1] = size;
        dic.0[2] = offset;
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, size as usize) };
        retry::retry_di_read(
            || ioctl_words(fd, DVD_READ_UNENCRYPTED, &mut dic, Some(&mut *out)),
            LIBDI_MAX_RETRIES,
        )
    })
}

pub fn read_dvd_config(val: &mut u32, flag: u32) -> i32 {
    with_state(|state| match read_dvd_config_inner(state, val, flag) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

pub fn read_dvd_copyright(copyright: &mut u32) -> i32 {
    with_state(|state| match read_copy_inner(state, copyright) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

/// # Safety
///
/// `buf` must point to a 0x800-byte writable buffer and be 32-byte aligned.
pub unsafe fn read_dvd_disc_key(buf: *mut c_void) -> i32 {
    with_state(|state| {
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, 0x800) };
        let mut dic = CommandWords::zeroed();
        dic.0[0] = DVD_READ_DISCKEY << 24;
        retry::retry_di_read(
            || ioctl_words(fd, DVD_READ_DISCKEY, &mut dic, Some(&mut *out)),
            LIBDI_MAX_RETRIES,
        )
    })
}

/// # Safety
///
/// `buf` must point to a 0x800-byte writable buffer and be 32-byte aligned.
pub unsafe fn read_dvd_physical(buf: *mut c_void) -> i32 {
    with_state(|state| {
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, 0x800) };
        let mut dic = CommandWords::zeroed();
        dic.0[0] = DVD_READ_PHYSICAL << 24;
        retry::retry_di_read(
            || ioctl_words(fd, DVD_READ_PHYSICAL, &mut dic, Some(&mut *out)),
            LIBDI_MAX_RETRIES,
        )
    })
}

/// # Safety
///
/// `buf` must point to a writable 64-byte buffer and be 32-byte aligned.
pub unsafe fn read_bca(buf: *mut c_void) -> i32 {
    with_state(|state| {
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, 64) };
        let mut dic = CommandWords::zeroed();
        dic.0[0] = DVD_READ_BCA << 24;
        ioctl_words(fd, DVD_READ_BCA, &mut dic, Some(out))
    })
}

/// # Safety
///
/// `buf` must point to a 0x20-byte writable buffer and be 32-byte aligned.
pub unsafe fn report_key(keytype: i32, lba: u32, buf: *mut c_void) -> i32 {
    with_state(|state| {
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        if buf.is_null() {
            return EINVAL;
        }
        if (buf as usize) & 0x1f != 0 {
            return EFAULT;
        }
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, 0x20) };
        let mut dic = CommandWords::zeroed();
        dic.0[0] = DVD_REPORTKEY << 24;
        dic.0[1] = (keytype as u32) & 0xFF;
        dic.0[2] = lba;
        ioctl_words(fd, DVD_REPORTKEY, &mut dic, Some(out))
    })
}

pub fn open_partition(offset: u32) -> i32 {
    with_state(|state| match partition_ioctlv_inner(state, offset) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

pub fn close_partition() -> i32 {
    with_state(|state| match close_partition_inner(state) {
        Ok(()) => 0,
        Err(rc) => rc,
    })
}

unsafe extern "C" fn diio_startup() -> bool {
    if with_state(|state| state.fd.is_none()) {
        return false;
    }
    mount();
    // BUG FIX: C waits for the asynchronous mount callback chain to finish
    // before its synchronous DiscInterface startup callback returns.
    wait_for_mount(
        MOUNT_TIMEOUT_TICKS,
        MOUNT_POLL_TICKS,
        get_status,
        lwp::now_ticks,
        lwp::sleep_ticks,
    )
}

unsafe extern "C" fn diio_is_inserted() -> bool {
    let mut status = 0;
    get_cover_register(&mut status) == 0 && (status & DVD_COVER_DISC_INSERTED) != 0
}

unsafe extern "C" fn diio_read_sectors(sector: u32, num_sectors: u32, buffer: *mut c_void) -> bool {
    unsafe { read_dvd(buffer, num_sectors, sector) == 0 }
}

unsafe extern "C" fn diio_write_sectors(
    _sector: u32,
    _num_sectors: u32,
    _buffer: *const c_void,
) -> bool {
    // Safety deviation: libogc reports success without writing. The DVD
    // interface does not advertise write capability, so unsupported writes
    // must fail rather than report a transfer that never happened.
    false
}

unsafe extern "C" fn diio_clear_status() -> bool {
    // BUG FIX: libogc's DI interface clear-status callback is an unconditional
    // success and does not submit DVD_GET_ERROR.
    true
}

fn diio_shutdown_inner(state: &mut DiState) -> bool {
    // BUG FIX: the C DiscInterface shutdown callback only tries to stop the
    // motor. It keeps the DI handle and partition state open.
    let _ = set_motor_inner(state, 0);
    true
}

unsafe extern "C" fn diio_shutdown() -> bool {
    with_state(diio_shutdown_inner)
}

pub const WII_DVD_INTERFACE: DiscInterface = DiscInterface {
    io_type: DEVICE_TYPE_WII_DVD,
    features: FEATURE_MEDIUM_CANREAD | FEATURE_WII_DVD,
    startup: Some(diio_startup),
    is_inserted: Some(diio_is_inserted),
    read_sectors: Some(diio_read_sectors),
    write_sectors: Some(diio_write_sectors),
    clear_status: Some(diio_clear_status),
    shutdown: Some(diio_shutdown),
};

/// Legacy libogc global descriptor value. Rust operations use the locked
/// descriptor in `DiState`; this exported value mirrors that descriptor.
#[unsafe(no_mangle)]
pub static mut di_fd: i32 = -1;

/// Legacy libogc disc interface symbol.
#[unsafe(no_mangle)]
pub static __io_wiidvd: DiscInterface = DiscInterface {
    io_type: DEVICE_TYPE_WII_DVD,
    features: FEATURE_MEDIUM_CANREAD | FEATURE_WII_DVD,
    startup: Some(diio_startup),
    is_inserted: Some(diio_is_inserted),
    read_sectors: Some(diio_read_sectors),
    write_sectors: Some(diio_write_sectors),
    clear_status: Some(diio_clear_status),
    shutdown: Some(diio_shutdown),
};

fn sync_legacy_fd() {
    let raw_fd = with_state(|state| state.fd.as_ref().map_or(-1, FileDescriptor::raw));
    unsafe { di_fd = raw_fd };
}

// Legacy libogc C ABI wrappers. Pointer arguments are validated before they
// become Rust references. The underlying operations retain their existing
// state and transport behavior.
#[unsafe(no_mangle)]
pub extern "C" fn DI_Init() -> i32 {
    init()
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_LoadDVDX(load: bool) {
    load_dvdx(load)
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_UseCache(enable: bool) {
    use_cache(enable)
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_SetInitCallback(callback: DiCallback) {
    set_init_callback(callback)
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_Mount() {
    mount()
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_Close() {
    close();
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_GetStatus() -> i32 {
    get_status()
}

#[unsafe(no_mangle)]
/// # Safety
/// A non-null `id` must be aligned and writable for one `DiDriveId` until this call returns.
pub unsafe extern "C" fn DI_Identify(id: *mut DiDriveId) -> i32 {
    if with_state(|state| state.fd.is_none()) {
        return ENXIO;
    }
    if id.is_null() {
        return EINVAL;
    }
    unsafe { identify(&mut *id) }
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_CheckDVDSupport() -> i32 {
    check_dvd_support()
}

#[unsafe(no_mangle)]
/// # Safety
/// A non-null `id` must be aligned and writable for one `u64` until this call returns.
pub unsafe extern "C" fn DI_ReadDiscID(id: *mut u64) -> i32 {
    if with_state(|state| state.fd.is_none()) {
        return ENXIO;
    }
    if id.is_null() {
        return EINVAL;
    }
    unsafe { read_disc_id(&mut *id) }
}

#[unsafe(no_mangle)]
/// # Safety
/// A non-null `error` must be aligned and writable for one `u32` until this call returns.
pub unsafe extern "C" fn DI_GetError(error: *mut u32) -> i32 {
    if with_state(|state| state.fd.is_none()) {
        return ENXIO;
    }
    if error.is_null() {
        return EINVAL;
    }
    unsafe { get_error(&mut *error) }
}

#[unsafe(no_mangle)]
/// # Safety
/// A non-null `status` must be aligned and writable for one `u32` until this call returns.
pub unsafe extern "C" fn DI_GetCoverRegister(status: *mut u32) -> i32 {
    if with_state(|state| state.fd.is_none()) {
        return ENXIO;
    }
    if status.is_null() {
        return EINVAL;
    }
    unsafe { get_cover_register(&mut *status) }
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_Reset() -> i32 {
    reset()
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_StopMotor() -> i32 {
    stop_motor()
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_Eject() -> i32 {
    eject()
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_KillDrive() -> i32 {
    kill_drive()
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`read_dvd`] apply to this C entry point.
pub unsafe extern "C" fn DI_ReadDVD(buf: *mut c_void, len: u32, lba: u32) -> i32 {
    unsafe { read_dvd(buf, len, lba) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`read_dvd_async`] apply to this C entry point.
pub unsafe extern "C" fn DI_ReadDVDAsync(
    buf: *mut c_void,
    len: u32,
    lba: u32,
    callback: Ipccallback,
) -> i32 {
    unsafe { read_dvd_async(buf, len, lba, callback) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`read`] apply to this C entry point.
pub unsafe extern "C" fn DI_Read(buf: *mut c_void, size: u32, offset: u32) -> i32 {
    unsafe { read(buf, size, offset) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`unencrypted_read`] apply to this C entry point.
pub unsafe extern "C" fn DI_UnencryptedRead(buf: *mut c_void, size: u32, offset: u32) -> i32 {
    unsafe { unencrypted_read(buf, size, offset) }
}

#[unsafe(no_mangle)]
/// # Safety
/// A non-null `val` must be aligned and writable for one `u32` until this call returns.
pub unsafe extern "C" fn DI_ReadDVDConfig(val: *mut u32, flag: u32) -> i32 {
    if with_state(|state| state.fd.is_none()) {
        return ENXIO;
    }
    if val.is_null() {
        return EINVAL;
    }
    unsafe { read_dvd_config(&mut *val, flag) }
}

#[unsafe(no_mangle)]
/// # Safety
/// A non-null `copyright` must be aligned and writable for one `u32` until this call returns.
pub unsafe extern "C" fn DI_ReadDVDCopyright(copyright: *mut u32) -> i32 {
    if with_state(|state| state.fd.is_none()) {
        return ENXIO;
    }
    if copyright.is_null() {
        return EINVAL;
    }
    unsafe { read_dvd_copyright(&mut *copyright) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`read_dvd_disc_key`] apply to this C entry point.
pub unsafe extern "C" fn DI_ReadDVDDiscKey(buf: *mut c_void) -> i32 {
    unsafe { read_dvd_disc_key(buf) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`read_dvd_physical`] apply to this C entry point.
pub unsafe extern "C" fn DI_ReadDVDPhysical(buf: *mut c_void) -> i32 {
    unsafe { read_dvd_physical(buf) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`read_bca`] apply to this C entry point.
pub unsafe extern "C" fn DI_Read_BCA(buf: *mut c_void) -> i32 {
    unsafe { read_bca(buf) }
}

#[unsafe(no_mangle)]
/// # Safety
/// The buffer and callback requirements of [`report_key`] apply to this C entry point.
pub unsafe extern "C" fn DI_ReportKey(keytype: i32, lba: u32, buf: *mut c_void) -> i32 {
    unsafe { report_key(keytype, lba, buf) }
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_OpenPartition(offset: u32) -> i32 {
    open_partition(offset)
}

#[unsafe(no_mangle)]
pub extern "C" fn DI_ClosePartition() -> i32 {
    close_partition()
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn legacy_c_interface_export_matches_wii_dvd_metadata() {
        assert_eq!(__io_wiidvd.io_type, DEVICE_TYPE_WII_DVD);
        assert_eq!(
            __io_wiidvd.features,
            FEATURE_MEDIUM_CANREAD | FEATURE_WII_DVD
        );
        assert!(__io_wiidvd.startup.is_some());
        assert!(__io_wiidvd.is_inserted.is_some());
        assert!(__io_wiidvd.read_sectors.is_some());
        assert!(__io_wiidvd.clear_status.is_some());
        assert!(__io_wiidvd.shutdown.is_some());
        let _: extern "C" fn() -> i32 = DI_Init;
        let _: extern "C" fn() -> i32 = DI_GetStatus;
        let _: extern "C" fn() = DI_Mount;
        let _: extern "C" fn() = DI_Close;
    }

    unsafe extern "C" fn count_async_callback(_: i32, _: *mut c_void) -> i32 {
        ASYNC_TEST_CALLBACKS.fetch_add(1, Ordering::SeqCst);
        0
    }

    static ASYNC_TEST_CALLBACKS: core::sync::atomic::AtomicU32 =
        core::sync::atomic::AtomicU32::new(0);

    #[test]
    fn async_submission_rejects_pre_init_and_pre_mount_without_callback() {
        let callback: Ipccallback = Some(count_async_callback);
        #[repr(align(32))]
        struct AlignedBuffer([u8; 32]);
        let mut buffer = AlignedBuffer([0; 32]);
        let ptr = buffer.0.as_mut_ptr().cast::<c_void>();
        ASYNC_TEST_CALLBACKS.store(0, Ordering::SeqCst);

        let (saved_fd, saved_status) = with_state(|state| {
            let saved = (state.fd.take(), state.status);
            state.status = DVD_INIT | DVD_NO_DISC;
            saved
        });
        assert_eq!(unsafe { read_dvd_async(ptr, 1, 0, callback) }, ENXIO);
        assert_eq!(
            unsafe { read_dvd_async(core::ptr::null_mut(), 1, 0, None) },
            ENXIO
        );
        assert_eq!(ASYNC_TEST_CALLBACKS.load(Ordering::SeqCst), 0);

        with_state(|state| {
            state.fd = Some(FileDescriptor::new(0));
            state.status = DVD_INIT | DVD_NO_DISC;
        });
        assert_eq!(unsafe { read_dvd_async(ptr, 1, 0, callback) }, -1);
        assert_eq!(ASYNC_TEST_CALLBACKS.load(Ordering::SeqCst), 0);

        // C checks descriptor and readiness before validating arguments.
        assert_eq!(
            unsafe { read_dvd_async(core::ptr::null_mut(), 1, 0, None) },
            -1
        );
        assert_eq!(ASYNC_TEST_CALLBACKS.load(Ordering::SeqCst), 0);

        with_state(|state| {
            state.fd = saved_fd;
            state.status = saved_status;
        });
    }

    #[test]
    fn async_submission_uses_c_buffer_alignment_errors() {
        let callback: Ipccallback = Some(count_async_callback);
        #[repr(align(32))]
        struct AlignedBuffer([u8; 64]);
        let mut buffer = AlignedBuffer([0; 64]);
        let aligned = buffer.0.as_mut_ptr().cast::<c_void>();
        let misaligned = unsafe { buffer.0.as_mut_ptr().add(1).cast::<c_void>() };

        let mut state = DiState::new();
        state.fd = Some(FileDescriptor::new(0));
        state.status = DVD_READY;

        assert_eq!(
            async_read_precheck(&state, core::ptr::null_mut(), callback),
            Err(EINVAL)
        );
        assert_eq!(
            async_read_precheck(&state, misaligned, callback),
            Err(EFAULT)
        );
        assert_eq!(async_read_precheck(&state, aligned, None), Err(EINVAL));
        assert_eq!(async_read_precheck(&state, aligned, callback), Ok(()));
    }

    #[test]
    fn layout_matches_c() {
        assert_eq!(core::mem::size_of::<DiDriveId>(), 8);
        assert_eq!(DVD_COVER_DISC_INSERTED, 0x02);
        assert_eq!(LIBDI_MAX_RETRIES, 16);
        assert_eq!(DEVICE_TYPE_WII_DVD, 0x5744_5644);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn() -> i32;
        let _ = load_dvdx as fn(bool);
        let _ = use_cache as fn(bool);
        let _ = set_init_callback as fn(DiCallback);
        let _ = mount as fn();
        let _ = close as fn() -> i32;
        let _ = get_status as fn() -> i32;
        let _ = identify as fn(&mut DiDriveId) -> i32;
        let _ = check_dvd_support as fn() -> i32;
        let _ = read_disc_id as fn(&mut u64) -> i32;
        let _ = get_error as fn(&mut u32) -> i32;
        let _ = get_cover_register as fn(&mut u32) -> i32;
        let _ = reset as fn() -> i32;
        let _ = stop_motor as fn() -> i32;
        let _ = eject as fn() -> i32;
        let _ = kill_drive as fn() -> i32;
        let _ = read as unsafe fn(*mut c_void, u32, u32) -> i32;
        let _ = unencrypted_read as unsafe fn(*mut c_void, u32, u32) -> i32;
        let _ = read_dvd_config as fn(&mut u32, u32) -> i32;
        let _ = read_dvd_copyright as fn(&mut u32) -> i32;
        let _ = open_partition as fn(u32) -> i32;
        let _ = close_partition as fn() -> i32;
    }

    #[test]
    fn disc_interface_write_callback_rejects_unsupported_writes() {
        assert!(WII_DVD_INTERFACE.read_sectors.is_some());
        let write_sectors = WII_DVD_INTERFACE.write_sectors.unwrap();
        assert!(!unsafe { write_sectors(0, 0, core::ptr::null()) });
        let legacy_write_sectors = __io_wiidvd.write_sectors;
        assert!(!unsafe { legacy_write_sectors.unwrap()(0, 0, core::ptr::null()) });
    }

    #[test]
    fn mount_callbacks_advance_each_ios_stage() {
        assert_eq!(mount_next_stage(MOUNT_GET_COVER, true), MOUNT_RESET);
        assert_eq!(
            mount_next_stage(MOUNT_GET_COVER, false),
            MOUNT_WAIT_FOR_COVER
        );
        assert_eq!(
            mount_next_stage(MOUNT_WAIT_FOR_COVER, false),
            MOUNT_WAIT_FOR_COVER
        );
        assert_eq!(mount_next_stage(MOUNT_WAIT_FOR_COVER, true), MOUNT_RESET);
        assert_eq!(mount_next_stage(MOUNT_RESET, false), MOUNT_IDENTIFY);
        assert_eq!(mount_next_stage(MOUNT_IDENTIFY, false), MOUNT_READ_DISC_ID);
        assert_eq!(mount_next_stage(MOUNT_READ_DISC_ID, false), 5);
        assert_eq!(mount_expected_result(MOUNT_RESET), 4);
        assert_eq!(mount_expected_result(MOUNT_IDENTIFY), 1);
    }

    #[test]
    fn mount_precheck_does_not_wait_when_no_disc_is_present() {
        assert_eq!(mount_cover_precheck(true, Ok(0)), MountPrecheck::NoDisc);
        assert_eq!(
            mount_cover_precheck(true, Ok(DVD_COVER_DISC_INSERTED)),
            MountPrecheck::Start
        );
        assert_eq!(
            mount_cover_precheck(false, Ok(DVD_COVER_DISC_INSERTED)),
            MountPrecheck::NoDevice
        );
        assert_eq!(
            mount_cover_precheck(true, Err(EIO)),
            MountPrecheck::CoverError(EIO)
        );
    }

    #[test]
    fn startup_waits_for_mount_completion_and_returns_ready_state() {
        let tick = core::cell::Cell::new(0);
        let mut statuses = [DVD_INIT as i32, DVD_READY as i32].into_iter();
        assert!(wait_for_mount(
            100,
            5,
            || statuses.next().unwrap_or(DVD_READY as i32),
            || tick.get(),
            |delay| tick.set(tick.get() + delay),
        ));
        assert_eq!(tick.get(), 5);
    }

    #[test]
    fn startup_returns_false_for_no_disc_without_waiting() {
        let tick = core::cell::Cell::new(0);
        assert!(!wait_for_mount(
            100,
            5,
            || DVD_NO_DISC as i32,
            || tick.get(),
            |delay| tick.set(tick.get() + delay),
        ));
        assert_eq!(tick.get(), 0);
    }

    #[test]
    fn interface_shutdown_keeps_the_device_open_and_always_succeeds() {
        let mut state = DiState::new();
        assert!(diio_shutdown_inner(&mut state));
        assert!(state.fd.is_none());
        assert_eq!(state.status, DVD_INIT | DVD_NO_DISC);
    }

    #[test]
    fn interface_clear_status_is_unconditional_success() {
        assert!(unsafe { diio_clear_status() });
    }
}
