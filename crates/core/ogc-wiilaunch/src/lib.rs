#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::mem::size_of;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

pub use ogc_es as es;
pub use ogc_ipc as ipc;
pub use ogc_network_sys as network;

use es::Tikview;
use ogc_wiilaunch_state::{
    self as launch_state, Aligned, FileOperations, LaunchOperations, StateFile, StateValue,
};
pub use ogc_wiilaunch_state::{NandBootInfo, StateFlags};

pub const WII_ENOTINIT: i32 = -0x9001;
pub const WII_EINTERNAL: i32 = -0x9002;
pub const WII_ECHECKSUM: i32 = -0x9003;
pub const WII_EINSTALL: i32 = -0x9004;
pub const WII_E2BIG: i32 = -0x9005;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LaunchError {
    NotInitialized,
    Internal,
    Checksum,
    InstallMissing,
    ArgsTooLarge,
    Es(i32),
}

impl LaunchError {
    pub const fn raw_code(self) -> i32 {
        match self {
            Self::NotInitialized => WII_ENOTINIT,
            Self::Internal => WII_EINTERNAL,
            Self::Checksum => WII_ECHECKSUM,
            Self::InstallMissing => WII_EINSTALL,
            Self::ArgsTooLarge => WII_E2BIG,
            Self::Es(code) => code,
        }
    }

    pub const fn from_raw_code(code: i32) -> Self {
        match code {
            WII_ENOTINIT => Self::NotInitialized,
            WII_EINTERNAL => Self::Internal,
            WII_ECHECKSUM => Self::Checksum,
            WII_EINSTALL => Self::InstallMissing,
            WII_E2BIG => Self::ArgsTooLarge,
            other => Self::Es(other),
        }
    }
}

impl From<LaunchError> for ogc_sys::ErrorCode {
    fn from(value: LaunchError) -> Self {
        Self::new(value.raw_code())
    }
}

pub type LaunchResult<T> = core::result::Result<T, LaunchError>;

fn raw_result(result: LaunchResult<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => error.raw_code(),
    }
}

fn status(status: i32) -> LaunchResult<i32> {
    if status < 0 {
        Err(LaunchError::from_raw_code(status))
    } else {
        Ok(status)
    }
}

pub const SETTINGS_CALENDAR: &CStr = c"Calendar/Calendar_index.html";
pub const SETTINGS_DISPLAY: &CStr = c"Display/Display_index.html";
pub const SETTINGS_SOUND: &CStr = c"Sound/Sound_index.html";
pub const SETTINGS_PARENTAL: &CStr = c"Parental_Control/Parental_Control_index.html";
pub const SETTINGS_INTERNET: &CStr = c"Internet/Internet_index.html";
pub const SETTINGS_WC24: &CStr = c"WiiConnect24/Wiiconnect24_index.html";
pub const SETTINGS_UPDATE: &CStr = c"Update/Update_index.html";

const NANDBOOTINFO_PATH: &CStr = c"/shared2/sys/NANDBOOTINFO";
const STATEFLAGS_PATH: &CStr = c"/title/00000001/00000002/data/state.dat";
const RETURN_TO_MENU_TITLE_ID: u64 = 0x0000_0001_0000_0002;
const URL_TITLE_MASK: u64 = !0xff;
const URL_TITLE_BASE: u64 = 0x0001_0001_4841_4400;

const TYPE_RETURN: u8 = 3;
const RETURN_TO_MENU: u8 = 0;
const RETURN_TO_SETTINGS: u8 = 1;
const RETURN_TO_ARGS: u8 = 2;

struct LaunchState {
    initialized: bool,
    nandboot: NandBootInfo,
    stateflags: StateFlags,
}

