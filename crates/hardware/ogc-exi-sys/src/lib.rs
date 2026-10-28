#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_irq::{
    self as irq, IRQ_EXI0_EXI, IRQ_EXI0_EXT, IRQ_EXI0_TC, IRQ_EXI1_EXI, IRQ_EXI1_EXT, IRQ_EXI1_TC,
    IRQ_EXI2_EXI, IRQ_EXI2_TC, IRQ_PI_DEBUG,
};

pub const EXI_READ: u32 = 0;
pub const EXI_WRITE: u32 = 1;
pub const EXI_READWRITE: u32 = 2;

pub const EXI_CHANNEL_0: i32 = 0;
pub const EXI_CHANNEL_1: i32 = 1;
pub const EXI_CHANNEL_2: i32 = 2;
pub const EXI_CHANNEL_MAX: i32 = 3;

pub const EXI_DEVICE_0: i32 = 0;
pub const EXI_DEVICE_1: i32 = 1;
pub const EXI_DEVICE_2: i32 = 2;
pub const EXI_DEVICE_MAX: i32 = 3;

pub const EXI_SPEED1MHZ: i32 = 0;
pub const EXI_SPEED2MHZ: i32 = 1;
pub const EXI_SPEED4MHZ: i32 = 2;
pub const EXI_SPEED8MHZ: i32 = 3;
pub const EXI_SPEED16MHZ: i32 = 4;
pub const EXI_SPEED32MHZ: i32 = 5;

pub const EXI_FLAG_DMA: u32 = 0x0001;
pub const EXI_FLAG_IMM: u32 = 0x0002;
pub const EXI_FLAG_SELECT: u32 = 0x0004;
pub const EXI_FLAG_ATTACH: u32 = 0x0008;
pub const EXI_FLAG_LOCKED: u32 = 0x0010;
const SYS_BASE_UNCACHED: u32 = 0xC000_0000;

const EXI_EXI_IRQ: u32 = 0x0002;
const EXI_TC_IRQ: u32 = 0x0008;
const EXI_EXT_IRQ: u32 = 0x0800;
const EXI_EXT_BIT: u32 = 0x1000;

pub const EXI_MEMCARD59: u32 = 0x0000_0004;
pub const EXI_MEMCARD123: u32 = 0x0000_0008;
pub const EXI_MEMCARD251: u32 = 0x0000_0010;
pub const EXI_MEMCARD507: u32 = 0x0000_0020;
pub const EXI_MEMCARD1019: u32 = 0x0000_0040;
pub const EXI_MEMCARD2043: u32 = 0x0000_0080;

pub type ExiCallback = Option<unsafe extern "C" fn(chn: i32, dev: i32) -> i32>;

#[derive(Copy, Clone)]
struct ExiBus {
    flags: u32,
    lockeddev: i32,
    queued_unlocks: [(i32, ExiCallback); 3],
    cb_exi: ExiCallback,
    cb_tc: ExiCallback,
    cb_ext: ExiCallback,
    imm_len: u32,
    imm_buff: *mut c_void,
    attached: bool,
    id_cache: Option<u32>,
    probe_since: u64,
    selected_dev: i32,
    selected_frq: i32,
}

impl ExiBus {
    const fn new() -> Self {
        Self {
            flags: 0,
            lockeddev: -1,
            queued_unlocks: [(-1, None); 3],
            cb_exi: None,
            cb_tc: None,
            cb_ext: None,
            imm_len: 0,
            imm_buff: core::ptr::null_mut(),
            attached: false,
            id_cache: None,
            probe_since: 0,
            selected_dev: -1,
            selected_frq: EXI_SPEED1MHZ,
        }
    }
}

struct SpinLock(AtomicBool);

impl SpinLock {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) -> SpinGuard<'_> {
        let interrupt_state = unsafe { lock_by_msr() };
        while self.0.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinGuard {
            lock: self,
            interrupt_state,
        }
    }
}

struct SpinGuard<'a> {
    lock: &'a SpinLock,
    interrupt_state: u32,
}

impl SpinGuard<'_> {
    #[inline(always)]
    fn release_lock_keep_interrupts(self) -> u32 {
        let state = self.interrupt_state;
        self.lock.0.store(false, Ordering::Release);
        core::mem::forget(self);
        state
    }
}

impl Drop for SpinGuard<'_> {
    fn drop(&mut self) {
        self.lock.0.store(false, Ordering::Release);
        unsafe {
            unlock_by_msr(self.interrupt_state);
        }
    }
}

static EXI_LOCK: SpinLock = SpinLock::new();
static EXI_IRQS_READY: AtomicBool = AtomicBool::new(false);
static mut EXI_BUS: [ExiBus; 3] = [ExiBus::new(), ExiBus::new(), ExiBus::new()];
#[cfg(any(test, not(target_arch = "powerpc")))]
static mut EXI_REGS: [u32; 15] = [0; 15];
const EXI_BUS_COUNT: usize = 3;

#[inline(always)]
fn exi_runtime_enabled() -> bool {
    true
}

fn chan_valid(chn: i32) -> bool {
    (0..3).contains(&chn)
}

fn dev_valid(dev: i32) -> bool {
    (0..3).contains(&dev)
}

const EXI_REG_BASE_GAMECUBE: usize = 0xCC00_6800;
const EXI_REG_BASE_WII: usize = 0xCD00_6800;
#[cfg(target_arch = "powerpc")]
static EXI_REG_BASE: AtomicUsize = AtomicUsize::new(0);

const fn exi_reg_base_for_model(hollywood_revision: u32) -> usize {
    if hollywood_revision == 0 {
        EXI_REG_BASE_GAMECUBE
    } else {
        EXI_REG_BASE_WII
    }
}

