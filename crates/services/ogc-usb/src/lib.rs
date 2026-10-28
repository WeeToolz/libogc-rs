#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

use ogc_lwp::sys as lwp;
pub use ogc_usb_sys as sys;
pub use sys::*;

pub use ogc_sys::{ErrorCode, Result};

fn status(status: i32) -> Result<()> {
    if status == USB_OK {
        Ok(())
    } else {
        Err(ErrorCode::new(status))
    }
}

pub fn initialize() -> i32 {
    sys::initialize()
}

pub fn try_initialize() -> Result<()> {
    status(initialize())
}

pub fn deinitialize() -> i32 {
    sys::deinitialize()
}

pub fn try_deinitialize() -> Result<()> {
    status(deinitialize())
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
pub unsafe fn open_device(device_id: i32, vid: u16, pid: u16, fd: *mut i32) -> i32 {
    unsafe { sys::open_device(device_id, vid, pid, fd) }
}

pub fn open_device_handle(device_id: i32, vid: u16, pid: u16) -> Result<i32> {
    let mut fd = -1;
    status(unsafe { open_device(device_id, vid, pid, &mut fd) }).map(|()| fd)
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
pub unsafe fn close_device(fd: *mut i32) -> i32 {
    unsafe { sys::close_device(fd) }
}

pub fn close_device_handle(fd: &mut i32) -> Result<()> {
    status(unsafe { close_device(fd) })
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
pub unsafe fn close_device_async(
    fd: *mut i32,
    cb: UsbCallback,
    usrdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::close_device_async(fd, cb, usrdata) }
}

/// # Safety
///
/// `fd` must be null or a valid writable descriptor pointer. `callback`, when
/// present, and `userdata` must remain valid for the async completion contract.
pub unsafe fn try_close_device_async(
    fd: *mut i32,
    callback: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> Result<()> {
    unsafe { status(close_device_async(fd, callback, userdata)) }
}

/// # Safety
///
/// `devdesc` must be null or a valid writable pointer.
pub unsafe fn get_device_description(fd: i32, devdesc: *mut UsbDevDesc) -> i32 {
    unsafe { sys::get_device_description(fd, devdesc) }
}

pub fn device_description(fd: i32) -> Result<UsbDevDesc> {
    let mut desc = UsbDevDesc::default();
    status(unsafe { get_device_description(fd, &mut desc) }).map(|()| desc)
}

/// # Safety
///
/// `udd` must be null or a valid writable pointer.
pub unsafe fn get_descriptors(fd: i32, udd: *mut UsbDevDesc) -> i32 {
    unsafe { sys::get_descriptors(fd, udd) }
}

pub fn descriptors(fd: i32) -> Result<UsbDevDesc> {
    let mut desc = UsbDevDesc::default();
    status(unsafe { get_descriptors(fd, &mut desc) }).map(|()| desc)
}

/// # Safety
///
/// `udd` must be a pointer previously populated by `get_descriptors`.
pub unsafe fn free_descriptors(udd: *mut UsbDevDesc) {
    unsafe { sys::free_descriptors(udd) }
}

/// # Safety
///
/// `data` must be null or a valid writable buffer of `size` bytes.
pub unsafe fn get_generic_descriptor(
    fd: i32,
    type_: u8,
    index: u8,
    interface: u8,
    data: *mut core::ffi::c_void,
    size: u32,
) -> i32 {
    unsafe { sys::get_generic_descriptor(fd, type_, index, interface, data, size) }
}

pub fn generic_descriptor(
    fd: i32,
    type_: u8,
    index: u8,
    interface: u8,
    data: &mut [u8],
) -> Result<()> {
    status(unsafe {
        get_generic_descriptor(
            fd,
            type_,
            index,
            interface,
            data.as_mut_ptr().cast(),
            data.len().min(u32::MAX as usize) as u32,
        )
    })
}

/// # Safety
///
/// `uhd` must be null or a valid writable pointer.
pub unsafe fn get_hid_descriptor(fd: i32, interface: u8, uhd: *mut UsbHidDesc, size: u32) -> i32 {
    unsafe { sys::get_hid_descriptor(fd, interface, uhd, size) }
}

pub fn hid_descriptor(fd: i32, interface: u8) -> Result<UsbHidDesc> {
    let mut desc = UsbHidDesc::default();
    status(unsafe {
        get_hid_descriptor(
            fd,
            interface,
            &mut desc,
            core::mem::size_of::<UsbHidDesc>() as u32,
        )
    })
    .map(|()| desc)
}

pub fn get_report_descriptor_size(fd: i32, interface: u8) -> i32 {
    sys::get_report_descriptor_size(fd, interface)
}

pub fn report_descriptor_size(fd: i32, interface: u8) -> Result<usize> {
    let size = get_report_descriptor_size(fd, interface);
    if size == USB_FAILED {
        Err(ErrorCode::new(USB_FAILED))
    } else {
        ogc_sys::result_from_status(size).map(|size| size as usize)
    }
}

/// # Safety
///
/// `data` must be null or a valid writable buffer of `size` bytes.
pub unsafe fn get_report_descriptor(
    fd: i32,
    interface: u8,
    data: *mut core::ffi::c_void,
    size: u16,
) -> i32 {
    unsafe { sys::get_report_descriptor(fd, interface, data, size) }
}

pub fn report_descriptor(fd: i32, interface: u8, data: &mut [u8]) -> Result<()> {
    status(unsafe {
        get_report_descriptor(
            fd,
            interface,
            data.as_mut_ptr().cast(),
            data.len().min(u16::MAX as usize) as u16,
        )
    })
}

/// # Safety
///
/// After successful registration, `cb` and `userdata` must remain valid until
/// one callback reports removal, close, watcher failure, or USB
/// deinitialization. The callback is one-shot.
pub unsafe fn device_removal_notify_async(
    fd: i32,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::device_removal_notify_async(fd, cb, userdata) }
}

/// # Safety
///
/// After successful registration, `cb` and `userdata` must remain valid until
/// the one-shot device-change callback runs or USB is deinitialized.
pub unsafe fn device_change_notify_async(
    interface_class: u8,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::device_change_notify_async(interface_class, cb, userdata) }
}

pub fn suspend_device(fd: i32) -> i32 {
    sys::suspend_device(fd)
}

pub fn try_suspend_device(fd: i32) -> Result<()> {
    status(suspend_device(fd))
}

pub fn resume_device(fd: i32) -> i32 {
    sys::resume_device(fd)
}

pub fn try_resume_device(fd: i32) -> Result<()> {
    status(resume_device(fd))
}

/// # Safety
///
/// Packet-size and data buffers must satisfy the USB transfer contract.
pub unsafe fn read_iso_msg(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::read_iso_msg(fd, b_endpoint, b_packets, rp_packet_sizes, rp_data) }
}

