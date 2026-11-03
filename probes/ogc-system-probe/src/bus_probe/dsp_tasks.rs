use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::probe_support::ProbeLogger;
use ogc_dsp as dsp;
use ogc_dsp_sys as dsp_sys;

const DSP_DRAM_BYTES: usize = 8192;
const DSP_PB_OFFSET: usize = 0x200;

#[repr(align(32))]
struct Align32<const N: usize>([u8; N]);

static DSP_CODE: Align32<1000> = Align32(*include_bytes!(
    "../../../../crates/media/aesnd-sys/dsp/dspmixer.bin"
));
static mut DSP_DRAM: Align32<DSP_DRAM_BYTES> = Align32([0; DSP_DRAM_BYTES]);
static mut DSP_TASK: MaybeUninit<dsp_sys::DspTask> = MaybeUninit::uninit();
static mut UNUSED_TASK: MaybeUninit<dsp_sys::DspTask> = MaybeUninit::uninit();
static TASK_INIT_CALLBACKS: AtomicU32 = AtomicU32::new(0);
static TASK_DONE_CALLBACKS: AtomicU32 = AtomicU32::new(0);
static TASK_MAIL_ERRORS: AtomicU32 = AtomicU32::new(0);

fn physical_address(address: usize) -> u32 {
    address as u32 & !0xc000_0000
}

fn send_mail(mail: u32) -> bool {
    dsp_sys::send_mail_to(mail);
    for _ in 0..1_000_000 {
        if dsp_sys::check_mail_to() == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

unsafe extern "C" fn task_init(_task: *mut dsp_sys::DspTask) {
    let dram = core::ptr::addr_of_mut!(DSP_DRAM).cast::<u8>();
    let parameter_block = unsafe { dram.add(DSP_PB_OFFSET) };
    let address = physical_address(parameter_block as usize);
    let handoff_ok = send_mail(0xface_0080) && send_mail(address);
    if !handoff_ok {
        TASK_MAIL_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
    TASK_INIT_CALLBACKS.fetch_add(1, Ordering::Release);
}

unsafe extern "C" fn task_done(_task: *mut dsp_sys::DspTask) {
    TASK_DONE_CALLBACKS.fetch_add(1, Ordering::Release);
}

fn wait_for_count(logger: &mut ProbeLogger, count: &AtomicU32, expected: u32) -> bool {
    for _ in 0..180 {
        if count.load(Ordering::Acquire) >= expected {
            return true;
        }
        logger.display().wait_vsync();
    }
    false
}

pub(super) fn probe_dsp_tasks(logger: &mut ProbeLogger) {
    logger.section("DSP TASK SCHEDULER PROBE");
    TASK_INIT_CALLBACKS.store(0, Ordering::Release);
    TASK_DONE_CALLBACKS.store(0, Ordering::Release);
    TASK_MAIL_ERRORS.store(0, Ordering::Release);

    let task = core::ptr::addr_of_mut!(DSP_TASK).cast::<dsp_sys::DspTask>();
    let unused = core::ptr::addr_of_mut!(UNUSED_TASK).cast::<dsp_sys::DspTask>();
    let dram = core::ptr::addr_of_mut!(DSP_DRAM).cast::<u8>();
    // SAFETY: these static task objects and aligned buffers live for the full
    // asynchronous DSP task lifetime. Their addresses stay stable.
    unsafe {
        task.write(dsp_sys::DspTask::default());
        unused.write(dsp_sys::DspTask::default());
        core::ptr::write_bytes(dram, 0, DSP_DRAM_BYTES);
        (*task).prio.write(128);
        (*task).iram_maddr = physical_address(DSP_CODE.0.as_ptr() as usize) as usize as *mut _;
        (*task).iram_len = DSP_CODE.0.len() as u32;
        (*task).iram_addr = 0;
        (*task).dram_maddr = physical_address(dram as usize) as usize as *mut _;
        (*task).dram_len = DSP_DRAM_BYTES as u32;
        (*task).dram_addr = 0;
        (*task).init_vec = 0x10;
        (*task).resume_vec = 0x10;
        (*task).init_cb = Some(task_init);
        (*task).done_cb = Some(task_done);
        (*task).res_cb = None;
        (*task).req_cb = None;
        ogc_cache::dc_flush_range(DSP_CODE.0.as_ptr().cast(), DSP_CODE.0.len() as u32);
        ogc_cache::dc_flush_range(dram.cast(), DSP_DRAM_BYTES as u32);
    }

    let idle_assert = unsafe { dsp_sys::assert_task(unused) }.is_null();
    let idle_assert_wrapper = unsafe { dsp::assert_task(unused) }.is_null();
    logger.record(
        "DSP task assert returns no task when the queue is empty",
        idle_assert && idle_assert_wrapper,
    );

    dsp_sys::reset();
    dsp_sys::unhalt();
    let mut boot_mail_ready = false;
    for _ in 0..180 {
        if dsp_sys::check_mail_from() != 0 {
            boot_mail_ready = true;
            break;
        }
        logger.display().wait_vsync();
    }
    logger.record(
        "DSP task reset exposes its boot mailbox before task scheduling",
        boot_mail_ready,
    );
    if !boot_mail_ready {
        return;
    }

    let added = unsafe { dsp_sys::add_task(task) } == task;
    let rejected_null = unsafe { dsp::add_task(core::ptr::null_mut()) }.is_null();
    let init_called = added && wait_for_count(logger, &TASK_INIT_CALLBACKS, 1);
    let initialized_state = unsafe {
        (*task).state.read() == dsp_sys::DSPTASK_RUN
            && (*task).flags.read() & dsp_sys::DSPTASK_ATTACH != 0
    };
    let handoff_ok = TASK_MAIL_ERRORS.load(Ordering::Acquire) == 0;
    logger.record(
        "DSP task direct add boots the AESND mixer fixture and hands off its parameter block",
        init_called && initialized_state && handoff_ok,
    );
    logger.record("DSP task wrapper add rejects a null task", rejected_null);

    unsafe {
        dsp_sys::cancel_task(task);
        dsp::cancel_task(task);
    }
    let cancel_flag = unsafe {
        (*task).flags.read() & dsp_sys::DSPTASK_CANCEL != 0
            && (*task).flags.read() & dsp_sys::DSPTASK_ATTACH != 0
    };
    logger.record(
        "DSP direct and wrapper cancel set the task cancellation flag",
        cancel_flag,
    );

    let terminated = send_mail(0xface_dead);
    let done_called = terminated && wait_for_count(logger, &TASK_DONE_CALLBACKS, 1);
    let removed = unsafe {
        (*task).state.read() == dsp_sys::DSPTASK_DONE
            && (*task).flags.read() == dsp_sys::DSPTASK_CLEARALL
    };
    logger.record(
        "DSP task completion callback removes the task and clears cancellation flags",
        done_called && removed && TASK_MAIL_ERRORS.load(Ordering::Acquire) == 0,
    );
}