impl LaunchState {
    const fn new() -> Self {
        Self {
            initialized: false,
            nandboot: NandBootInfo {
                checksum: 0,
                argsoff: 0,
                unk1: 0,
                unk2: 0,
                apptype: 0,
                titletype: 0,
                launchcode: 0,
                unknown: [0; 2],
                launcher: 0,
                argbuf: [0; 0x1000],
            },
            stateflags: StateFlags {
                checksum: 0,
                flags: 0,
                type_: 0,
                discstate: 0,
                returnto: 0,
                unknown: [0; 6],
            },
        }
    }
}

struct StateCell(UnsafeCell<LaunchState>);

// SAFETY: All access to the contained LaunchState is serialized by
// STATE_MUTEX. StateGuard holds that mutex for the lifetime of every
// reference created from STATE.
unsafe impl Sync for StateCell {}

static STATE: StateCell = StateCell(UnsafeCell::new(LaunchState::new()));
static STATE_MUTEX: AtomicU32 = AtomicU32::new(ogc_lwp_sys::LWP_MUTEX_NULL);

struct StateGuard {
    mutex: ogc_lwp_sys::Mutex,
}

impl Deref for StateGuard {
    type Target = LaunchState;

    fn deref(&self) -> &Self::Target {
        // SAFETY: This guard owns STATE_MUTEX for its entire lifetime.
        unsafe { &*STATE.0.get() }
    }
}

impl DerefMut for StateGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: This guard owns STATE_MUTEX exclusively for its lifetime.
        unsafe { &mut *STATE.0.get() }
    }
}

impl Drop for StateGuard {
    fn drop(&mut self) {
        let _ = ogc_lwp_sys::mutex_unlock(self.mutex);
    }
}

fn lock_state() -> LaunchResult<StateGuard> {
    let mut mutex = STATE_MUTEX.load(Ordering::Acquire);

    if mutex == ogc_lwp_sys::LWP_MUTEX_NULL {
        let mut candidate = ogc_lwp_sys::LWP_MUTEX_NULL;

        // SAFETY: `candidate` is a valid unique output location.
        if unsafe { ogc_lwp_sys::mutex_init(&mut candidate, 0) } != ogc_lwp_sys::LWP_SUCCESSFUL {
            return Err(LaunchError::Internal);
        }

        match STATE_MUTEX.compare_exchange(
            ogc_lwp_sys::LWP_MUTEX_NULL,
            candidate,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => mutex = candidate,
            Err(existing) => {
                let _ = ogc_lwp_sys::mutex_destroy(candidate);
                mutex = existing;
            }
        }
    }

    if ogc_lwp_sys::mutex_lock(mutex) != ogc_lwp_sys::LWP_SUCCESSFUL {
        return Err(LaunchError::Internal);
    }

    Ok(StateGuard { mutex })
}

fn read_struct<T: StateValue>(path: &CStr) -> LaunchResult<T> {
    let fd = ipc::open_file(path, ipc::IPC_OPEN_READ).map_err(|_| LaunchError::Internal)?;

    let mut value = Aligned(T::default());
    let bytes = unsafe {
        core::slice::from_raw_parts_mut((&mut value.0 as *mut T).cast::<u8>(), size_of::<T>())
    };
    let ret = fd.read(bytes);
    let _ = fd.close();
    if !matches!(ret, Ok(count) if count == size_of::<T>()) {
        return Err(LaunchError::Internal);
    }
    if !launch_state::valid_checksum(&value.0) {
        return Err(LaunchError::Checksum);
    }
    Ok(value.0)
}

struct IpcFileOperations<'a> {
    path: &'a CStr,
}

impl FileOperations for IpcFileOperations<'_> {
    type Handle = ipc::FileDescriptor;
    type Error = i32;

    fn open_write(&mut self) -> Result<Self::Handle, Self::Error> {
        ipc::open_file(self.path, ipc::IPC_OPEN_RW)
    }

    fn write(&mut self, handle: &Self::Handle, bytes: &[u8]) -> Result<usize, Self::Error> {
        handle.write(bytes)
    }

    fn close(&mut self, handle: Self::Handle) -> Result<(), Self::Error> {
        handle.close()
    }

    fn open_read(&mut self) -> Result<Self::Handle, Self::Error> {
        ipc::open_file(self.path, ipc::IPC_OPEN_READ)
    }

    fn read(&mut self, handle: &Self::Handle, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        handle.read(bytes)
    }
}