/// # Safety
///
/// Packet-size and data buffers must satisfy the USB transfer contract.
pub unsafe fn read_iso_msg_async(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut core::ffi::c_void,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe {
        sys::read_iso_msg_async(
            fd,
            b_endpoint,
            b_packets,
            rp_packet_sizes,
            rp_data,
            cb,
            userdata,
        )
    }
}

/// # Safety
///
/// `rp_data` must be a writable buffer of `w_length` bytes.
pub unsafe fn read_intr_msg(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::read_intr_msg(fd, b_endpoint, w_length, rp_data) }
}

/// # Safety
///
/// `rp_data` must be a writable buffer of `w_length` bytes.
pub unsafe fn read_intr_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::read_intr_msg_async(fd, b_endpoint, w_length, rp_data, cb, userdata) }
}

/// # Safety
///
/// `rp_data` must be a writable buffer of `w_length` bytes.
pub unsafe fn read_blk_msg(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::read_blk_msg(fd, b_endpoint, w_length, rp_data) }
}

/// # Safety
///
/// `rp_data` must be a writable buffer of `w_length` bytes.
pub unsafe fn read_blk_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::read_blk_msg_async(fd, b_endpoint, w_length, rp_data, cb, userdata) }
}

/// # Safety
///
/// `rp_data` must be a writable buffer of `w_length` bytes.
pub unsafe fn read_ctrl_msg(
    fd: i32,
    bm_request_type: u8,
    bm_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe {
        sys::read_ctrl_msg(
            fd,
            bm_request_type,
            bm_request,
            w_value,
            w_index,
            w_length,
            rp_data,
        )
    }
}

