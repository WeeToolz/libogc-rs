#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};

mod mailbox;

#[cfg(target_arch = "powerpc")]
use ogc_irq::{self as irq, IRQ_DSP_DSP};

pub const DSPTASK_INIT: u32 = 0;
pub const DSPTASK_RUN: u32 = 1;
pub const DSPTASK_YIELD: u32 = 2;
pub const DSPTASK_DONE: u32 = 3;

pub const DSPTASK_CLEARALL: u32 = 0x0000_0000;
pub const DSPTASK_ATTACH: u32 = 0x0000_0001;
pub const DSPTASK_CANCEL: u32 = 0x0000_0002;

const DSPCR_DSPRESET: u16 = 0x0800;
const DSPCR_DSPDMA: u16 = 0x0200;
const DSPCR_DSPINT: u16 = 0x0080;
const DSPCR_ARINT: u16 = 0x0020;
const DSPCR_AIINT: u16 = 0x0008;
const DSPCR_HALT: u16 = 0x0004;
const DSPCR_PIINT: u16 = 0x0002;
const DSPCR_RES: u16 = 0x0001;

// BUG FIX: C sends task memory pointers to the DSP mailbox unchanged. Clearing
// the top address bits changes the memory address received by the DSP.
#[inline]
fn task_mailbox_address(address: *mut c_void) -> u32 {
    address as usize as u32
}

/// A C-compatible `volatile u32` field.
///
/// The cell preserves the C field's size and alignment while making every
/// access use volatile loads or stores.
#[repr(transparent)]
pub struct VolatileU32(UnsafeCell<u32>);

impl VolatileU32 {
    pub const fn new(value: u32) -> Self {
        Self(UnsafeCell::new(value))
    }

    pub fn read(&self) -> u32 {
        unsafe { read_volatile(self.0.get()) }
    }

    pub fn write(&self, value: u32) {
        unsafe { write_volatile(self.0.get(), value) }
    }
}

impl Default for VolatileU32 {
    fn default() -> Self {
        Self::new(0)
    }
}

#[repr(C)]
#[derive(Default)]
pub struct DspTask {
    // BUG FIX: C declares these task fields volatile. Keep the C memory layout
    // and force volatile access at every Rust read and write.
    pub state: VolatileU32,
    pub prio: VolatileU32,
    pub flags: VolatileU32,
    pub iram_maddr: *mut c_void,
    pub iram_len: u32,
    pub iram_addr: u32,
    pub dram_maddr: *mut c_void,
    pub dram_len: u32,
    pub dram_addr: u32,
    pub init_vec: u16,
    pub resume_vec: u16,
    pub init_cb: DspTaskCallback,
    pub res_cb: DspTaskCallback,
    pub done_cb: DspTaskCallback,
    pub req_cb: DspTaskCallback,
    pub next: *mut DspTask,
    pub prev: *mut DspTask,
}

pub type DsptaskT = DspTask;
pub type DspTaskCallback = Option<unsafe extern "C" fn(task: *mut DspTask)>;
pub type DspCallback = Option<unsafe extern "C" fn()>;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct DspState {
    inited: bool,
    curr: *mut DspTask,
    first: *mut DspTask,
    last: *mut DspTask,
    rude: *mut DspTask,
    rude_pending: bool,
    intcb: DspCallback,
}

static mut DSP_STATE: DspState = DspState {
    inited: false,
    curr: core::ptr::null_mut(),
    first: core::ptr::null_mut(),
    last: core::ptr::null_mut(),
    rude: core::ptr::null_mut(),
    rude_pending: false,
    intcb: None,
};

#[inline(always)]
fn dsp_registers_available() -> bool {
    cfg!(target_arch = "powerpc")
}

#[cfg(target_arch = "powerpc")]
const DSP_REG_BASE: usize = 0xCC00_5000;