fn write_struct<T: StateValue>(path: &CStr, value: &mut T) -> LaunchResult<()> {
    let mut operations = IpcFileOperations { path };
    launch_state::write_verified(&mut operations, value).map_err(|_| LaunchError::Internal)
}

pub fn initialize() -> i32 {
    let mut state = match lock_state() {
        Ok(state) => state,
        Err(error) => return error.raw_code(),
    };

    state.stateflags = read_struct(STATEFLAGS_PATH).unwrap_or_default();
    state.nandboot = read_struct(NANDBOOTINFO_PATH).unwrap_or_default();
    state.initialized = true;
    0
}

pub fn try_initialize() -> LaunchResult<()> {
    let mut state = lock_state()?;
    state.stateflags = read_struct(STATEFLAGS_PATH)?;
    state.nandboot = read_struct(NANDBOOTINFO_PATH)?;
    state.initialized = true;
    Ok(())
}

pub fn launch_title(title_id: u64) -> i32 {
    raw_result(try_launch_title(title_id))
}

pub fn try_launch_title(title_id: u64) -> LaunchResult<()> {
    let mut state = lock_state()?;
    try_launch_title_inner(&mut state, title_id)
}

fn ticket_views(title_id: u64) -> LaunchResult<Aligned<[Tikview; 4]>> {
    let mut numviews = 0u32;
    status(unsafe { es::get_num_ticket_views(title_id, &mut numviews) })?;
    if numviews > 4 {
        return Err(LaunchError::Internal);
    }

    let mut views = Aligned([Tikview::default(); 4]);
    status(unsafe { es::get_ticket_views(title_id, views.0.as_mut_ptr(), numviews) })?;
    Ok(views)
}

fn default_nandboot(title_id: u64) -> NandBootInfo {
    let mut nandboot = NandBootInfo {
        apptype: 0x81,
        titletype: if title_id == RETURN_TO_MENU_TITLE_ID {
            4
        } else {
            2
        },
        ..NandBootInfo::default()
    };
    if unsafe { es::get_title_id(&mut nandboot.launcher) } < 0 {
        nandboot.launcher = RETURN_TO_MENU_TITLE_ID;
    }
    nandboot
}

struct WiiLaunchOperations<'a> {
    state: &'a mut LaunchState,
    title_id: u64,
    views: Aligned<[Tikview; 4]>,
    nandboot: NandBootInfo,
    stateflags: Option<StateFlags>,
}

impl LaunchOperations for WiiLaunchOperations<'_> {
    type Error = LaunchError;

    fn persist(&mut self, file: StateFile) -> LaunchResult<()> {
        match file {
            StateFile::NandBootInfo => {
                write_struct(NANDBOOTINFO_PATH, &mut self.nandboot)?;
                self.state.nandboot = self.nandboot;
            }
            StateFile::StateFlags => {
                let stateflags = self.stateflags.as_mut().ok_or(LaunchError::Internal)?;
                write_struct(STATEFLAGS_PATH, stateflags)?;
                self.state.stateflags = *stateflags;
            }
        }
        Ok(())
    }

    fn launch(&mut self) -> LaunchResult<()> {
        network::net_wc24cleanup();
        ogc_video::set_black(true);
        ogc_video::flush();
        let launch_result =
            status(unsafe { es::launch_title(self.title_id, self.views.0.as_ptr()) });
        // A successful title launch transfers control. Any return means the
        // caller remains active, so restore video output before propagating it.
        ogc_video::set_black(false);
        ogc_video::flush();
        match launch_result {
            Err(error) => Err(error),
            Ok(_) => Err(LaunchError::Internal),
        }
    }
}