#[cfg(target_arch = "powerpc")]
fn hardware_exi_reg_base() -> usize {
    let cached = EXI_REG_BASE.load(Ordering::Acquire);
    if cached != 0 {
        return cached;
    }
    let hollywood_revision = unsafe { ogc_sys::sys_get_hollywood_revision() };
    let detected = exi_reg_base_for_model(hollywood_revision);
    match EXI_REG_BASE.compare_exchange(0, detected, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => detected,
        Err(base) => base,
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn reg_ptr(index: usize) -> *mut u32 {
    (hardware_exi_reg_base() as *mut u32).wrapping_add(index)
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
fn reg_ptr(index: usize) -> *mut u32 {
    core::ptr::addr_of_mut!(EXI_REGS)
        .cast::<u32>()
        .wrapping_add(index)
}

#[inline(always)]
unsafe fn read(index: usize) -> u32 {
    unsafe { read_volatile(reg_ptr(index)) }
}

#[inline(always)]
unsafe fn write(index: usize, value: u32) {
    unsafe { write_volatile(reg_ptr(index), value) }
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
    let msr = unsafe { ppc_mfmsr() };
    unsafe {
        ppc_mtmsr(msr & !(1 << 15));
    }
    msr & (1 << 15)
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn lock_by_msr() -> u32 {
    0
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn unlock_by_msr(state: u32) {
    let msr = unsafe { ppc_mfmsr() } & !(1 << 15);
    unsafe {
        ppc_mtmsr(msr | state);
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn unlock_by_msr(_state: u32) {}

#[inline(always)]
fn mem_virtual_to_physical(ptr: *const c_void) -> u32 {
    (ptr as usize as u32) & !SYS_BASE_UNCACHED
}

fn bus_mut(chn: i32) -> Option<&'static mut ExiBus> {
    if !chan_valid(chn) {
        return None;
    }
    Some(unsafe { &mut EXI_BUS[chn as usize] })
}

#[cfg(target_arch = "powerpc")]
#[inline]
const fn irq_mask(id: u32) -> u32 {
    1u32 << id
}

#[cfg(target_arch = "powerpc")]
const EXI_IRQ_MASK: u32 = irq_mask(IRQ_EXI0_EXI)
    | irq_mask(IRQ_EXI0_TC)
    | irq_mask(IRQ_EXI0_EXT)
    | irq_mask(IRQ_EXI1_EXI)
    | irq_mask(IRQ_EXI1_TC)
    | irq_mask(IRQ_EXI1_EXT)
    | irq_mask(IRQ_EXI2_EXI)
    | irq_mask(IRQ_EXI2_TC);

#[cfg(target_arch = "powerpc")]
#[inline]
fn mask_attached_external_interrupt(chn: usize) {
    if chn < 2 {
        irq::mask(irq_mask(channel_irq_id(IRQ_EXI0_EXT, chn)));
    }
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn unmask_attached_external_interrupt(chn: usize) {
    if chn < 2 {
        irq::unmask(irq_mask(channel_irq_id(IRQ_EXI0_EXT, chn)));
    }
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn irq_channel(irq_id: u32, first: u32) -> Option<usize> {
    let delta = irq_id.checked_sub(first)?;
    (delta % 3 == 0)
        .then_some((delta / 3) as usize)
        .filter(|channel| *channel < EXI_BUS_COUNT)
}

#[cfg(target_arch = "powerpc")]
#[inline]
unsafe fn clear_channel_irqs(chn: usize, exi: bool, tc: bool, ext: bool) {
    let index = chn * 5;
    let mut value = unsafe { read(index) } & !(EXI_EXI_IRQ | EXI_TC_IRQ | EXI_EXT_IRQ);
    if exi {
        value |= EXI_EXI_IRQ;
    }
    if tc {
        value |= EXI_TC_IRQ;
    }
    if ext {
        value |= EXI_EXT_IRQ;
    }
    unsafe {
        write(index, value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline]
unsafe fn selected_device(chn: usize) -> i32 {
    ((unsafe { read(chn * 5) } >> 8) & 0x3) as i32
}

unsafe fn complete_transfer(bus: &mut ExiBus, immediate_word: u32) -> ExiCallback {
    let callback = bus.cb_tc.take();
    if bus.flags & EXI_FLAG_IMM != 0 && !bus.imm_buff.is_null() && bus.imm_len > 0 {
        let bytes = bus.imm_len.min(4) as usize;
        let ptr = bus.imm_buff.cast::<u8>();
        for index in 0..bytes {
            unsafe {
                *ptr.add(index) = (immediate_word >> ((3 - index) * 8)) as u8;
            }
        }
    }
    bus.flags &= !(EXI_FLAG_DMA | EXI_FLAG_IMM);
    bus.imm_buff = core::ptr::null_mut();
    bus.imm_len = 0;
    callback
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn irq_handler(irq_id: u32, _ctx: *mut c_void) {
    let Some(chn) = irq_channel(irq_id, IRQ_EXI0_EXI) else {
        return;
    };
    let device = unsafe { selected_device(chn) };
    let callback = {
        let _guard = EXI_LOCK.lock();
        unsafe {
            clear_channel_irqs(chn, true, false, false);
            EXI_BUS[chn].cb_exi
        }
    };
    if let Some(callback) = callback {
        unsafe {
            callback(chn as i32, device);
        }
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn tc_irq_handler(irq_id: u32, _ctx: *mut c_void) {
    let Some(chn) = irq_channel(irq_id, IRQ_EXI0_TC) else {
        return;
    };
    irq::mask(irq_mask(irq_id));
    let device = unsafe { selected_device(chn) };
    let callback = {
        let _guard = EXI_LOCK.lock();
        unsafe {
            clear_channel_irqs(chn, false, true, false);
            let immediate_word = read(chn * 5 + 4);
            complete_transfer(&mut EXI_BUS[chn], immediate_word)
        }
    };
    if let Some(callback) = callback {
        unsafe {
            callback(chn as i32, device);
        }
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn ext_irq_handler(irq_id: u32, _ctx: *mut c_void) {
    let Some(chn) = irq_channel(irq_id, IRQ_EXI0_EXT) else {
        return;
    };
    irq::mask(irq_mask(irq_id));
    let device = unsafe { selected_device(chn) };
    let callback = {
        let _guard = EXI_LOCK.lock();
        unsafe {
            clear_channel_irqs(chn, false, false, true);
            let bus = &mut EXI_BUS[chn];
            bus.attached = false;
            bus.id_cache = None;
            bus.probe_since = 0;
            bus.flags &= !EXI_FLAG_ATTACH;
            bus.cb_ext
        }
    };
    if let Some(callback) = callback {
        unsafe {
            callback(chn as i32, device);
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn handler_address(handler: ogc_irq::KIrqHandlerFn) -> usize {
    handler.map_or(0, |handler| handler as usize)
}

#[cfg(target_arch = "powerpc")]
unsafe fn install_irq_handlers() -> bool {
    let handlers = [
        (
            IRQ_EXI0_EXI,
            Some(irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI0_TC,
            Some(tc_irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI0_EXT,
            Some(ext_irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI1_EXI,
            Some(irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI1_TC,
            Some(tc_irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI1_EXT,
            Some(ext_irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI2_EXI,
            Some(irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
        (
            IRQ_EXI2_TC,
            Some(tc_irq_handler as unsafe extern "C" fn(u32, *mut c_void)),
        ),
    ];
    for (irq_id, handler) in handlers {
        unsafe {
            irq::request(irq_id, handler, core::ptr::null_mut());
        }
    }
    handlers.into_iter().all(|(irq_id, expected)| {
        handler_address(irq::get_handler(irq_id)) == handler_address(expected)
    })
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn channel_irq_id(first: u32, chn: usize) -> u32 {
    first + chn as u32 * 3
}

#[cfg(target_arch = "powerpc")]
fn refresh_exi_interrupt(chn: usize, bus: &ExiBus) {
    if !EXI_IRQS_READY.load(Ordering::Acquire) {
        if chn == EXI_CHANNEL_0 as usize {
            irq::mask(irq_mask(IRQ_EXI0_EXI) | irq_mask(IRQ_EXI2_EXI));
        } else {
            irq::mask(irq_mask(channel_irq_id(IRQ_EXI0_EXI, chn)));
        }
        return;
    }

    match chn as i32 {
        EXI_CHANNEL_0 => {
            let mask = irq_mask(IRQ_EXI0_EXI) | irq_mask(IRQ_EXI2_EXI);
            irq::mask(mask);
            let exi2_has_callback = unsafe { EXI_BUS[EXI_CHANNEL_2 as usize].cb_exi.is_some() };
            if bus.flags & EXI_FLAG_LOCKED == 0 && (bus.cb_exi.is_some() || exi2_has_callback) {
                irq::unmask(mask);
            }
        }
        EXI_CHANNEL_1 => {
            let mask = irq_mask(IRQ_EXI1_EXI);
            irq::mask(mask);
            if bus.flags & EXI_FLAG_LOCKED == 0 && bus.cb_exi.is_some() {
                irq::unmask(mask);
            }
        }
        EXI_CHANNEL_2 => {
            // Match libogc's debugger-only channel-2 branch.
            irq::mask(irq_mask(IRQ_EXI0_EXI));
            if bus.flags & EXI_FLAG_LOCKED == 0 && irq::get_handler(IRQ_PI_DEBUG).is_some() {
                irq::unmask(irq_mask(IRQ_EXI2_EXI));
            }
        }
        _ => {}
    }
}

#[cfg(target_arch = "powerpc")]
fn arm_transfer_interrupt(chn: usize) {
    unsafe {
        clear_channel_irqs(chn, false, true, false);
    }
    irq::unmask(irq_mask(channel_irq_id(IRQ_EXI0_TC, chn)));
}

#[cfg(target_arch = "powerpc")]
fn arm_external_interrupt(chn: usize) {
    unsafe {
        clear_channel_irqs(chn, true, false, false);
    }
    irq::unmask(irq_mask(channel_irq_id(IRQ_EXI0_EXT, chn)));
}

pub fn probe_reset() {
    if !exi_runtime_enabled() {
        return;
    }
    // BUG FIX: C clears the cached device IDs before it probes both external
    // channels. Reset the Rust cache before the same probe sequence.
    {
        let _guard = EXI_LOCK.lock();
        unsafe {
            EXI_BUS[EXI_CHANNEL_0 as usize].id_cache = None;
            EXI_BUS[EXI_CHANNEL_1 as usize].id_cache = None;
            EXI_BUS[EXI_CHANNEL_0 as usize].probe_since = 0;
            EXI_BUS[EXI_CHANNEL_1 as usize].probe_since = 0;
        }
    }
    probe(EXI_CHANNEL_0);
    probe(EXI_CHANNEL_1);
    let mut serial_port_id = 0;
    unsafe {
        get_id(EXI_CHANNEL_0, EXI_DEVICE_2, &mut serial_port_id);
    }
}

pub fn init() {
    if !exi_runtime_enabled() {
        return;
    }
    EXI_IRQS_READY.store(false, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    irq::mask(EXI_IRQ_MASK);
    let _guard = EXI_LOCK.lock();
    unsafe {
        write(0, 0);
        write(5, 0);
        write(10, 0);
        write(0, 0x2000);
        let mut idx = 0;
        while idx < EXI_BUS_COUNT {
            core::ptr::write(core::ptr::addr_of_mut!(EXI_BUS[idx]), ExiBus::new());
            idx += 1;
        }
        #[cfg(target_arch = "powerpc")]
        {
            EXI_IRQS_READY.store(install_irq_handlers(), Ordering::Release);
        }
    }
    drop(_guard);
    // BUG FIX: libogc initializes its device-probe state after it installs the
    // EXI interrupt handlers. Do the same after releasing the state lock.
    probe_reset();
}

pub fn interrupts_ready() -> bool {
    EXI_IRQS_READY.load(Ordering::Acquire)
}

pub fn get_state(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    bus_mut(chn).map_or(0, |bus| bus.flags as i32)
}

pub fn lock(chn: i32, dev: i32, unlock_cb: ExiCallback) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, dev, unlock_cb);
        return 0;
    }
    if !chan_valid(chn) || !dev_valid(dev) {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & EXI_FLAG_LOCKED != 0 {
        if let Some(callback) = unlock_cb {
            if bus
                .queued_unlocks
                .iter()
                .any(|(queued_dev, _)| *queued_dev == dev)
            {
                return 0;
            }
            if let Some(slot) = bus
                .queued_unlocks
                .iter_mut()
                .find(|(queued_dev, _)| *queued_dev < 0)
            {
                *slot = (dev, Some(callback));
            }
        }
        return 0;
    }
    bus.flags |= EXI_FLAG_LOCKED;
    bus.lockeddev = dev;
    #[cfg(target_arch = "powerpc")]
    refresh_exi_interrupt(chn as usize, bus);
    1
}

pub fn unlock(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    if !chan_valid(chn) {
        return 0;
    }
    let guard = EXI_LOCK.lock();
    let callback = {
        let bus = unsafe { &mut EXI_BUS[chn as usize] };
        if bus.flags & EXI_FLAG_LOCKED == 0 {
            return 0;
        }
        bus.flags &= !EXI_FLAG_LOCKED;
        bus.lockeddev = -1;
        #[cfg(target_arch = "powerpc")]
        refresh_exi_interrupt(chn as usize, bus);
        let slot = bus
            .queued_unlocks
            .iter_mut()
            .find(|(dev, cb)| *dev >= 0 && cb.is_some());
        slot.map(|entry| {
            let queued = *entry;
            *entry = (-1, None);
            queued
        })
    };

    if let Some((queued_dev, Some(cb))) = callback {
        // C releases its queue bookkeeping before the callback, but keeps external
        // interrupts disabled until the callback returns. Release only the Rust
        // software lock here so callbacks may call back into EXI without deadlock.
        let interrupt_state = guard.release_lock_keep_interrupts();
        unsafe {
            cb(chn, queued_dev);
            unlock_by_msr(interrupt_state);
        }
    }
    1
}

fn can_select(chn: i32, dev: i32, bus: &ExiBus) -> bool {
    chn == EXI_CHANNEL_2 || (bus.flags & EXI_FLAG_LOCKED != 0 && bus.lockeddev == dev)
}

#[inline]
const fn has_selected_transfer(flags: u32) -> bool {
    flags & EXI_FLAG_SELECT != 0 && flags & (EXI_FLAG_DMA | EXI_FLAG_IMM) != 0
}

#[inline]
fn device_zero_present(chn: i32) -> bool {
    if !(EXI_CHANNEL_0..=EXI_CHANNEL_1).contains(&chn) {
        return chn == EXI_CHANNEL_2;
    }

    device_zero_present_in_register(unsafe { read((chn as usize) * 5) })
}

#[inline]
const fn device_zero_present_in_register(register: u32) -> bool {
    register & EXI_EXT_BIT != 0
}

#[inline]
const fn device_zero_probe_succeeds(register: u32) -> bool {
    device_zero_present_in_register(register) && register & EXI_EXT_IRQ == 0
}

const fn probe_debounce_ticks(ticks_per_second: u64) -> u64 {
    // C reports a device only after diff_usec() reaches 20 microseconds.
    (20 * ticks_per_second).div_ceil(1_000_000)
}

const fn probe_debounce_elapsed(start: u64, now: u64, ticks_per_second: u64) -> bool {
    now.wrapping_sub(start) >= probe_debounce_ticks(ticks_per_second)
}

#[cfg(target_arch = "powerpc")]
fn read_probe_timebase() -> u64 {
    let (upper, lower): (u32, u32);
    unsafe {
        core::arch::asm!(
            "1:",
            "mftbu {upper}",
            "mftb {lower}",
            "mftbu {check}",
            "cmpw {upper}, {check}",
            "bne 1b",
            upper = out(reg) upper,
            lower = out(reg) lower,
            check = out(reg) _,
            options(nostack)
        );
    }
    ((upper as u64) << 32) | lower as u64
}

#[cfg(target_arch = "powerpc")]
fn probe_timebase_hz() -> u64 {
    if unsafe { ogc_sys::sys_get_hollywood_revision() } == 0 {
        40_500_000
    } else {
        60_750_000
    }
}

fn device_zero_probe_locked(chn: i32, bus: &mut ExiBus) -> bool {
    if !(EXI_CHANNEL_0..=EXI_CHANNEL_1).contains(&chn) {
        return chn == EXI_CHANNEL_2;
    }
    let index = (chn as usize) * 5;
    let register = unsafe { read(index) };
    if register & EXI_EXT_IRQ != 0 {
        bus.id_cache = None;
        bus.probe_since = 0;
        #[cfg(target_arch = "powerpc")]
        unsafe {
            clear_channel_irqs(chn as usize, false, false, true);
        }
        #[cfg(not(target_arch = "powerpc"))]
        unsafe {
            write(index, register & !EXI_EXT_IRQ);
        }
        return false;
    }
    if !device_zero_probe_succeeds(register) {
        bus.id_cache = None;
        bus.probe_since = 0;
        return false;
    }
    if bus.attached {
        return true;
    }
    let now = read_probe_timebase();
    if bus.probe_since == 0 {
        bus.probe_since = now;
        return false;
    }
    probe_debounce_elapsed(bus.probe_since, now, probe_timebase_hz())
}

#[inline]
const fn select_sd_needs_id_check(chn: i32, dev: i32, attached: bool) -> bool {
    chn != EXI_CHANNEL_2 && dev == EXI_DEVICE_0 && !attached
}

#[inline]
const fn imm_ex_has_work(n_len: u32) -> bool {
    n_len != 0
}

#[inline]
const fn deselect_result_after_probe(selected_dev: i32, probe_ok: bool) -> i32 {
    if selected_dev == EXI_DEVICE_0 && !probe_ok {
        0
    } else {
        1
    }
}

pub fn select(chn: i32, dev: i32, frq: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, dev, frq);
        return 0;
    }
    if !chan_valid(chn) || !dev_valid(dev) {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & EXI_FLAG_SELECT != 0 || !can_select(chn, dev, bus) {
        return 0;
    }
    if chn != EXI_CHANNEL_2
        && dev == EXI_DEVICE_0
        && !bus.attached
        && !device_zero_probe_locked(chn, bus)
    {
        return 0;
    }
    bus.flags |= EXI_FLAG_SELECT;
    bus.selected_dev = dev;
    bus.selected_frq = frq;
    unsafe {
        let val = read((chn as usize) * 5);
        write(
            (chn as usize) * 5,
            (val & 0x405) | (0x80u32 << (dev as u32)) | ((frq as u32) << 4),
        );
    }
    #[cfg(target_arch = "powerpc")]
    if bus.attached {
        mask_attached_external_interrupt(chn as usize);
    }
    1
}

pub fn select_sd(chn: i32, dev: i32, frq: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, dev, frq);
        return 0;
    }
    if !chan_valid(chn) || !dev_valid(dev) {
        return 0;
    }
    // BUG FIX: C validates device 0 by probing it and reading its ID when the
    // ID is not cached. Keep that transaction outside the EXI spin lock because
    // get_id acquires the same lock while it selects and reads the device.
    if select_sd_needs_id_check(chn, dev, {
        let _guard = EXI_LOCK.lock();
        unsafe { EXI_BUS[chn as usize].attached }
    }) {
        let mut id = 0;
        if unsafe { get_id(chn, dev, &mut id) } == 0 {
            return 0;
        }
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & EXI_FLAG_SELECT != 0 {
        return 0;
    }
    if chn != EXI_CHANNEL_2 {
        if dev == EXI_DEVICE_0 && !bus.attached && !device_zero_probe_locked(chn, bus) {
            return 0;
        }
        if bus.flags & EXI_FLAG_LOCKED == 0 || bus.lockeddev != dev {
            return 0;
        }
    }
    bus.flags |= EXI_FLAG_SELECT;
    bus.selected_dev = dev;
    bus.selected_frq = frq;
    unsafe {
        let val = read((chn as usize) * 5);
        write((chn as usize) * 5, (val & 0x405) | ((frq as u32) << 4));
    }
    #[cfg(target_arch = "powerpc")]
    if bus.attached {
        mask_attached_external_interrupt(chn as usize);
    }
    1
}

pub fn deselect(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    if !chan_valid(chn) {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & EXI_FLAG_SELECT == 0 {
        return 0;
    }
    let selected_dev = bus.selected_dev;
    bus.flags &= !EXI_FLAG_SELECT;
    bus.selected_dev = -1;
    unsafe {
        let val = read((chn as usize) * 5);
        write((chn as usize) * 5, val & 0x405);
    }
    #[cfg(target_arch = "powerpc")]
    if bus.attached {
        unmask_attached_external_interrupt(chn as usize);
    }
    // BUG FIX: C probes device 0 after it deselects it and reports failure if
    // the presence check fails. Invalidate the cached ID on removal as well.
    if chn != EXI_CHANNEL_2 && selected_dev == EXI_DEVICE_0 {
        deselect_result_after_probe(selected_dev, device_zero_probe_locked(chn, bus))
    } else {
        deselect_result_after_probe(selected_dev, true)
    }
}

pub fn sync(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    if !chan_valid(chn) {
        return 0;
    }
    while unsafe { read((chn as usize) * 5 + 3) } & 1 != 0 {
        core::hint::spin_loop();
    }
    // BUG FIX: C waits for hardware completion before disabling interrupts.
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    let mut result = 0;
    // BUG FIX: C reports completion only while a transfer remains selected.
    if has_selected_transfer(bus.flags) {
        if bus.flags & EXI_FLAG_IMM != 0 && !bus.imm_buff.is_null() && bus.imm_len > 0 {
            unsafe {
                let value = read((chn as usize) * 5 + 4);
                let bytes = bus.imm_len.min(4) as usize;
                let ptr = bus.imm_buff as *mut u8;
                for i in 0..bytes {
                    *ptr.add(i) = (value >> ((3 - i) * 8)) as u8;
                }
            }
        }
        bus.flags &= !(EXI_FLAG_DMA | EXI_FLAG_IMM);
        result = 1;
    }
    result
}

/// # Safety
///
/// `p_data` must be valid for the requested transfer size and remain alive
/// until `sync` completes.
pub unsafe fn imm(
    chn: i32,
    p_data: *mut c_void,
    n_len: u32,
    n_mode: i32,
    tc_cb: ExiCallback,
) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, p_data, n_len, n_mode, tc_cb);
        return 0;
    }
    if !chan_valid(chn) || n_len == 0 || n_len > 4 {
        return 0;
    }
    if tc_cb.is_some() && !interrupts_ready() {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & (EXI_FLAG_DMA | EXI_FLAG_IMM) != 0 || bus.flags & EXI_FLAG_SELECT == 0 {
        return 0;
    }
    bus.flags |= EXI_FLAG_IMM;
    bus.imm_len = n_len;
    bus.imm_buff = p_data;
    bus.cb_tc = tc_cb;
    #[cfg(target_arch = "powerpc")]
    if tc_cb.is_some() {
        arm_transfer_interrupt(chn as usize);
    }
    if n_mode != EXI_READ as i32 {
        let mut value = 0u32;
        let src = p_data as *const u8;
        for i in 0..n_len as usize {
            value |= (unsafe { *src.add(i) } as u32) << ((3 - i) * 8);
        }
        unsafe {
            write((chn as usize) * 5 + 4, value);
        }
    }
    if n_mode == EXI_WRITE as i32 {
        bus.imm_len = 0;
    }
    unsafe {
        write(
            (chn as usize) * 5 + 3,
            (((n_len - 1) & 0x3) << 4) | ((n_mode as u32 & 0x3) << 2) | 0x01,
        );
    }
    1
}

/// # Safety
///
/// `p_data` must be valid for the requested transfer size and remain alive
/// until the transfer completes.
pub unsafe fn imm_ex(chn: i32, p_data: *mut c_void, n_len: u32, n_mode: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, p_data, n_len, n_mode);
        return 0;
    }
    // BUG FIX: C returns failure when the transfer loop has no bytes to send.
    if !imm_ex_has_work(n_len) {
        return 0;
    }
    unsafe {
        let mut offset = 0usize;
        let mut remaining = n_len as usize;
        let data = p_data as *mut u8;
        while remaining > 0 {
            let chunk = remaining.min(4);
            if imm(chn, data.add(offset).cast(), chunk as u32, n_mode, None) == 0 {
                return 0;
            }
            if sync(chn) == 0 {
                return 0;
            }
            offset += chunk;
            remaining -= chunk;
        }
        1
    }
}

/// # Safety
///
/// `p_data` must be valid for the DMA span and remain pinned/aligned as the
/// caller expects for the EXI transfer.
pub unsafe fn dma(
    chn: i32,
    p_data: *mut c_void,
    n_len: u32,
    n_mode: i32,
    tc_cb: ExiCallback,
) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, p_data, n_len, n_mode, tc_cb);
        return 0;
    }
    if !chan_valid(chn) || n_len == 0 {
        return 0;
    }
    if tc_cb.is_some() && !interrupts_ready() {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & (EXI_FLAG_DMA | EXI_FLAG_IMM) != 0 || bus.flags & EXI_FLAG_SELECT == 0 {
        return 0;
    }
    bus.flags |= EXI_FLAG_DMA;
    bus.imm_buff = p_data;
    bus.imm_len = n_len;
    bus.cb_tc = tc_cb;
    #[cfg(target_arch = "powerpc")]
    if tc_cb.is_some() {
        arm_transfer_interrupt(chn as usize);
    }
    unsafe {
        write(
            (chn as usize) * 5 + 1,
            mem_virtual_to_physical(p_data.cast_const()) & 0x03FF_FFE0,
        );
        write((chn as usize) * 5 + 2, n_len);
        write((chn as usize) * 5 + 3, ((n_mode as u32 & 0x3) << 2) | 0x03);
    }
    1
}

/// # Safety
///
/// `n_id` must point to writable memory for one `u32`.
pub unsafe fn get_id(chn: i32, dev: i32, n_id: *mut u32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, dev);
        if !n_id.is_null() {
            unsafe {
                *n_id = 0;
            }
        }
        return 0;
    }
    if !chan_valid(chn) || !dev_valid(dev) || n_id.is_null() {
        return 0;
    }
    let cache_channel = (EXI_CHANNEL_0..=EXI_CHANNEL_1).contains(&chn) && dev == EXI_DEVICE_0;
    if cache_channel {
        let _guard = EXI_LOCK.lock();
        let bus = unsafe { &mut EXI_BUS[chn as usize] };
        if !device_zero_probe_locked(chn, bus) {
            return 0;
        }
        if let Some(id) = bus.id_cache {
            unsafe { *n_id = id };
            return 1;
        }
    }
    if lock(chn, dev, None) == 0 {
        return 0;
    }

    let mut command = [0u8; 2];
    let mut id_bytes = [0u8; 4];
    let mut success = select(chn, dev, EXI_SPEED1MHZ) != 0;
    if success {
        success = unsafe {
            imm(
                chn,
                command.as_mut_ptr().cast(),
                command.len() as u32,
                EXI_WRITE as i32,
                None,
            ) != 0
        } && sync(chn) != 0;
    }
    if success {
        success = unsafe {
            imm(
                chn,
                id_bytes.as_mut_ptr().cast(),
                id_bytes.len() as u32,
                EXI_READ as i32,
                None,
            ) != 0
        } && sync(chn) != 0;
    }

    if get_state(chn) as u32 & EXI_FLAG_SELECT != 0 && deselect(chn) == 0 {
        success = false;
    }
    if unlock(chn) == 0 {
        success = false;
    }
    if !success {
        return 0;
    }

    let id = u32::from_be_bytes(id_bytes);
    if cache_channel {
        let _guard = EXI_LOCK.lock();
        let bus = unsafe { &mut EXI_BUS[chn as usize] };
        if !device_zero_probe_locked(chn, bus) {
            return 0;
        }
        bus.id_cache = Some(id);
    }
    unsafe {
        *n_id = id;
    }
    1
}

pub fn attach(chn: i32, ext_cb: ExiCallback) -> i32 {
    if !exi_runtime_enabled() {
        let _ = (chn, ext_cb);
        return 0;
    }
    if !(EXI_CHANNEL_0..=EXI_CHANNEL_1).contains(&chn) || !interrupts_ready() {
        return 0;
    }
    // BUG FIX: C attaches only after EXI_Probe has read a valid device ID.
    if probe(chn) == 0 {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.flags & EXI_FLAG_ATTACH != 0 || !device_zero_probe_locked(chn, bus) {
        return 0;
    }
    bus.attached = true;
    bus.cb_ext = ext_cb;
    bus.flags |= EXI_FLAG_ATTACH;
    #[cfg(target_arch = "powerpc")]
    arm_external_interrupt(chn as usize);
    1
}

pub fn detach(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    if !chan_valid(chn) {
        return 0;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    if bus.attached && bus.flags & EXI_FLAG_LOCKED != 0 && bus.lockeddev != EXI_DEVICE_0 {
        return 0;
    }
    bus.attached = false;
    bus.flags &= !EXI_FLAG_ATTACH;
    bus.cb_ext = None;
    #[cfg(target_arch = "powerpc")]
    {
        let channel = chn as usize;
        let mut mask = irq_mask(channel_irq_id(IRQ_EXI0_EXI, channel))
            | irq_mask(channel_irq_id(IRQ_EXI0_TC, channel));
        if chn <= EXI_CHANNEL_1 {
            mask |= irq_mask(channel_irq_id(IRQ_EXI0_EXT, channel));
        }
        irq::mask(mask);
    }
    1
}

pub fn probe(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    let mut id = 0u32;
    if unsafe { get_id(chn, EXI_DEVICE_0, &mut id) } != 0 {
        1
    } else {
        0
    }
}

pub fn probe_ex(chn: i32) -> i32 {
    if !exi_runtime_enabled() {
        let _ = chn;
        return 0;
    }
    probe_ex_result(probe(chn), {
        (EXI_CHANNEL_0..=EXI_CHANNEL_1).contains(&chn) && device_zero_present(chn)
    })
}

#[inline]
const fn probe_ex_result(probe_result: i32, device_present: bool) -> i32 {
    if probe_result == 1 {
        1
    } else if device_present {
        0
    } else {
        -1
    }
}

pub fn register_exi_callback(chn: i32, exi_cb: ExiCallback) -> ExiCallback {
    if !exi_runtime_enabled() {
        let _ = (chn, exi_cb);
        return None;
    }
    if !chan_valid(chn) {
        return None;
    }
    let _guard = EXI_LOCK.lock();
    let bus = unsafe { &mut EXI_BUS[chn as usize] };
    let old = bus.cb_exi;
    if exi_cb.is_none() || interrupts_ready() {
        bus.cb_exi = exi_cb;
    }
    #[cfg(target_arch = "powerpc")]
    if chn == EXI_CHANNEL_2 {
        let exi0 = unsafe { &EXI_BUS[EXI_CHANNEL_0 as usize] };
        refresh_exi_interrupt(EXI_CHANNEL_0 as usize, exi0);
    } else {
        refresh_exi_interrupt(chn as usize, bus);
    }
    old
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);
    static CALLBACK_CHAN: AtomicI32 = AtomicI32::new(-1);
    static CALLBACK_DEV: AtomicI32 = AtomicI32::new(-1);

    struct TestGuard;

    impl Drop for TestGuard {
        fn drop(&mut self) {
            TEST_LOCK.store(false, Ordering::Release);
        }
    }

    unsafe extern "C" fn cb_a(chn: i32, dev: i32) -> i32 {
        CALLBACK_CHAN.store(chn, Ordering::SeqCst);
        CALLBACK_DEV.store(dev, Ordering::SeqCst);
        0
    }

    unsafe extern "C" fn cb_b(_: i32, _: i32) -> i32 {
        0
    }

    fn reset_host_state() {
        unsafe {
            EXI_REGS = [0; 15];
            for bus in match core::ptr::addr_of_mut!(EXI_BUS).as_mut() {
                Some(bus) => bus,
                None => return,
            } {
                *bus = ExiBus::new();
            }
        }
        CALLBACK_CHAN.store(-1, Ordering::SeqCst);
        CALLBACK_DEV.store(-1, Ordering::SeqCst);
    }

    fn test_guard() -> TestGuard {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        reset_host_state();
        TestGuard
    }

    #[test]
    fn exports_exist() {
        let _guard = test_guard();
        let _ = probe as fn(i32) -> i32;
        let _ = probe_ex as fn(i32) -> i32;
        let _ = lock as fn(i32, i32, ExiCallback) -> i32;
        let _ = unlock as fn(i32) -> i32;
        let _ = select as fn(i32, i32, i32) -> i32;
        let _ = select_sd as fn(i32, i32, i32) -> i32;
        let _ = deselect as fn(i32) -> i32;
        let _ = sync as fn(i32) -> i32;
        let _ = imm as unsafe fn(i32, *mut c_void, u32, i32, ExiCallback) -> i32;
        let _ = imm_ex as unsafe fn(i32, *mut c_void, u32, i32) -> i32;
        let _ = dma as unsafe fn(i32, *mut c_void, u32, i32, ExiCallback) -> i32;
        let _ = get_state as fn(i32) -> i32;
        let _ = get_id as unsafe fn(i32, i32, *mut u32) -> i32;
        let _ = attach as fn(i32, ExiCallback) -> i32;
        let _ = detach as fn(i32) -> i32;
        let _ = probe_reset as fn();
        let _ = register_exi_callback as fn(i32, ExiCallback) -> ExiCallback;
    }

    #[test]
    fn init_clears_channel_state_and_probe_reset_preserves_it() {
        let _guard = test_guard();
        init();
        assert_eq!(get_state(EXI_CHANNEL_0), 0);
        assert_eq!(get_state(EXI_CHANNEL_1), 0);
        assert_eq!(get_state(EXI_CHANNEL_2), 0);

        assert_eq!(attach(EXI_CHANNEL_0, Some(cb_a)), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_0, None), 1);
        assert_ne!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_LOCKED, 0);
        probe_reset();
        assert_ne!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_LOCKED, 0);
    }

    #[test]
    fn lock_select_deselect_and_unlock_follow_host_compat_state() {
        let _guard = test_guard();
        assert_eq!(select(EXI_CHANNEL_0, EXI_DEVICE_0, EXI_SPEED1MHZ), 0);
        assert_eq!(attach(EXI_CHANNEL_0, None), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_0, Some(cb_a)), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_0, None), 0);
        assert_eq!(select(EXI_CHANNEL_0, EXI_DEVICE_0, EXI_SPEED8MHZ), 1);
        assert_ne!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_SELECT, 0);
        assert_eq!(select(EXI_CHANNEL_0, EXI_DEVICE_0, EXI_SPEED8MHZ), 0);
        assert_eq!(deselect(EXI_CHANNEL_0), 1);
        assert_eq!(unlock(EXI_CHANNEL_0), 1);
        assert_eq!(CALLBACK_CHAN.load(Ordering::SeqCst), EXI_CHANNEL_0);
        assert_eq!(CALLBACK_DEV.load(Ordering::SeqCst), 0);
        assert_eq!(unlock(EXI_CHANNEL_0), 0);
    }

    #[test]
    fn lock_queues_one_unlock_callback_per_waiting_device() {
        let _guard = test_guard();
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_0, None), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_1, Some(cb_a)), 0);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_1, Some(cb_a)), 0);
        assert_eq!(unlock(EXI_CHANNEL_0), 1);
        assert_eq!(CALLBACK_CHAN.load(Ordering::SeqCst), EXI_CHANNEL_0);
        assert_eq!(CALLBACK_DEV.load(Ordering::SeqCst), EXI_DEVICE_1);
    }

    #[test]
    fn probe_ex_preserves_unknown_state() {
        assert_eq!(probe_ex_result(1, false), 1);
        assert_eq!(probe_ex_result(0, true), 0);
        assert_eq!(probe_ex_result(0, false), -1);
    }

    #[test]
    fn exi_register_base_matches_console_model() {
        assert_eq!(exi_reg_base_for_model(0), EXI_REG_BASE_GAMECUBE);
        assert_eq!(exi_reg_base_for_model(1), EXI_REG_BASE_WII);
        assert_eq!(exi_reg_base_for_model(u32::MAX), EXI_REG_BASE_WII);
    }

    #[test]
    fn device_zero_probe_rejects_missing_or_pending_device() {
        assert!(!device_zero_probe_succeeds(0));
        assert!(!device_zero_probe_succeeds(EXI_EXT_IRQ));
        assert!(device_zero_probe_succeeds(EXI_EXT_BIT));
    }

    #[test]
    fn device_zero_probe_waits_twenty_microseconds() {
        let gc_ticks = probe_debounce_ticks(40_500_000);
        let wii_ticks = probe_debounce_ticks(60_750_000);
        assert_eq!(gc_ticks, 810);
        assert_eq!(wii_ticks, 1_215);
        assert!(!probe_debounce_elapsed(100, 100 + gc_ticks - 1, 40_500_000));
        assert!(probe_debounce_elapsed(100, 100 + gc_ticks, 40_500_000));
        assert!(!probe_debounce_elapsed(u64::MAX - 400, 408, 60_750_000));
        assert!(probe_debounce_elapsed(u64::MAX - 400, 815, 60_750_000));
    }

    #[test]
    fn select_sd_checks_unattached_device_zero_id() {
        assert!(select_sd_needs_id_check(EXI_CHANNEL_0, EXI_DEVICE_0, false));
        assert!(select_sd_needs_id_check(EXI_CHANNEL_1, EXI_DEVICE_0, false));
        assert!(!select_sd_needs_id_check(EXI_CHANNEL_0, EXI_DEVICE_0, true));
        assert!(!select_sd_needs_id_check(
            EXI_CHANNEL_0,
            EXI_DEVICE_1,
            false
        ));
        assert!(!select_sd_needs_id_check(
            EXI_CHANNEL_2,
            EXI_DEVICE_0,
            false
        ));
    }

    #[test]
    fn exi_imm_ex_empty_transfer_matches_c_failure_result() {
        assert!(!imm_ex_has_work(0));
        assert!(imm_ex_has_work(1));
    }

    #[test]
    fn deselect_reports_device_zero_probe_failure() {
        assert_eq!(deselect_result_after_probe(EXI_DEVICE_0, false), 0);
        assert_eq!(deselect_result_after_probe(EXI_DEVICE_0, true), 1);
        assert_eq!(deselect_result_after_probe(EXI_DEVICE_1, false), 1);
    }

    #[test]
    fn sync_requires_the_channel_to_remain_selected() {
        assert!(!has_selected_transfer(EXI_FLAG_IMM));
        assert!(!has_selected_transfer(EXI_FLAG_DMA));
        assert!(has_selected_transfer(EXI_FLAG_SELECT | EXI_FLAG_IMM));
        assert!(has_selected_transfer(EXI_FLAG_SELECT | EXI_FLAG_DMA));
    }

    #[test]
    fn invalid_channel_and_device_are_rejected() {
        let _guard = test_guard();
        assert_eq!(lock(-1, EXI_DEVICE_0, None), 0);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_MAX, None), 0);
        assert_eq!(select(EXI_CHANNEL_MAX, EXI_DEVICE_0, EXI_SPEED1MHZ), 0);
        assert_eq!(deselect(EXI_CHANNEL_MAX), 0);
        assert_eq!(sync(EXI_CHANNEL_MAX), 0);
        let mut id = 0;
        // BUG FIX: Pass valid writable storage to the unsafe API, even though
        // the invalid channel causes it to return before writing the output.
        assert_eq!(unsafe { get_id(EXI_CHANNEL_MAX, EXI_DEVICE_0, &mut id) }, 0);
    }

    #[test]
    fn immediate_and_dma_transfer_state_completes_on_sync() {
        let _guard = test_guard();
        assert_eq!(attach(EXI_CHANNEL_0, None), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_0, None), 1);
        assert_eq!(select(EXI_CHANNEL_0, EXI_DEVICE_0, EXI_SPEED1MHZ), 1);

        let mut write_buf = [0x12, 0x34, 0x56, 0x78];
        assert_eq!(
            unsafe {
                imm(
                    EXI_CHANNEL_0,
                    write_buf.as_mut_ptr().cast(),
                    4,
                    EXI_WRITE as i32,
                    None,
                )
            },
            1
        );
        assert_ne!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_IMM, 0);
        assert_eq!(sync(EXI_CHANNEL_0), 1);
        assert_eq!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_IMM, 0);

        unsafe {
            write(4, 0xaabb_ccdd);
        }
        let mut read = [0u8; 4];
        assert_eq!(
            unsafe {
                imm(
                    EXI_CHANNEL_0,
                    read.as_mut_ptr().cast(),
                    4,
                    EXI_READ as i32,
                    None,
                )
            },
            1
        );
        assert_eq!(sync(EXI_CHANNEL_0), 1);
        assert_eq!(read, [0xaa, 0xbb, 0xcc, 0xdd]);

        let mut dma_buf = [0u8; 32];
        assert_eq!(
            unsafe {
                dma(
                    EXI_CHANNEL_0,
                    dma_buf.as_mut_ptr().cast(),
                    dma_buf.len() as u32,
                    EXI_READ as i32,
                    None,
                )
            },
            1
        );
        assert_ne!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_DMA, 0);
        assert_eq!(sync(EXI_CHANNEL_0), 1);
        assert_eq!(get_state(EXI_CHANNEL_0) as u32 & EXI_FLAG_DMA, 0);
    }

    #[test]
    fn callback_registration_does_not_attach_a_device() {
        let _guard = test_guard();
        assert!(register_exi_callback(EXI_CHANNEL_0, Some(cb_a)).is_none());
        assert_eq!(
            register_exi_callback(EXI_CHANNEL_0, Some(cb_b)).map(|cb| cb as *const () as usize),
            Some(cb_a as *const () as usize)
        );
        assert!(register_exi_callback(EXI_CHANNEL_MAX, Some(cb_a)).is_none());

        let bus = ExiBus::new();
        assert!(!bus.attached);
    }

    #[test]
    fn device_presence_requires_the_external_device_bit() {
        assert!(!device_zero_present_in_register(0));
        assert!(!device_zero_present_in_register(0x0800));
        assert!(device_zero_present_in_register(0x1000));
        assert!(device_zero_present_in_register(0x1800));
    }

    #[test]
    fn interrupt_ids_map_to_the_expected_channels() {
        assert_eq!(irq_channel(IRQ_EXI0_EXI, IRQ_EXI0_EXI), Some(0));
        assert_eq!(irq_channel(IRQ_EXI1_EXI, IRQ_EXI0_EXI), Some(1));
        assert_eq!(irq_channel(IRQ_EXI2_EXI, IRQ_EXI0_EXI), Some(2));
        assert_eq!(irq_channel(IRQ_EXI0_TC, IRQ_EXI0_TC), Some(0));
        assert_eq!(irq_channel(IRQ_EXI1_TC, IRQ_EXI0_TC), Some(1));
        assert_eq!(irq_channel(IRQ_EXI2_TC, IRQ_EXI0_TC), Some(2));
        assert_eq!(irq_channel(IRQ_EXI0_EXT, IRQ_EXI0_EXI), None);
    }

    #[test]
    fn transfer_completion_copies_data_and_returns_callback() {
        let mut output = [0u8; 4];
        let mut bus = ExiBus::new();
        bus.flags = EXI_FLAG_SELECT | EXI_FLAG_IMM;
        bus.imm_buff = output.as_mut_ptr().cast();
        bus.imm_len = output.len() as u32;
        bus.cb_tc = Some(cb_a);

        let callback = unsafe { complete_transfer(&mut bus, 0x1234_5678) };

        assert_eq!(output, [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(bus.flags, EXI_FLAG_SELECT);
        assert!(bus.imm_buff.is_null());
        assert_eq!(bus.imm_len, 0);
        assert_eq!(
            callback.map(|callback| callback as usize),
            Some(cb_a as usize)
        );
    }

    #[test]
    fn detach_refuses_locked_non_device_zero_channels() {
        let _guard = test_guard();
        assert_eq!(attach(EXI_CHANNEL_0, None), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_1, None), 1);
        assert_eq!(detach(EXI_CHANNEL_0), 0);
        assert_eq!(unlock(EXI_CHANNEL_0), 1);
        assert_eq!(lock(EXI_CHANNEL_0, EXI_DEVICE_0, None), 1);
        assert_eq!(select(EXI_CHANNEL_0, EXI_DEVICE_0, EXI_SPEED1MHZ), 1);
    }

    #[test]
    fn attach_is_rejected_when_the_channel_is_already_attached() {
        let _guard = test_guard();
        assert_eq!(attach(EXI_CHANNEL_0, None), 1);
        assert_eq!(attach(EXI_CHANNEL_0, Some(cb_a)), 0);
        assert_eq!(detach(EXI_CHANNEL_0), 1);
        assert_eq!(attach(EXI_CHANNEL_0, Some(cb_a)), 1);
    }
}