/// # Safety
///
/// Packet-size and data buffers must satisfy the USB transfer contract.
pub unsafe fn write_iso_msg(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::write_iso_msg(fd, b_endpoint, b_packets, rp_packet_sizes, rp_data) }
}

/// # Safety
///
/// Packet-size and data buffers must satisfy the USB transfer contract.
pub unsafe fn write_iso_msg_async(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut core::ffi::c_void,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe {
        sys::write_iso_msg_async(
            fd,
            b_endpoint,
            b_packets,
            rp_packet_sizes,
            rp_data,
            cb,
            userdata,
        )
    }
}

/// # Safety
///
/// `rp_data` must point to a readable buffer of `w_length` bytes. On PowerPC,
/// the pointer must be 32-byte aligned for nonzero transfers.
pub unsafe fn write_intr_msg(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::write_intr_msg(fd, b_endpoint, w_length, rp_data) }
}

/// # Safety
///
/// `rp_data` must point to a readable buffer of `w_length` bytes. On PowerPC,
/// the pointer must be 32-byte aligned for nonzero transfers and remain valid
/// until the completion callback runs.
pub unsafe fn write_intr_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::write_intr_msg_async(fd, b_endpoint, w_length, rp_data, cb, userdata) }
}

/// # Safety
///
/// `rp_data` must be a readable buffer of `w_length` bytes.
pub unsafe fn write_blk_msg(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::write_blk_msg(fd, b_endpoint, w_length, rp_data) }
}

/// # Safety
///
/// `rp_data` must be a readable buffer of `w_length` bytes.
pub unsafe fn write_blk_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
    cb: UsbCallback,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::write_blk_msg_async(fd, b_endpoint, w_length, rp_data, cb, userdata) }
}

/// # Safety
///
/// `rp_data` must be a readable buffer of `w_length` bytes.
pub unsafe fn write_ctrl_msg(
    fd: i32,
    bm_request_type: u8,
    bm_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe {
        sys::write_ctrl_msg(
            fd,
            bm_request_type,
            bm_request,
            w_value,
            w_index,
            w_length,
            rp_data,
        )
    }
}

/// # Safety
///
/// `configuration` must be null or a valid writable pointer.
pub unsafe fn get_configuration(fd: i32, configuration: *mut u8) -> i32 {
    unsafe { sys::get_configuration(fd, configuration) }
}

pub fn configuration(fd: i32) -> Result<u8> {
    let mut configuration = 0;
    status(unsafe { get_configuration(fd, &mut configuration) }).map(|()| configuration)
}

pub fn set_configuration(fd: i32, configuration: u8) -> i32 {
    sys::set_configuration(fd, configuration)
}

pub fn try_set_configuration(fd: i32, configuration: u8) -> Result<()> {
    status(set_configuration(fd, configuration))
}

pub fn set_alternative_interface(fd: i32, interface: u8, alternate_setting: u8) -> i32 {
    sys::set_alternative_interface(fd, interface, alternate_setting)
}

pub fn try_set_alternative_interface(fd: i32, interface: u8, alternate_setting: u8) -> Result<()> {
    status(set_alternative_interface(fd, interface, alternate_setting))
}

pub fn clear_halt(fd: i32, endpoint_address: u8) -> i32 {
    sys::clear_halt(fd, endpoint_address)
}

pub fn try_clear_halt(fd: i32, endpoint_address: u8) -> Result<()> {
    status(clear_halt(fd, endpoint_address))
}

/// # Safety
///
/// `descr_buffer` and `cnt_descr` must be valid writable pointers when non-null.
pub unsafe fn get_device_list(
    descr_buffer: *mut UsbDeviceEntry,
    num_descr: u8,
    interface_class: u8,
    cnt_descr: *mut u8,
) -> i32 {
    unsafe { sys::get_device_list(descr_buffer, num_descr, interface_class, cnt_descr) }
}