fn execute_prepared_launch(
    state: &mut LaunchState,
    title_id: u64,
    views: Aligned<[Tikview; 4]>,
    nandboot: NandBootInfo,
    stateflags: Option<StateFlags>,
) -> LaunchResult<()> {
    let persist_stateflags = stateflags.is_some();
    let mut operations = WiiLaunchOperations {
        state,
        title_id,
        views,
        nandboot,
        stateflags,
    };
    launch_state::commit_and_launch(&mut operations, persist_stateflags)
}

fn try_launch_title_inner(state: &mut LaunchState, title_id: u64) -> LaunchResult<()> {
    if !state.initialized {
        return Err(LaunchError::NotInitialized);
    }
    let views = ticket_views(title_id)?;
    let nandboot = default_nandboot(title_id);
    execute_prepared_launch(state, title_id, views, nandboot, None)
}

pub fn launch_title_with_args(title_id: u64, launchcode: i32, args: &[&CStr]) -> i32 {
    raw_result(try_launch_title_with_args(title_id, launchcode, args))
}

pub fn try_launch_title_with_args(
    title_id: u64,
    launchcode: i32,
    args: &[&CStr],
) -> LaunchResult<()> {
    let mut state = lock_state()?;
    try_launch_title_with_args_inner(&mut state, title_id, launchcode, args)
}

fn try_launch_title_with_args_inner(
    state: &mut LaunchState,
    title_id: u64,
    launchcode: i32,
    args: &[&CStr],
) -> LaunchResult<()> {
    if !state.initialized {
        return Err(LaunchError::NotInitialized);
    }

    let title_buf = launch_state::format_title_id(title_id);
    let title_cstr = CStr::from_bytes_with_nul(&title_buf).map_err(|_| LaunchError::Internal)?;
    let mut argv = Vec::with_capacity(args.len() + 2);
    argv.push(title_cstr);
    argv.extend_from_slice(args);

    let views = ticket_views(title_id)?;
    let mut nandboot = state.nandboot;
    if unsafe { es::get_title_id(&mut nandboot.launcher) } < 0 {
        nandboot.launcher = RETURN_TO_MENU_TITLE_ID;
    }

    nandboot.titletype = if title_id == RETURN_TO_MENU_TITLE_ID {
        4
    } else {
        2
    };
    nandboot.apptype = 0x81;
    nandboot.launchcode = launchcode as u32;

    let mut stateflags = state.stateflags;
    stateflags.type_ = TYPE_RETURN;
    stateflags.returnto = RETURN_TO_ARGS;

    launch_state::set_args(&mut nandboot, &argv).map_err(|_| LaunchError::ArgsTooLarge)?;
    execute_prepared_launch(state, title_id, views, nandboot, Some(stateflags))
}

pub fn return_to_menu() -> i32 {
    raw_result(try_return_to_menu())
}

pub fn try_return_to_menu() -> LaunchResult<()> {
    let mut state = lock_state()?;
    try_return_to_inner(&mut state, RETURN_TO_MENU)
}

pub fn return_to_settings() -> i32 {
    raw_result(try_return_to_settings())
}

pub fn try_return_to_settings() -> LaunchResult<()> {
    let mut state = lock_state()?;
    try_return_to_inner(&mut state, RETURN_TO_SETTINGS)
}

fn try_return_to_inner(state: &mut LaunchState, destination: u8) -> LaunchResult<()> {
    if !state.initialized {
        return Err(LaunchError::NotInitialized);
    }
    let views = ticket_views(RETURN_TO_MENU_TITLE_ID)?;
    let nandboot = default_nandboot(RETURN_TO_MENU_TITLE_ID);
    let mut stateflags = state.stateflags;
    stateflags.type_ = TYPE_RETURN;
    stateflags.returnto = destination;
    execute_prepared_launch(
        state,
        RETURN_TO_MENU_TITLE_ID,
        views,
        nandboot,
        Some(stateflags),
    )
}

pub fn return_to_settings_page(page: &CStr) -> i32 {
    raw_result(try_return_to_settings_page(page))
}

