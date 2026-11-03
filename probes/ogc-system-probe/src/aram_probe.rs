//! Probe ARAM, ARQ, and ARQM APIs with aligned caller-owned memory.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::probe_support::ProbeLogger;

const TEST_BYTES: usize = 32;
const ACTIVE_DMA_BYTES: usize = 32 * 1024;
const DMA_WAIT_FRAMES: usize = 240;
const OWNER_REMOVED: u32 = 0x4152_5158;
const OWNER_KEPT: u32 = 0x4152_5159;

#[repr(align(32))]
struct Aligned([u8; TEST_BYTES]);

static mut WRITE_BUFFER: Aligned = Aligned([0x31; TEST_BYTES]);
static mut READ_BUFFER: Aligned = Aligned([0; TEST_BYTES]);
#[repr(align(32))]
struct LargeAligned([u8; ACTIVE_DMA_BYTES]);
static mut ACTIVE_BUFFER: LargeAligned = LargeAligned([0x57; ACTIVE_DMA_BYTES]);
static CALLBACKS: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn dma_callback() {
    CALLBACKS.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn arq_callback(_request: *mut ogc_aram::arq::ArqRequest) {
    CALLBACKS.fetch_add(1, Ordering::AcqRel);
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("ARAM AND ARQ API PROBE");

    let mut allocation_stack = [0_u32; 4];
    let base = ogc_aram::init(allocation_stack.as_mut_ptr(), allocation_stack.len() as u32);
    let internal_size = ogc_aram::get_internal_size();
    logger.record(
        "ARAM init reports an initialized user region",
        base == ogc_aram::get_base_address()
            && base == 0x4000
            && ogc_aram::check_init() != 0
            && internal_size > base
            && ogc_aram::get_size() >= internal_size,
    );
    logger.record(
        "ARAM DMA status is idle after initialization",
        ogc_aram::get_dma_status() == 0,
    );

    let first = ogc_aram::alloc(0x100);
    let second = ogc_aram::alloc(0x200);
    let mut freed_len = 0;
    let after_free = ogc_aram::free(Some(&mut freed_len));
    let after_free_without_len = ogc_aram::free(None);
    logger.record(
        "ARAM allocation stack returns blocks in reverse order",
        first == base
            && second == base + 0x100
            && freed_len == 0x200
            && after_free == second
            && after_free_without_len == first,
    );

    CALLBACKS.store(0, Ordering::Release);
    let previous = ogc_aram::register_callback(Some(dma_callback));
    let replaced = ogc_aram::register_callback(Some(dma_callback));
    let callback_swap = previous.is_none() && replaced.is_some();
    let callback_mem = unsafe { aligned_ptr(core::ptr::addr_of_mut!(WRITE_BUFFER)) };
    ogc_aram::start_dma(
        ogc_aram::AR_MRAMTOARAM,
        callback_mem as usize as u32,
        base + 0x1000,
        TEST_BYTES as u32,
    );
    let dma_idle = wait_for_dma(logger) && ogc_aram::get_dma_status() == 0;
    let callback_done = dma_idle && wait_for_callbacks(logger, 1);
    let removed = ogc_aram::register_callback(None);
    logger.note(&alloc::format!(
        "ARAM callback previous={} replaced={} removed={} count={}",
        previous.is_some(),
        replaced.is_some(),
        removed.is_some(),
        CALLBACKS.load(Ordering::Acquire)
    ));
    logger.record(
        "ARAM callback replacement and DMA write completion are observable",
        callback_swap && callback_done,
    );

    ogc_aram::start_dma_read(
        unsafe { aligned_ptr(core::ptr::addr_of_mut!(READ_BUFFER)) } as usize as u32,
        base + 0x1000,
        TEST_BYTES as u32,
    );
    let read_done = wait_for_dma(logger);
    let source = unsafe { &(*core::ptr::addr_of!(WRITE_BUFFER)).0 };
    let copied = unsafe { &(*core::ptr::addr_of!(READ_BUFFER)).0 };
    logger.record(
        "ARAM DMA read returns the written bytes",
        read_done && source == copied,
    );

    ogc_aram::start_dma_write(
        callback_mem as usize as u32,
        base + 0x1040,
        TEST_BYTES as u32,
    );
    let write_done = wait_for_dma(logger);
    ogc_aram::start_dma_read(
        unsafe { aligned_ptr(core::ptr::addr_of_mut!(READ_BUFFER)) } as usize as u32,
        base + 0x1040,
        TEST_BYTES as u32,
    );
    let write_read_done = wait_for_dma(logger);
    let write_roundtrip = unsafe { &(*core::ptr::addr_of!(WRITE_BUFFER)).0 }
        == unsafe { &(*core::ptr::addr_of!(READ_BUFFER)).0 };
    logger.record(
        "ARAM DMA write wrapper roundtrips aligned bytes",
        write_done && write_read_done && write_roundtrip,
    );
    ogc_aram::clear(ogc_aram::AR_ARAMINTUSER);
    ogc_aram::start_dma_read(
        unsafe { aligned_ptr(core::ptr::addr_of_mut!(READ_BUFFER)) } as usize as u32,
        base + 0x1040,
        TEST_BYTES as u32,
    );
    let clear_read_done = wait_for_dma(logger);
    let zeroed = unsafe { &(*core::ptr::addr_of!(READ_BUFFER)).0 }
        .iter()
        .all(|byte| *byte == 0);
    logger.record(
        "ARAM user clear zeroes the selected region",
        clear_read_done && zeroed,
    );

    let request = ogc_aram::arq::ArqRequest::new();
    logger.record(
        "ARQ request constructor initializes a ready empty request",
        request.state == ogc_aram::arq::ARQ_TASK_READY
            && request.node.next.is_null()
            && request.node.prev.is_null()
            && request.callback.is_none(),
    );
    let mut sync_before_init = ogc_aram::arq::ArqRequest::new();
    unsafe {
        ogc_aram::arq::post_request(
            &mut sync_before_init,
            transfer(OWNER_KEPT, base + 0x2000, callback_mem),
        );
    }
    let mut async_before_init = ogc_aram::arq::ArqRequest::new();
    unsafe {
        ogc_aram::arq::post_request_async(
            &mut async_before_init,
            transfer(OWNER_KEPT, base + 0x2000, callback_mem),
            Some(arq_callback),
        );
    }
    logger.record(
        "ARQ requests before initialization stay ready and do not start DMA",
        sync_before_init.state == ogc_aram::arq::ARQ_TASK_READY
            && async_before_init.state == ogc_aram::arq::ARQ_TASK_READY
            && ogc_aram::get_dma_status() == 0,
    );
    ogc_aram::arq::init();
    let default_chunk = ogc_aram::arq::chunk_size();
    ogc_aram::arq::set_chunk_size(65);
    let rounded_chunk = ogc_aram::arq::chunk_size();
    ogc_aram::arq::set_chunk_size(4096);
    logger.record(
        "ARQ init and chunk size use the default and round to 32 bytes",
        default_chunk == ogc_aram::arq::ARQ_DEF_CHUNK_SIZE && rounded_chunk == 96,
    );

    CALLBACKS.store(0, Ordering::Release);
    let mut async_request = ogc_aram::arq::ArqRequest::new();
    unsafe {
        ogc_aram::arq::post_request_async(
            &mut async_request,
            ogc_aram::arq::ArqTransfer {
                owner: OWNER_KEPT,
                dir: ogc_aram::arq::ARQ_MRAMTOARAM,
                prio: ogc_aram::arq::ARQ_PRIO_HI,
                aram_addr: base + 0x2000,
                mram_addr: callback_mem as usize as u32,
                len: TEST_BYTES as u32,
            },
            Some(arq_callback),
        );
    }
    let async_done = wait_for_request(logger, &async_request)
        && async_request.state == ogc_aram::arq::ARQ_TASK_FINISHED
        && async_request.owner == OWNER_KEPT
        && CALLBACKS.load(Ordering::Acquire) == 1;
    logger.note(&alloc::format!(
        "ARQ async state={} callbacks={}",
        async_request.state,
        CALLBACKS.load(Ordering::Acquire)
    ));
    logger.record(
        "ARQ async request calls back only after DMA completion",
        async_done,
    );

    let mut sync_request = ogc_aram::arq::ArqRequest::new();
    unsafe {
        ogc_aram::arq::post_request(
            &mut sync_request,
            ogc_aram::arq::ArqTransfer {
                owner: OWNER_KEPT,
                dir: ogc_aram::arq::ARQ_MRAMTOARAM,
                prio: ogc_aram::arq::ARQ_PRIO_HI,
                aram_addr: base + 0x2040,
                mram_addr: callback_mem as usize as u32,
                len: TEST_BYTES as u32,
            },
        );
    }
    logger.record(
        "ARQ synchronous request returns after its request is finished",
        sync_request.state == ogc_aram::arq::ARQ_TASK_FINISHED,
    );

    let mut active = ogc_aram::arq::ArqRequest::new();
    let mut queued_remove = ogc_aram::arq::ArqRequest::new();
    let mut queued_owner = ogc_aram::arq::ArqRequest::new();
    let mut queued_flush = ogc_aram::arq::ArqRequest::new();
    unsafe {
        ogc_aram::arq::post_request_async(
            &mut active,
            ogc_aram::arq::ArqTransfer {
                owner: OWNER_KEPT,
                dir: ogc_aram::arq::ARQ_MRAMTOARAM,
                prio: ogc_aram::arq::ARQ_PRIO_HI,
                aram_addr: base + 0x4000,
                mram_addr: active_buffer_ptr() as usize as u32,
                len: ACTIVE_DMA_BYTES as u32,
            },
            Some(arq_callback),
        );
        ogc_aram::arq::post_request_async(
            &mut queued_remove,
            transfer(OWNER_KEPT, base + 0x20c0, callback_mem),
            Some(arq_callback),
        );
        ogc_aram::arq::post_request_async(
            &mut queued_owner,
            transfer(OWNER_REMOVED, base + 0x2100, callback_mem),
            Some(arq_callback),
        );
        ogc_aram::arq::post_request_async(
            &mut queued_flush,
            transfer(OWNER_KEPT, base + 0x2140, callback_mem),
            Some(arq_callback),
        );
    }
    unsafe { ogc_aram::arq::remove_request(&mut queued_remove) };
    let removed_owners = ogc_aram::arq::remove_owner_request(OWNER_REMOVED);
    ogc_aram::arq::flush_queue();
    let queue_removals = queued_remove.state == ogc_aram::arq::ARQ_TASK_READY
        && queued_owner.state == ogc_aram::arq::ARQ_TASK_READY
        && queued_flush.state == ogc_aram::arq::ARQ_TASK_READY
        && removed_owners == 1;
    let active_done =
        wait_for_request(logger, &active) && active.state == ogc_aram::arq::ARQ_TASK_FINISHED;
    logger.note(&alloc::format!(
        "ARQ removal active={} remove={} owner={} removed_owners={} active_done={active_done}",
        active.state,
        queued_remove.state,
        queued_owner.state,
        removed_owners
    ));
    logger.record(
        "ARQ removes queued requests by address and owner, then flushes work",
        queue_removals && active_done,
    );

    let manager_base = base + 0x10000;
    ogc_aram::arq::manager::init(manager_base, 0x2000);
    let zero_address = ogc_aram::arq::manager::zero_buffer();
    let manager_free_before = ogc_aram::arq::manager::free_size();
    let manager_top_before = ogc_aram::arq::manager::stack_pointer();
    let pushed =
        unsafe { ogc_aram::arq::manager::push_data(callback_mem.cast(), TEST_BYTES as i32) };
    let manager_free_after = ogc_aram::arq::manager::free_size();
    let manager_top_after = ogc_aram::arq::manager::stack_pointer();
    ogc_aram::arq::manager::pop();
    logger.record(
        "ARQM pushes aligned data and restores its stack on pop",
        zero_address == manager_base
            && manager_free_before == 0x2000 - 256
            && manager_top_before == manager_base + 256
            && pushed == manager_top_before
            && manager_free_after == manager_free_before - 32
            && manager_top_after == manager_top_before + 32
            && ogc_aram::arq::manager::free_size() == manager_free_before
            && ogc_aram::arq::manager::stack_pointer() == manager_top_before,
    );
    ogc_aram::arq::manager::init(manager_base + 0x4000, 128);
    logger.record(
        "ARQM failed zero-buffer write leaves no allocatable state",
        ogc_aram::arq::manager::free_size() == 0
            && ogc_aram::arq::manager::zero_buffer() == 0
            && ogc_aram::arq::manager::stack_pointer() == 0,
    );

    ogc_aram::arq::reset();
    ogc_aram::arq::init();
    let mut after_arq_reset = ogc_aram::arq::ArqRequest::new();
    unsafe {
        ogc_aram::arq::post_request(
            &mut after_arq_reset,
            transfer(OWNER_KEPT, base + 0x2180, callback_mem),
        );
    }
    logger.record(
        "ARQ reset permits initialization and a later completed request",
        after_arq_reset.state == ogc_aram::arq::ARQ_TASK_FINISHED,
    );
    ogc_aram::arq::reset();
    ogc_aram::reset();
    let reset_state = ogc_aram::check_init() == 0;
    let reinit = ogc_aram::init(allocation_stack.as_mut_ptr(), allocation_stack.len() as u32);
    logger.record(
        "ARAM reset clears initialization and allows a fresh init",
        reset_state && reinit == base && ogc_aram::check_init() != 0,
    );
}

unsafe fn aligned_ptr(buffer: *mut Aligned) -> *mut u8 {
    unsafe { core::ptr::addr_of_mut!((*buffer).0).cast::<u8>() }
}

unsafe fn active_buffer_ptr() -> *mut u8 {
    unsafe { core::ptr::addr_of_mut!((*core::ptr::addr_of_mut!(ACTIVE_BUFFER)).0).cast() }
}

fn transfer(owner: u32, aram_addr: u32, buffer: *mut u8) -> ogc_aram::arq::ArqTransfer {
    ogc_aram::arq::ArqTransfer {
        owner,
        dir: ogc_aram::arq::ARQ_MRAMTOARAM,
        prio: ogc_aram::arq::ARQ_PRIO_HI,
        aram_addr,
        mram_addr: buffer as usize as u32,
        len: TEST_BYTES as u32,
    }
}

fn wait_for_dma(logger: &mut ProbeLogger) -> bool {
    for _ in 0..DMA_WAIT_FRAMES {
        if ogc_aram::get_dma_status() == 0 {
            return true;
        }
        logger.display().wait_vsync();
    }
    false
}

fn wait_for_request(logger: &mut ProbeLogger, request: &ogc_aram::arq::ArqRequest) -> bool {
    for _ in 0..DMA_WAIT_FRAMES {
        let state = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(request.state)) };
        if state == ogc_aram::arq::ARQ_TASK_FINISHED {
            return true;
        }
        logger.display().wait_vsync();
    }
    false
}

fn wait_for_callbacks(logger: &mut ProbeLogger, required: u32) -> bool {
    for _ in 0..DMA_WAIT_FRAMES {
        if CALLBACKS.load(Ordering::Acquire) >= required {
            return true;
        }
        logger.display().wait_vsync();
    }
    false
}