#[cfg(not(target_arch = "powerpc"))]
static mut DSP_REGS: [u16; 6] = [0; 6];

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn reg_ptr(index: usize) -> *mut u16 {
    (DSP_REG_BASE as *mut u16).wrapping_add(index)
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
fn reg_ptr(index: usize) -> *mut u16 {
    core::ptr::addr_of_mut!(DSP_REGS)
        .cast::<u16>()
        .wrapping_add(index)
}

#[inline(always)]
unsafe fn read(index: usize) -> u16 {
    unsafe { read_volatile(reg_ptr(index)) }
}

#[inline(always)]
unsafe fn write(index: usize, value: u16) {
    unsafe {
        write_volatile(reg_ptr(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_mfmsr() -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) value, options(nostack));
    }
    value
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_mtmsr(value: u32) {
    unsafe {
        core::arch::asm!("mtmsr {0}", in(reg) value, options(nostack));
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn lock_by_msr() -> u32 {
    unsafe {
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !(1 << 15));
        msr & (1 << 15)
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn lock_by_msr() -> u32 {
    0
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn unlock_by_msr(state: u32) {
    unsafe {
        let msr = ppc_mfmsr() & !(1 << 15);
        ppc_mtmsr(msr | state);
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn unlock_by_msr(_state: u32) {}

#[inline(always)]
unsafe fn mail_to_ready() -> bool {
    unsafe { (read(0) >> 15) & 1 != 0 }
}

#[inline(always)]
unsafe fn mail_from_ready() -> bool {
    unsafe { (read(2) >> 15) & 1 != 0 }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn irq_handler(_irq: u32, _ctx: *mut c_void) {
    unsafe {
        let reg5 = read(5);
        write(5, (reg5 & !(DSPCR_AIINT | DSPCR_ARINT)) | DSPCR_DSPINT);
        if let Some(cb) = DSP_STATE.intcb {
            cb();
        }
    }
}

unsafe fn insert_task(task: *mut DspTask) {
    unsafe {
        if DSP_STATE.first.is_null() {
            DSP_STATE.curr = task;
            DSP_STATE.last = task;
            DSP_STATE.first = task;
            {
                (*task).next = core::ptr::null_mut();
                (*task).prev = core::ptr::null_mut();
            }
            return;
        }

        let mut current = DSP_STATE.first;
        while !current.is_null() {
            {
                if (*task).prio.read() < (*current).prio.read() {
                    (*task).prev = (*current).prev;
                    (*current).prev = task;
                    (*task).next = current;
                    if (*task).prev.is_null() {
                        DSP_STATE.first = task;
                    } else {
                        (*(*task).prev).next = task;
                    }
                    return;
                }
                current = (*current).next;
            }
        }

        {
            (*DSP_STATE.last).next = task;
            (*task).next = core::ptr::null_mut();
            (*task).prev = DSP_STATE.last;
        }
        DSP_STATE.last = task;
    }
}

unsafe fn remove_task(task: *mut DspTask) {
    unsafe {
        {
            (*task).flags.write(DSPTASK_CLEARALL);
            (*task).state.write(DSPTASK_DONE);
        }

        if DSP_STATE.first == task {
            {
                if !(*task).next.is_null() {
                    DSP_STATE.first = (*task).next;
                    (*DSP_STATE.first).prev = core::ptr::null_mut();
                    return;
                }
            }
            DSP_STATE.curr = core::ptr::null_mut();
            DSP_STATE.last = core::ptr::null_mut();
            DSP_STATE.first = core::ptr::null_mut();
            return;
        }

        if DSP_STATE.last == task {
            {
                DSP_STATE.last = (*task).prev;
                (*DSP_STATE.last).next = core::ptr::null_mut();
            }
            DSP_STATE.curr = DSP_STATE.first;
            return;
        }

        {
            DSP_STATE.curr = (*DSP_STATE.curr).next;
        }
    }
}

unsafe fn exec_task(exec: *mut DspTask, hire: *mut DspTask) {
    if exec.is_null() {
        unsafe {
            send_mail_to(0);
            while mail_to_ready() {}
            send_mail_to(0);
            while mail_to_ready() {}
            send_mail_to(0);
            while mail_to_ready() {}
        }
    } else {
        unsafe {
            send_mail_to(task_mailbox_address((*exec).dram_maddr));
            while mail_to_ready() {}
            send_mail_to((*exec).dram_len);
            while mail_to_ready() {}
            send_mail_to((*exec).dram_addr);
            while mail_to_ready() {}
        }
    }

    unsafe {
        send_mail_to(task_mailbox_address((*hire).iram_maddr));
        while mail_to_ready() {}
        send_mail_to((*hire).iram_len);
        while mail_to_ready() {}
        send_mail_to((*hire).iram_addr);
        while mail_to_ready() {}
        if (*hire).state.read() == DSPTASK_INIT {
            send_mail_to((*hire).init_vec as u32);
            while mail_to_ready() {}
            send_mail_to(0);
            while mail_to_ready() {}
            send_mail_to(0);
            while mail_to_ready() {}
            send_mail_to(0);
            while mail_to_ready() {}
            return;
        }

        send_mail_to((*hire).resume_vec as u32);
        while mail_to_ready() {}
        send_mail_to(task_mailbox_address((*hire).dram_maddr));
        while mail_to_ready() {}
        send_mail_to((*hire).dram_len);
        while mail_to_ready() {}
        send_mail_to((*hire).dram_addr);
        while mail_to_ready() {}
    }
}

unsafe fn boot_task(task: *mut DspTask) {
    unsafe {
        while !mail_from_ready() {
            core::hint::spin_loop();
        }
        let _mail = read_mail_from();

        {
            send_mail_to(0x80F3_A001);
            while mail_to_ready() {}
            send_mail_to(task_mailbox_address((*task).iram_maddr));
            while mail_to_ready() {}
            send_mail_to(0x80F3_C002);
            while mail_to_ready() {}
            send_mail_to((*task).iram_addr & 0xffff);
            while mail_to_ready() {}
            send_mail_to(0x80F3_A002);
            while mail_to_ready() {}
            send_mail_to((*task).iram_len);
            while mail_to_ready() {}
            send_mail_to(0x80F3_B002);
            while mail_to_ready() {}
            send_mail_to(0);
            while mail_to_ready() {}
            send_mail_to(0x80F3_D001);
            while mail_to_ready() {}
            send_mail_to((*task).init_vec as u32);
            while mail_to_ready() {}
        }
    }
}

unsafe extern "C" fn def_taskcb() {
    unsafe {
        while !mail_from_ready() {
            core::hint::spin_loop();
        }

        if DSP_STATE.curr.is_null() {
            return;
        }

        let mut mail = read_mail_from();
        if !DSP_STATE.curr.is_null()
            && { ((*DSP_STATE.curr).flags.read() & DSPTASK_CANCEL) != 0 }
            && mail == 0xDCD1_0002
        {
            mail = 0xDCD1_0003;
        }

        match mail {
            0xDCD1_0000 => {
                (*DSP_STATE.curr).state.write(DSPTASK_RUN);
                if let Some(cb) = (*DSP_STATE.curr).init_cb {
                    cb(DSP_STATE.curr);
                }
            }
            0xDCD1_0001 => {
                (*DSP_STATE.curr).state.write(DSPTASK_RUN);
                if let Some(cb) = (*DSP_STATE.curr).res_cb {
                    cb(DSP_STATE.curr);
                }
            }
            0xDCD1_0002 => {
                if DSP_STATE.rude_pending {
                    if DSP_STATE.rude == DSP_STATE.curr {
                        send_mail_to(0xCDD1_0003);
                        while mail_to_ready() {}
                        DSP_STATE.rude = core::ptr::null_mut();
                        DSP_STATE.rude_pending = false;
                        if let Some(cb) = (*DSP_STATE.curr).res_cb {
                            cb(DSP_STATE.curr);
                        }
                    } else {
                        send_mail_to(0xCDD1_0001);
                        while mail_to_ready() {}
                        exec_task(DSP_STATE.curr, DSP_STATE.rude);
                        (*DSP_STATE.curr).state.write(DSPTASK_YIELD);
                        DSP_STATE.curr = DSP_STATE.rude;
                        DSP_STATE.rude = core::ptr::null_mut();
                        DSP_STATE.rude_pending = false;
                    }
                } else if (*DSP_STATE.curr).next.is_null() {
                    if DSP_STATE.first == DSP_STATE.curr {
                        send_mail_to(0xCDD1_0003);
                        while mail_to_ready() {}
                        if let Some(cb) = (*DSP_STATE.curr).res_cb {
                            cb(DSP_STATE.curr);
                        }
                    } else {
                        send_mail_to(0xCDD1_0001);
                        while mail_to_ready() {}
                        exec_task(DSP_STATE.curr, DSP_STATE.first);
                        (*DSP_STATE.curr).state.write(DSPTASK_YIELD);
                        DSP_STATE.curr = DSP_STATE.first;
                    }
                } else {
                    send_mail_to(0xCDD1_0001);
                    while mail_to_ready() {}
                    exec_task(DSP_STATE.curr, (*DSP_STATE.curr).next);
                    (*DSP_STATE.curr).state.write(DSPTASK_YIELD);
                    DSP_STATE.curr = (*DSP_STATE.curr).next;
                }
            }
            0xDCD1_0003 => {
                if DSP_STATE.rude_pending {
                    if let Some(cb) = (*DSP_STATE.curr).done_cb {
                        cb(DSP_STATE.curr);
                    }
                    send_mail_to(0xCDD1_0001);
                    while mail_to_ready() {}
                    exec_task(core::ptr::null_mut(), DSP_STATE.rude);
                    remove_task(DSP_STATE.curr);
                    DSP_STATE.curr = DSP_STATE.rude;
                    DSP_STATE.rude_pending = false;
                    DSP_STATE.rude = core::ptr::null_mut();
                } else if (*DSP_STATE.curr).next.is_null() {
                    if DSP_STATE.first == DSP_STATE.curr {
                        if let Some(cb) = (*DSP_STATE.curr).done_cb {
                            cb(DSP_STATE.curr);
                        }
                        send_mail_to(0xCDD1_0002);
                        while mail_to_ready() {}
                        (*DSP_STATE.curr).state.write(DSPTASK_DONE);
                        remove_task(DSP_STATE.curr);
                    }
                } else {
                    if let Some(cb) = (*DSP_STATE.curr).done_cb {
                        cb(DSP_STATE.curr);
                    }
                    send_mail_to(0xCDD1_0001);
                    while mail_to_ready() {}
                    (*DSP_STATE.curr).state.write(DSPTASK_DONE);
                    exec_task(core::ptr::null_mut(), DSP_STATE.first);
                    DSP_STATE.curr = DSP_STATE.first;
                    remove_task(DSP_STATE.last);
                }
            }
            0xDCD1_0004 => {
                if let Some(cb) = (*DSP_STATE.curr).req_cb {
                    cb(DSP_STATE.curr);
                }
            }
            _ => {}
        }
    }
}

pub fn init() {
    if !dsp_registers_available() {
        unsafe {
            let level = lock_by_msr();
            DSP_STATE.inited = false;
            DSP_STATE.curr = core::ptr::null_mut();
            DSP_STATE.first = core::ptr::null_mut();
            DSP_STATE.last = core::ptr::null_mut();
            DSP_STATE.rude = core::ptr::null_mut();
            DSP_STATE.rude_pending = false;
            DSP_STATE.intcb = None;
            unlock_by_msr(level);
        }
        return;
    }
    unsafe {
        let level = lock_by_msr();
        if !DSP_STATE.inited {
            DSP_STATE.intcb = Some(def_taskcb);
            #[cfg(target_arch = "powerpc")]
            {
                if irq::get_handler(IRQ_DSP_DSP).is_some() {
                    DSP_STATE.intcb = None;
                    unlock_by_msr(level);
                    return;
                }
                let previous_handler =
                    irq::request(IRQ_DSP_DSP, Some(irq_handler), core::ptr::null_mut());
                if previous_handler.is_some() {
                    // Interrupts are disabled above. If the IRQ backend changed
                    // independently, restore its handler and do not initialize
                    // hardware that this driver cannot service.
                    irq::request(IRQ_DSP_DSP, previous_handler, core::ptr::null_mut());
                    DSP_STATE.intcb = None;
                    unlock_by_msr(level);
                    return;
                }
                irq::unmask(1 << IRQ_DSP_DSP);
            }

            let reg5 = read(5);
            write(
                5,
                (reg5 & !(DSPCR_AIINT | DSPCR_ARINT | DSPCR_DSPINT)) | DSPCR_DSPRESET,
            );
            let reg5 = read(5);
            write(
                5,
                reg5 & !(DSPCR_HALT | DSPCR_AIINT | DSPCR_ARINT | DSPCR_DSPINT),
            );

            DSP_STATE.curr = core::ptr::null_mut();
            DSP_STATE.first = core::ptr::null_mut();
            DSP_STATE.last = core::ptr::null_mut();
            DSP_STATE.rude = core::ptr::null_mut();
            DSP_STATE.rude_pending = false;
            DSP_STATE.inited = true;
        }
        unlock_by_msr(level);
    }
}

pub fn register_callback(usr_cb: DspCallback) -> DspCallback {
    if !dsp_registers_available() {
        let _ = usr_cb;
        return None;
    }
    unsafe {
        let level = lock_by_msr();
        let ret = DSP_STATE.intcb;
        DSP_STATE.intcb = if usr_cb.is_some() {
            usr_cb
        } else {
            Some(def_taskcb)
        };
        unlock_by_msr(level);
        ret
    }
}

pub fn check_mail_to() -> u32 {
    if !dsp_registers_available() {
        return 0;
    }
    unsafe { ((read(0) >> 15) & 1) as u32 }
}

pub fn check_mail_from() -> u32 {
    if !dsp_registers_available() {
        return 0;
    }
    unsafe { ((read(2) >> 15) & 1) as u32 }
}

pub fn read_mail_from() -> u32 {
    if !dsp_registers_available() {
        return 0;
    }
    mailbox::read_from(|index| unsafe { read(index) })
}

pub fn send_mail_to(mail: u32) {
    if !dsp_registers_available() {
        let _ = mail;
        return;
    }
    unsafe {
        write(0, (mail >> 16) as u16);
        write(1, (mail & 0xffff) as u16);
    }
}

pub fn read_cp_uto_dsp() -> u32 {
    if !dsp_registers_available() {
        return 0;
    }
    unsafe { ((read(0) as u32) << 16) | (read(1) as u32 & 0xffff) }
}

pub fn assert_int() {
    if !dsp_registers_available() {
        return;
    }
    unsafe {
        let level = lock_by_msr();
        let reg5 = read(5);
        write(
            5,
            (reg5 & !(DSPCR_AIINT | DSPCR_ARINT | DSPCR_DSPINT)) | DSPCR_PIINT,
        );
        unlock_by_msr(level);
    }
}

pub fn reset() {
    if !dsp_registers_available() {
        return;
    }
    unsafe {
        let level = lock_by_msr();
        let old = read(5);
        write(
            5,
            (old & !(DSPCR_AIINT | DSPCR_ARINT | DSPCR_DSPINT)) | (DSPCR_DSPRESET | DSPCR_RES),
        );
        unlock_by_msr(level);
    }
}

/// Stop DSP activity using the libogc shutdown sequence before a system reset.
pub fn shutdown() {
    shutdown_with_progress(None);
}

/// Run the production shutdown sequence and report each hardware wait.
/// The observer receives a stage, DSP control register, and DSP mailbox word.
/// Stages 1, 2, and 3 precede the DMA, mailbox, and reset waits. Stage 4
/// reports completion. The observer must not change DSP state.
pub fn shutdown_with_progress(progress: Option<fn(u32, u16, u16)>) {
    if !dsp_registers_available() {
        return;
    }
    unsafe {
        write(5, DSPCR_DSPRESET | DSPCR_HALT);
        write(27, read(27) & !0x8000);
        if let Some(progress) = progress {
            progress(1, read(5), read(2));
        }
        while (read(5) & 0x0600) != 0 {}

        write(
            5,
            DSPCR_DSPRESET | DSPCR_DSPINT | DSPCR_ARINT | DSPCR_AIINT | DSPCR_HALT,
        );
        write(0, 0);
        if let Some(progress) = progress {
            progress(2, read(5), read(2));
        }
        // The low-half read acknowledges pending DSP mail. Polling only the
        // high half leaves the busy bit set and prevents reset from starting.
        mailbox::wait_for_empty(|index| read(index));

        let start: u32;
        core::arch::asm!("mftb {tick}", tick = out(reg) start, options(nostack));
        loop {
            let now: u32;
            core::arch::asm!("mftb {tick}", tick = out(reg) now, options(nostack));
            if now.wrapping_sub(start) >= 44 {
                break;
            }
        }

        write(5, read(5) | DSPCR_RES);
        if let Some(progress) = progress {
            progress(3, read(5), read(2));
        }
        while (read(5) & DSPCR_RES) != 0 {}
        if let Some(progress) = progress {
            progress(4, read(5), read(2));
        }
    }
}

pub fn halt() {
    if !dsp_registers_available() {
        return;
    }
    unsafe {
        let level = lock_by_msr();
        let old = read(5);
        write(
            5,
            (old & !(DSPCR_AIINT | DSPCR_ARINT | DSPCR_DSPINT)) | DSPCR_HALT,
        );
        unlock_by_msr(level);
    }
}

pub fn unhalt() {
    if !dsp_registers_available() {
        return;
    }
    unsafe {
        let level = lock_by_msr();
        let reg5 = read(5);
        write(
            5,
            reg5 & !(DSPCR_AIINT | DSPCR_ARINT | DSPCR_DSPINT | DSPCR_HALT),
        );
        unlock_by_msr(level);
    }
}

pub fn get_dma_status() -> u32 {
    if !dsp_registers_available() {
        return 0;
    }
    unsafe { (read(5) & DSPCR_DSPDMA) as u32 }
}

/// # Safety
///
/// `task` must point to a valid `DspTask` that remains allocated while it is
/// linked into the DSP task list.
pub unsafe fn add_task(task: *mut DspTask) -> *mut DspTask {
    if !dsp_registers_available() {
        let _ = task;
        return core::ptr::null_mut();
    }
    if task.is_null() {
        return core::ptr::null_mut();
    }

    unsafe {
        let level = lock_by_msr();
        insert_task(task);
        (*task).state.write(DSPTASK_INIT);
        (*task).flags.write(DSPTASK_ATTACH);
        let start_now = DSP_STATE.first == task;
        unlock_by_msr(level);

        if start_now {
            boot_task(task);
        }
    }

    task
}

/// # Safety
///
/// `task` must point to a valid `DspTask` that is currently managed by the
/// DSP scheduler.
pub unsafe fn cancel_task(task: *mut DspTask) {
    if !dsp_registers_available() {
        let _ = task;
        return;
    }
    if task.is_null() {
        return;
    }

    unsafe {
        let level = lock_by_msr();
        (*task).flags.write((*task).flags.read() | DSPTASK_CANCEL);
        unlock_by_msr(level);
    }
}

/// # Safety
///
/// `task` must point to a valid `DspTask` that is currently managed by the
/// DSP scheduler.
pub unsafe fn assert_task(task: *mut DspTask) -> *mut DspTask {
    if !dsp_registers_available() {
        let _ = task;
        return core::ptr::null_mut();
    }
    if task.is_null() {
        return core::ptr::null_mut();
    }

    unsafe {
        let level = lock_by_msr();
        let mut ret = core::ptr::null_mut();
        if task == DSP_STATE.curr {
            DSP_STATE.rude = task;
            DSP_STATE.rude_pending = true;
            ret = task;
        } else if !DSP_STATE.curr.is_null() && (*task).prio.read() < (*DSP_STATE.curr).prio.read() {
            DSP_STATE.rude = task;
            DSP_STATE.rude_pending = true;
            if (*DSP_STATE.curr).state.read() == DSPTASK_RUN {
                let reg5 = read(5);
                write(
                    5,
                    (reg5 & !(DSPCR_DSPINT | DSPCR_ARINT | DSPCR_AIINT)) | DSPCR_PIINT,
                );
            }
            ret = task;
        }
        unlock_by_msr(level);
        ret
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    unsafe extern "C" fn cb_a() {}

    unsafe extern "C" fn cb_b() {}

    // BUG FIX: The host-only register array is absent in this PowerPC test build.
    fn reset_state() {
        unsafe {
            DSP_STATE = DspState::default();
        }
    }

    #[test]
    fn exports_exist() {
        let _ = init as unsafe fn();
        let _ = register_callback as unsafe fn(DspCallback) -> DspCallback;
        let _ = check_mail_to as unsafe fn() -> u32;
        let _ = check_mail_from as unsafe fn() -> u32;
        let _ = read_mail_from as unsafe fn() -> u32;
        let _ = send_mail_to as unsafe fn(u32);
        let _ = read_cp_uto_dsp as unsafe fn() -> u32;
        let _ = assert_int as unsafe fn();
        let _ = reset as unsafe fn();
        let _ = halt as unsafe fn();
        let _ = unhalt as unsafe fn();
        let _ = get_dma_status as unsafe fn() -> u32;
        let _ = add_task as unsafe fn(*mut DspTask) -> *mut DspTask;
        let _ = cancel_task as unsafe fn(*mut DspTask);
        let _ = assert_task as unsafe fn(*mut DspTask) -> *mut DspTask;
    }

    #[test]
    fn task_layout_matches_c_shape() {
        let ptr_size = size_of::<*mut c_void>();
        let callback_size = size_of::<DspTaskCallback>();
        let expected = 3 * 4
            + ptr_size
            // BUG FIX: Each C address group has one pointer and two u32 fields.
            + 2 * 4
            + ptr_size
            + 2 * 4
            + 2 * 2
            + 4 * callback_size
            + 2 * size_of::<*mut DspTask>();
        assert_eq!(size_of::<DspTask>(), expected);
        assert_eq!(align_of::<DspTask>(), align_of::<*mut c_void>());
        assert_eq!(offset_of!(DspTask, state), 0);
        assert_eq!(offset_of!(DspTask, prio), 4);
        assert_eq!(offset_of!(DspTask, flags), 8);
        assert!(offset_of!(DspTask, init_cb) > offset_of!(DspTask, resume_vec));
        assert!(offset_of!(DspTask, next) > offset_of!(DspTask, req_cb));
    }

    #[test]
    fn task_volatile_fields_read_and_write_values() {
        let field = VolatileU32::new(DSPTASK_INIT);
        assert_eq!(field.read(), DSPTASK_INIT);
        field.write(DSPTASK_RUN);
        assert_eq!(field.read(), DSPTASK_RUN);
    }

    #[test]
    fn task_mailbox_address_preserves_pointer_bits() {
        let address = 0xc123_4567usize as *mut c_void;
        assert_eq!(task_mailbox_address(address), 0xc123_4567);
    }

    #[test]
    fn callback_registration_replaces_previous_callback() {
        reset_state();

        assert!(register_callback(Some(cb_a)).is_none());
        assert_eq!(
            register_callback(Some(cb_b)).map(|cb| cb as *const () as usize),
            Some(cb_a as *const () as usize)
        );
        assert_eq!(
            register_callback(None).map(|cb| cb as *const () as usize),
            Some(cb_b as *const () as usize)
        );
        assert!(unsafe { core::ptr::addr_of!(DSP_STATE.intcb).read().is_some() });
    }

    #[test]
    fn host_mailbox_and_control_register_state_is_host_compat() {
        reset_state();

        send_mail_to(0x1234_5678);
        assert_eq!(read_cp_uto_dsp(), 0x1234_5678);
        unsafe {
            write(2, 0xabcd);
            write(3, 0xef01);
        }
        assert_eq!(read_mail_from(), 0xabcd_ef01);

        assert_int();
        assert_ne!(unsafe { read(5) } & DSPCR_PIINT, 0);
        halt();
        assert_ne!(unsafe { read(5) } & DSPCR_HALT, 0);
        unhalt();
        assert_eq!(unsafe { read(5) } & DSPCR_HALT, 0);
    }

    #[test]
    fn task_queue_orders_by_priority_and_tracks_cancel_assert() {
        reset_state();
        let mut low = DspTask {
            prio: VolatileU32::new(10),
            ..DspTask::default()
        };
        let mut high = DspTask {
            prio: VolatileU32::new(1),
            ..DspTask::default()
        };

        unsafe {
            insert_task(&mut low);
            insert_task(&mut high);
            assert_eq!(
                DSP_STATE.first as usize,
                (&mut high as *mut DspTask) as usize
            );
            assert_eq!(DSP_STATE.last as usize, (&mut low as *mut DspTask) as usize);
            assert_eq!(high.next as usize, (&mut low as *mut DspTask) as usize);
            assert_eq!(low.prev as usize, (&mut high as *mut DspTask) as usize);

            DSP_STATE.curr = &mut low;
            assert_eq!(
                assert_task(&mut high) as usize,
                (&mut high as *mut DspTask) as usize
            );
            assert_eq!(
                DSP_STATE.rude as usize,
                (&mut high as *mut DspTask) as usize
            );
            assert!(DSP_STATE.rude_pending);

            cancel_task(&mut high);
            assert_ne!(high.flags.read() & DSPTASK_CANCEL, 0);

            remove_task(&mut high);
            assert_eq!(high.state.read(), DSPTASK_DONE);
            assert_eq!(high.flags.read(), DSPTASK_CLEARALL);
        }
    }
}