pub fn try_return_to_settings_page(page: &CStr) -> LaunchResult<()> {
    let mut state = lock_state()?;
    if !state.initialized {
        return Err(LaunchError::NotInitialized);
    }
    try_launch_title_with_args_inner(&mut state, RETURN_TO_MENU_TITLE_ID, 1, &[page])
}

pub fn open_url(url: &CStr) -> i32 {
    raw_result(try_open_url(url))
}

pub fn try_open_url(url: &CStr) -> LaunchResult<()> {
    let mut state = lock_state()?;
    if !state.initialized {
        return Err(LaunchError::NotInitialized);
    }

    let mut titlecount = 0u32;
    status(unsafe { es::get_num_titles(&mut titlecount) }).map_err(|_| LaunchError::Internal)?;

    let title_bytes = (titlecount as usize)
        .checked_mul(size_of::<u64>())
        .ok_or(LaunchError::Internal)?;
    let storage_len = title_bytes.checked_add(31).ok_or(LaunchError::Internal)?;
    let mut title_storage = alloc::vec![0u8; storage_len];
    let titles_ptr = ((title_storage.as_mut_ptr() as usize + 31) & !31) as *mut u64;
    status(unsafe { es::get_titles(titles_ptr, titlecount) }).map_err(|_| LaunchError::Internal)?;
    let titles = unsafe { core::slice::from_raw_parts(titles_ptr, titlecount as usize) };

    let mut tid = 0u64;
    for &title in titles {
        if (title & URL_TITLE_MASK) == URL_TITLE_BASE {
            tid = title;
            break;
        }
    }
    if tid == 0 {
        return Err(LaunchError::InstallMissing);
    }

    let mut tmd_size = 0u32;
    if unsafe { es::get_stored_tmd_size(tid, &mut tmd_size) } < 0 {
        return Err(LaunchError::InstallMissing);
    }

    try_launch_title_with_args_inner(&mut state, tid, 0, &[url])
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn internal_layout_matches_c() {
        assert_eq!(core::mem::size_of::<NandBootInfo>(), 4128);
        assert_eq!(core::mem::align_of::<NandBootInfo>(), 8);
        assert_eq!(core::mem::size_of::<StateFlags>(), 32);
        assert_eq!(core::mem::align_of::<StateFlags>(), 4);
        assert_eq!(core::mem::align_of::<Aligned<NandBootInfo>>(), 32);
        assert_eq!(core::mem::align_of::<Aligned<[Tikview; 4]>>(), 32);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = initialize as fn() -> i32;
        let _ = launch_title as fn(u64) -> i32;
        let _ = launch_title_with_args as fn(u64, i32, &[&CStr]) -> i32;
        let _ = return_to_menu as fn() -> i32;
        let _ = return_to_settings as fn() -> i32;
        let _ = return_to_settings_page as fn(&CStr) -> i32;
        let _ = open_url as fn(&CStr) -> i32;
        let _ = try_initialize as fn() -> LaunchResult<()>;
        let _ = try_launch_title as fn(u64) -> LaunchResult<()>;
        let _ = try_launch_title_with_args as fn(u64, i32, &[&CStr]) -> LaunchResult<()>;
        let _ = try_return_to_menu as fn() -> LaunchResult<()>;
        let _ = try_return_to_settings as fn() -> LaunchResult<()>;
        let _ = try_return_to_settings_page as fn(&CStr) -> LaunchResult<()>;
        let _ = try_open_url as fn(&CStr) -> LaunchResult<()>;
    }

    #[test]
    fn typed_errors_round_trip_to_legacy_codes() {
        assert_eq!(LaunchError::NotInitialized.raw_code(), WII_ENOTINIT);
        assert_eq!(
            LaunchError::from_raw_code(WII_E2BIG),
            LaunchError::ArgsTooLarge
        );
        assert_eq!(raw_result(Err(LaunchError::InstallMissing)), WII_EINSTALL);
        assert_eq!(raw_result(Ok(())), 0);
    }
}