pub fn device_list(entries: &mut [UsbDeviceEntry], interface_class: u8) -> Result<usize> {
    let mut count = 0;
    status(unsafe {
        get_device_list(
            entries.as_mut_ptr(),
            entries.len().min(u8::MAX as usize) as u8,
            interface_class,
            &mut count,
        )
    })
    .map(|()| count as usize)
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer.
pub unsafe fn get_ascii_string(
    fd: i32,
    b_index: u8,
    w_lang_id: u16,
    w_length: u16,
    rp_data: *mut core::ffi::c_void,
) -> i32 {
    unsafe { sys::get_ascii_string(fd, b_index, w_lang_id, w_length, rp_data) }
}

pub fn ascii_string(fd: i32, index: u8, lang_id: u16, data: &mut [u8]) -> Result<()> {
    status(unsafe {
        get_ascii_string(
            fd,
            index,
            lang_id,
            data.len().min(u16::MAX as usize) as u16,
            data.as_mut_ptr().cast(),
        )
    })
}

pub mod mouse {
    use super::*;

    const MOUSE_THREAD_STACKSIZE: usize = 1024 * 4;
    const MOUSE_THREAD_PRIO: u8 = 65;
    const MOUSE_MAX_DATA: usize = 32;
    const DEVLIST_MAXSIZE: usize = 8;
    const USB_REQTYPE_INTERFACE_SET: u8 = 0x21;
    const USB_REQ_SETPROTOCOL: u8 = 0x0b;

    #[repr(C)]
    #[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
    pub struct MouseEvent {
        pub button: u8,
        pub rx: i32,
        pub ry: i32,
        pub rz: i32,
    }

    #[derive(Copy, Clone)]
    struct MouseDevice {
        connected: bool,
        has_wheel: bool,
        fd: i32,
        configuration: u8,
        interface: u32,
        alt_interface: u32,
        ep: u8,
        ep_size: u32,
    }

    struct MouseQueue {
        events: alloc::collections::VecDeque<MouseEvent>,
    }

    #[repr(align(32))]
    #[derive(Copy, Clone)]
    struct MouseTransfer {
        raw: [i8; MOUSE_MAX_DATA],
        event: MouseEvent,
    }

    #[repr(align(8))]
    struct MouseStack([u8; MOUSE_THREAD_STACKSIZE]);

    struct MouseState {
        inited: bool,
        thread_running: bool,
        thread_quit: bool,
        thread: lwp::LwpThread,
        sem: lwp::Semaphore,
        device: MouseDevice,
        queue: MouseQueue,
    }

    impl MouseEvent {
        const ZERO: Self = Self {
            button: 0,
            rx: 0,
            ry: 0,
            rz: 0,
        };
    }

    impl MouseDevice {
        const fn new() -> Self {
            Self {
                connected: false,
                has_wheel: false,
                fd: -1,
                configuration: 0,
                interface: 0,
                alt_interface: 0,
                ep: 0,
                ep_size: 0,
            }
        }
    }

    impl MouseQueue {
        const fn new() -> Self {
            Self {
                events: alloc::collections::VecDeque::new(),
            }
        }

        fn push(&mut self, event: MouseEvent) {
            self.events.push_back(event);
        }

        fn pop(&mut self) -> Option<MouseEvent> {
            self.events.pop_front()
        }

        fn clear(&mut self) -> usize {
            let len = self.events.len();
            self.events.clear();
            len
        }
    }

    impl MouseTransfer {
        const fn new() -> Self {
            Self {
                raw: [0; MOUSE_MAX_DATA],
                event: MouseEvent::ZERO,
            }
        }
    }

    impl MouseState {
        const fn new() -> Self {
            Self {
                inited: false,
                thread_running: false,
                thread_quit: false,
                thread: lwp::LWP_THREAD_NULL,
                sem: lwp::LWP_SEM_NULL,
                device: MouseDevice::new(),
                queue: MouseQueue::new(),
            }
        }
    }

    static MOUSE_LOCK: AtomicBool = AtomicBool::new(false);
    static mut MOUSE_STATE: MouseState = MouseState::new();
    static mut MOUSE_STACK: MouseStack = MouseStack([0; MOUSE_THREAD_STACKSIZE]);

    fn lock_mouse() {
        while MOUSE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
    }

    fn unlock_mouse() {
        MOUSE_LOCK.store(false, Ordering::Release);
    }

    fn with_state<R>(f: impl FnOnce(&mut MouseState) -> R) -> R {
        lock_mouse();
        let state = core::ptr::addr_of_mut!(MOUSE_STATE);
        let out = unsafe { f(&mut *state) };
        unlock_mouse();
        out
    }

    fn post_sem_if_init() {
        let sem = with_state(|state| state.sem);
        if sem != lwp::LWP_SEM_NULL {
            let _ = lwp::sem_post(sem);
        }
    }

    fn finish_deinit(state: &mut MouseState) -> i32 {
        state.inited = false;
        1
    }

    unsafe fn mouse_close_with(
        state: &mut MouseState,
        close_device_fn: impl FnOnce(*mut i32) -> i32,
    ) -> i32 {
        if state.device.fd != -1 {
            // C USBMouse_Close ignores USB_CloseDevice's result and clears the
            // handle even when IOS reports an error.
            let _ = close_device_fn(core::ptr::addr_of_mut!(state.device.fd));
            state.device.fd = -1;
        }
        state.device.connected = false;
        USB_OK
    }

    unsafe fn mouse_close(state: &mut MouseState) -> i32 {
        unsafe { mouse_close_with(state, |fd| close_device(fd)) }
    }

    fn cleanup_mouse_resources(
        state: &mut MouseState,
        close_device_fn: impl FnOnce(*mut i32) -> i32,
        destroy_sem_fn: impl FnOnce(lwp::Semaphore) -> i32,
    ) -> i32 {
        state.thread_running = false;
        // BUG FIX: MOUSE_Deinit continues cleanup and reports success when USB
        // close or semaphore destruction fails. Both handles are cleared.
        unsafe {
            let _ = mouse_close_with(state, close_device_fn);
        }
        state.queue.clear();
        if state.sem != lwp::LWP_SEM_NULL {
            let _ = destroy_sem_fn(state.sem);
            state.sem = lwp::LWP_SEM_NULL;
        }
        finish_deinit(state)
    }

    fn deinit_with(
        state: &mut MouseState,
        post_sem_fn: impl FnOnce(lwp::Semaphore) -> i32,
        join_thread_fn: impl FnOnce(lwp::LwpThread) -> i32,
        close_device_fn: impl FnOnce(*mut i32) -> i32,
        destroy_sem_fn: impl FnOnce(lwp::Semaphore) -> i32,
    ) -> i32 {
        if !state.inited {
            return 1;
        }

        if state.thread_running {
            state.thread_quit = true;
            if state.sem != lwp::LWP_SEM_NULL {
                // C ignores post and join errors and continues cleanup.
                let _ = post_sem_fn(state.sem);
            }
            if state.thread != lwp::LWP_THREAD_NULL {
                let _ = join_thread_fn(state.thread);
            }
        }

        cleanup_mouse_resources(state, close_device_fn, destroy_sem_fn)
    }

    unsafe extern "C" fn mouse_event_cb(result: i32, userdata: *mut c_void) -> i32 {
        unsafe {
            let transfer = &mut *(userdata as *mut MouseTransfer);

            with_state(|state| {
                if result >= 3 {
                    transfer.event.button = transfer.raw[0] as u8;
                    transfer.event.rx = transfer.raw[1] as i32;
                    transfer.event.ry = transfer.raw[2] as i32;
                    if state.device.has_wheel {
                        if result < 4 || transfer.raw[3] < -1 || transfer.raw[3] > 1 {
                            state.device.has_wheel = false;
                            transfer.event.rz = 0;
                        } else {
                            transfer.event.rz = transfer.raw[3] as i32;
                        }
                    } else {
                        transfer.event.rz = 0;
                    }
                } else {
                    state.device.connected = false;
                }
            });

            post_sem_if_init();
            0
        }
    }

    unsafe extern "C" fn disconnect_cb(_result: i32, _userdata: *mut c_void) -> i32 {
        with_state(|state| {
            state.device.connected = false;
        });
        post_sem_if_init();
        1
    }

    unsafe extern "C" fn device_change_cb(_result: i32, _userdata: *mut c_void) -> i32 {
        post_sem_if_init();
        1
    }

    unsafe fn mouse_open(state: &mut MouseState) -> i32 {
        unsafe {
            let mut buffer = [UsbDeviceEntry::default(); DEVLIST_MAXSIZE];
            let mut device_count: u8 = 0;
            if get_device_list(
                buffer.as_mut_ptr(),
                DEVLIST_MAXSIZE as u8,
                USB_CLASS_HID,
                &mut device_count,
            ) < 0
            {
                return -2;
            }

            let mut found = false;
            for entry in buffer.iter().take(device_count as usize) {
                if entry.vid == 0 || entry.pid == 0 {
                    continue;
                }

                let mut fd = 0;
                if open_device(entry.device_id, entry.vid, entry.pid, &mut fd) < 0 {
                    continue;
                }

                let mut udd = UsbDevDesc::default();
                if get_descriptors(fd, &mut udd) < 0 {
                    let _ = close_device(&mut fd);
                    continue;
                }

                if udd.configurations.is_null() {
                    free_descriptors(&mut udd);
                    let _ = close_device(&mut fd);
                    continue;
                }

                for conf_idx in 0..udd.b_num_configurations as usize {
                    let ucd = &*udd.configurations.add(conf_idx);
                    if ucd.interfaces.is_null() {
                        continue;
                    }

                    for interface_idx in 0..ucd.b_num_interfaces as usize {
                        let uid = &*ucd.interfaces.add(interface_idx);
                        if uid.b_interface_class != USB_CLASS_HID
                            || uid.b_interface_sub_class != USB_SUBCLASS_BOOT
                            || uid.b_interface_protocol != USB_PROTOCOL_MOUSE
                        {
                            continue;
                        }

                        if uid.endpoints.is_null() {
                            continue;
                        }

                        for ep_idx in 0..uid.b_num_endpoints as usize {
                            let ued = &*uid.endpoints.add(ep_idx);
                            if ued.bm_attributes != USB_ENDPOINT_INTERRUPT {
                                continue;
                            }
                            if (ued.b_endpoint_address & USB_ENDPOINT_IN) == 0 {
                                continue;
                            }
                            if ued.w_max_packet_size as usize > MOUSE_MAX_DATA {
                                continue;
                            }

                            state.device.fd = fd;
                            state.device.configuration = ucd.b_configuration_value;
                            state.device.interface = uid.b_interface_number as u32;
                            state.device.alt_interface = uid.b_alternate_setting as u32;
                            state.device.ep = ued.b_endpoint_address;
                            state.device.ep_size = ued.w_max_packet_size as u32;
                            found = true;
                            break;
                        }

                        if found {
                            break;
                        }
                    }

                    if found {
                        break;
                    }
                }

                free_descriptors(&mut udd);

                if found {
                    break;
                }

                let _ = close_device(&mut fd);
            }

            if !found {
                return -3;
            }

            if device_removal_notify_async(
                state.device.fd,
                Some(disconnect_cb),
                core::ptr::null_mut(),
            ) < 0
            {
                mouse_close(state);
                return -8;
            }

            let _ = write_ctrl_msg(
                state.device.fd,
                USB_REQTYPE_INTERFACE_SET,
                USB_REQ_SETPROTOCOL,
                0,
                state.device.interface as u16,
                0,
                core::ptr::null_mut(),
            );

            state.device.has_wheel = true;
            state.device.connected = true;
            1
        }
    }

    pub fn is_connected() -> bool {
        with_state(|state| state.device.connected)
    }

    unsafe extern "C" fn mouse_thread(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let _ = arg;
            let mut transfer = MouseTransfer::new();

            loop {
                if with_state(|state| state.thread_quit) {
                    break;
                }

                if !is_connected() {
                    with_state(|state| {
                        mouse_close(state);
                    });
                    if with_state(|state| mouse_open(state)) < 0 {
                        let _ = device_change_notify_async(
                            USB_CLASS_HID,
                            Some(device_change_cb),
                            core::ptr::null_mut(),
                        );
                        let sem = with_state(|state| state.sem);
                        if sem != lwp::LWP_SEM_NULL {
                            let _ = lwp::sem_wait(sem);
                        }
                        continue;
                    }
                }

                let (fd, ep, ep_size) = with_state(|state| {
                    (
                        state.device.fd,
                        state.device.ep,
                        state.device.ep_size as u16,
                    )
                });

                let rc = read_intr_msg_async(
                    fd,
                    ep,
                    ep_size,
                    transfer.raw.as_mut_ptr().cast(),
                    Some(mouse_event_cb),
                    (&mut transfer as *mut MouseTransfer).cast(),
                );
                if rc < 0 {
                    break;
                }

                let sem = with_state(|state| state.sem);
                if sem != lwp::LWP_SEM_NULL {
                    let _ = lwp::sem_wait(sem);
                }

                with_state(|state| {
                    state.queue.push(transfer.event);
                });
                transfer.event = MouseEvent::ZERO;
            }

            core::ptr::null_mut()
        }
    }

    pub fn init() -> i32 {
        if with_state(|state| state.inited) {
            return 0;
        }

        if initialize() != USB_OK {
            return -1;
        }

        let sem_rc = with_state(|state| {
            if state.sem != lwp::LWP_SEM_NULL {
                return 0;
            }
            let mut sem = lwp::LWP_SEM_NULL;
            // SAFETY: `sem` is a valid unique output reference.
            let rc = unsafe { lwp::sem_init(&mut sem, 0, 1) };
            if rc == 0 {
                state.sem = sem;
            }
            rc
        });
        if sem_rc != 0 {
            let usb_result = deinitialize();
            if usb_result != USB_OK {
                return usb_result;
            }
            return -2;
        }

        let thread_rc = with_state(|state| {
            if state.thread_running {
                state.inited = true;
                return 0;
            }

            state.thread_quit = false;
            let mut thread = lwp::LWP_THREAD_NULL;
            let rc = unsafe {
                lwp::create_thread(
                    &mut thread,
                    Some(mouse_thread),
                    core::ptr::null_mut(),
                    core::ptr::addr_of_mut!(MOUSE_STACK.0).cast::<u8>().cast(),
                    MOUSE_THREAD_STACKSIZE as u32,
                    MOUSE_THREAD_PRIO,
                )
            };
            if rc == 0 {
                state.thread = thread;
                state.thread_running = true;
                state.inited = true;
            }
            rc
        });
        if thread_rc != 0 {
            let destroy_rc = with_state(|state| {
                if state.sem != lwp::LWP_SEM_NULL {
                    let rc = lwp::sem_destroy(state.sem);
                    if rc == lwp::LWP_SUCCESSFUL {
                        state.sem = lwp::LWP_SEM_NULL;
                    }
                    return rc;
                }
                lwp::LWP_SUCCESSFUL
            });
            let usb_rc = deinitialize();
            if destroy_rc != lwp::LWP_SUCCESSFUL || usb_rc != USB_OK {
                return -2;
            }
            return -6;
        }

        0
    }

    pub fn deinit() -> i32 {
        with_state(|state| {
            deinit_with(
                state,
                lwp::sem_post,
                |thread| {
                    let mut value = core::ptr::null_mut();
                    // SAFETY: `value` is a valid unique output for the synchronous join.
                    unsafe { lwp::join_thread(thread, &mut value) }
                },
                |fd| unsafe { close_device(fd) },
                lwp::sem_destroy,
            )
        })
    }

    pub fn get_event(event: Option<&mut MouseEvent>) -> i32 {
        let popped = with_state(|state| state.queue.pop());
        match popped {
            Some(ev) => {
                if let Some(out) = event {
                    *out = ev;
                }
                1
            }
            None => 0,
        }
    }

    pub fn flush_events() -> i32 {
        with_state(|state| state.queue.clear() as i32)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn mouse_queue_does_not_drop_events_after_legacy_capacity() {
            let mut queue = MouseQueue::new();
            for index in 0..96 {
                queue.push(MouseEvent {
                    button: index as u8,
                    rx: index,
                    ry: -index,
                    rz: 0,
                });
            }
            for index in 0..96 {
                assert_eq!(
                    queue.pop(),
                    Some(MouseEvent {
                        button: index as u8,
                        rx: index,
                        ry: -index,
                        rz: 0,
                    })
                );
            }
            assert_eq!(queue.pop(), None);
        }

        #[test]
        fn successful_mouse_cleanup_clears_mouse_initialized_state() {
            let mut state = MouseState::new();
            state.inited = true;

            assert_eq!(finish_deinit(&mut state), 1);
            assert!(!state.inited);
        }

        #[test]
        fn mouse_cleanup_continues_after_close_error() {
            let mut state = MouseState::new();
            state.inited = true;
            state.thread_running = true;
            state.device.fd = 7;
            state.device.connected = true;
            state.sem = 11;
            state.queue.push(MouseEvent {
                button: 1,
                rx: 2,
                ry: 3,
                rz: 0,
            });
            let mut close_called = false;
            let mut destroy_called = false;

            let result = cleanup_mouse_resources(
                &mut state,
                |fd| {
                    close_called = true;
                    assert_eq!(unsafe { *fd }, 7);
                    -9
                },
                |sem| {
                    destroy_called = true;
                    assert_eq!(sem, 11);
                    lwp::LWP_SUCCESSFUL
                },
            );

            assert_eq!(result, 1);
            assert!(close_called);
            assert!(destroy_called);
            assert_eq!(state.device.fd, -1);
            assert!(!state.device.connected);
            assert_eq!(state.sem, lwp::LWP_SEM_NULL);
            assert_eq!(state.queue.pop(), None);
            assert!(!state.thread_running);
            assert!(!state.inited);
        }

        #[test]
        fn mouse_cleanup_clears_semaphore_after_destroy_error() {
            let mut state = MouseState::new();
            state.inited = true;
            state.device.fd = -1;
            state.sem = 12;
            state.queue.push(MouseEvent {
                button: 4,
                rx: 5,
                ry: 6,
                rz: 1,
            });
            let mut destroy_called = false;

            let result = cleanup_mouse_resources(
                &mut state,
                |_| panic!("close must not run for a closed mouse descriptor"),
                |sem| {
                    destroy_called = true;
                    assert_eq!(sem, 12);
                    -8
                },
            );

            assert_eq!(result, 1);
            assert!(destroy_called);
            assert_eq!(state.device.fd, -1);
            assert_eq!(state.sem, lwp::LWP_SEM_NULL);
            assert_eq!(state.queue.pop(), None);
            assert!(!state.inited);
        }

        #[test]
        fn mouse_deinit_continues_after_post_and_join_errors() {
            let mut state = MouseState::new();
            state.inited = true;
            state.thread_running = true;
            state.thread = 21;
            state.sem = 22;
            let mut post_called = false;
            let mut join_called = false;
            let result = deinit_with(
                &mut state,
                |sem| {
                    post_called = true;
                    assert_eq!(sem, 22);
                    -5
                },
                |thread| {
                    join_called = true;
                    assert_eq!(thread, 21);
                    -6
                },
                |_| panic!("no mouse descriptor is open"),
                |sem| {
                    assert_eq!(sem, 22);
                    lwp::LWP_SUCCESSFUL
                },
            );

            assert_eq!(result, 1);
            assert!(post_called);
            assert!(join_called);
            assert!(!state.thread_running);
            assert_eq!(state.sem, lwp::LWP_SEM_NULL);
            assert!(!state.inited);
        }
    }
}

pub use mouse::MouseEvent;

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn result_helpers_wrap_usb_status_codes() -> Result<()> {
        assert_eq!(try_initialize(), Ok(()));
        let mut fd = open_device_handle(7, 0x1234, 0x5678)?;
        assert!(fd > 0);

        assert_eq!(try_set_configuration(fd, 3), Ok(()));
        assert_eq!(configuration(fd), Ok(3));
        assert_eq!(report_descriptor_size(fd, 0), Ok(64));

        let mut entries = [UsbDeviceEntry::default(); 2];
        let count = device_list(&mut entries, USB_CLASS_HID)?;
        assert_eq!(count, 1);
        assert_eq!(entries[0].vid, 0);

        assert_eq!(close_device_handle(&mut fd), Ok(()));
        assert_eq!(fd, -1);
        assert_eq!(try_deinitialize(), Ok(()));
        Ok(())
    }

    #[test]
    fn result_helpers_report_pointer_errors() {
        assert_eq!(
            try_close_device_async(core::ptr::null_mut(), None, core::ptr::null_mut()),
            Err(ErrorCode::new(USB_FAILED))
        );
    }
}
