//! Native DSP task and mailbox protocol from libaesnd.

use super::*;
use core::mem::MaybeUninit;
use ogc_dsp_sys as dsp;

const PB_BYTES: u32 = 64;
const DSP_DRAM_BYTES: usize = 8192;
const MAIL_TIMEOUT_TICKS: u64 = lwp::TICKS_PER_SECOND / 10;
const RESET_TIMEOUT_TICKS: u64 = lwp::TICKS_PER_SECOND;
static CODE: DmaAligned<[u8; 1000]> = DmaAligned(*include_bytes!("../dsp/dspmixer.bin"));

struct Hardware {
    task: UnsafeCell<MaybeUninit<dsp::DspTask>>,
    dram: UnsafeCell<DmaAligned<[u8; DSP_DRAM_BYTES]>>,
}
// SAFETY: Task and DRAM storage have permanent addresses. The DSP scheduler
// owns the task from add_task through the done interrupt; CPU initialization is
// serialized by AESND_STATE and is forbidden until that task has completed.
unsafe impl Sync for Hardware {}
static HARDWARE: Hardware = Hardware {
    task: UnsafeCell::new(MaybeUninit::uninit()),
    dram: UnsafeCell::new(DmaAligned([0; DSP_DRAM_BYTES])),
};

#[derive(Debug)]
pub(super) enum Error {
    MailTimeout,
    TaskRejected,
    ResetPending,
    #[cfg(gamecube)]
    AramUnavailable,
    #[cfg(gamecube)]
    AramBusy,
}

fn report(error: &Error) {
    ogc_system::report(format_args!("AESND DSP operation failed: {error:?}\n"));
}

fn send(mail: u32) -> Result<(), Error> {
    dsp::send_mail_to(mail);
    let started = lwp::now_ticks();
    while dsp::check_mail_to() != 0 {
        if lwp::now_ticks().wrapping_sub(started) >= MAIL_TIMEOUT_TICKS {
            return Err(Error::MailTimeout);
        }
        core::hint::spin_loop();
    }
    Ok(())
}

/// Copy only the voice-owned fields. Output, padding and the global audio
/// callback belong to the persistent command, as in __aesndcopycommand.
fn copy_command(dst: &mut Aesndpb, src: &Aesndpb) {
    let (out, pad, callback, arg) = (
        dst.out_buf,
        dst.pad,
        dst.audio_callback,
        dst.audio_callback_arg,
    );
    *dst = *src;
    dst.out_buf = out;
    dst.pad = pad;
    dst.audio_callback = callback;
    dst.audio_callback_arg = arg;
}

fn command_ptr(state: &mut AesndState) -> *mut Aesndpb {
    &mut state.command.0
}

fn select_voice(start: usize) -> Option<(usize, VoiceIdentity)> {
    let callback;
    let selected;
    {
        let mut state = AESND_STATE.lock();
        if !state.initialized || state.stopping {
            return None;
        }
        let index = (start..MAX_VOICES as usize).find(|&index| {
            let flags = state.voicepb[index].flags;
            flags & VOICE_USED != 0 && flags & VOICE_STOPPED == 0
        })?;
        let identity = VoiceIdentity {
            session: state.session,
            generation: state.voice_generation[index],
        };
        let voice = state.voicepb[index];
        copy_command(&mut state.command, &voice);
        state.current_voice = Some((index, identity));
        let legacy = state.legacy_voice_callbacks[index];
        callback = voice_callback_for(&mut state.command, legacy, VOICE_STATE_RUNNING);
        selected = (index, identity);
    }
    if let Some(callback) = callback {
        unsafe { callback.invoke() };
    }
    if voice_identity_is_current(selected.0, selected.1) {
        Some(selected)
    } else {
        select_voice(selected.0 + 1)
    }
}

fn dispatch_voice(mail: u32) -> Result<(), Error> {
    let mut state = AESND_STATE.lock();
    if state.stopping || !state.initialized {
        return Err(Error::ResetPending);
    }
    unsafe { dc_flush_range(command_ptr(&mut state).cast(), PB_BYTES) };
    if mail == 0xface_0010 && !state.process.begin(lwp::now_ticks()) {
        return Err(Error::TaskRejected);
    }
    drop(state);
    send(mail)
}

unsafe extern "C" fn task_init(_task: *mut dsp::DspTask) {
    let address = {
        let mut state = AESND_STATE.lock();
        (command_ptr(&mut state) as usize as u32) & !SYS_BASE_UNCACHED
    };
    let result = send(0xface_0080).and_then(|()| send(address));
    match result {
        Ok(()) => AESND_STATE.lock().process.task_ready(),
        Err(error) => report(&error),
    }
}

unsafe extern "C" fn task_done(_task: *mut dsp::DspTask) {
    let mut state = AESND_STATE.lock();
    state.process.task_done();
    state.current_voice = None;
    state.stopping = false;
    state.initialized = false;
}

unsafe extern "C" fn task_request(_task: *mut dsp::DspTask) {
    {
        let mut state = AESND_STATE.lock();
        if state.stopping {
            return;
        }
        if state.process.output_requested() {
            if !state.process.complete(lwp::now_ticks()) {
                report(&Error::TaskRejected);
            }
            let index = state.currab;
            unsafe {
                ogc_cache::dc_invalidate_range(
                    state.audio_buffer[index].as_mut_ptr().cast(),
                    SND_BUFFERSIZE,
                )
            };
            state.current_voice = None;
            return;
        }
        if !state.process.is_active() {
            report(&Error::TaskRejected);
            return;
        }
        unsafe { ogc_cache::dc_invalidate_range(command_ptr(&mut state).cast(), PB_BYTES) };
    }
    let current = AESND_STATE.lock().current_voice;
    let Some((index, identity)) = current else {
        report(&Error::TaskRejected);
        return;
    };
    if AESND_STATE.lock().command.flags & VOICE_FINISHED == 0 {
        return;
    }
    AESND_STATE.lock().command.flags &= !VOICE_FINISHED;
    service_voice_request(index, identity);
    {
        let mut state = AESND_STATE.lock();
        if state.stopping {
            return;
        }
        if state.session == identity.session && state.voice_generation[index] == identity.generation
        {
            let command = state.command.0;
            copy_command(&mut state.voicepb[index], &command);
        }
    }
    let result = if select_voice(index + 1).is_some() {
        dispatch_voice(0xface_0020)
    } else {
        AESND_STATE.lock().process.request_output();
        send(0xface_0100)
    };
    if let Err(error) = result {
        report(&error);
    }
}

unsafe extern "C" fn audio_dma_callback() {
    let (ptr, callback, paused, ready) = {
        let mut state = AESND_STATE.lock();
        if !state.initialized || state.stopping {
            return;
        }
        state.currab ^= 1;
        let index = state.currab;
        let ptr = if state.globalpause || state.voicesstopped {
            state.mute_buffer.as_mut_ptr()
        } else {
            state.audio_buffer[index].as_mut_ptr()
        };
        (
            ptr,
            audio_callback_for(&state),
            state.globalpause,
            state.process.is_ready(),
        )
    };
    if let Some(callback) = callback {
        unsafe { callback.invoke(ptr.cast(), SND_BUFFERSIZE) };
    }
    // CPU callback writes must reach RAM before AI DMA. DSP output is already
    // in RAM; invalidation in task_request prevents stale CPU data overwrites.
    unsafe { dc_flush_range(ptr.cast(), SND_BUFFERSIZE) };
    ogc_audio::init_dma(ptr as usize as u32, SND_BUFFERSIZE);
    if paused || !ready {
        return;
    }
    if select_voice(0).is_none() {
        let mut state = AESND_STATE.lock();
        state.voicesstopped = true;
        state.process.idle();
        return;
    }
    {
        let mut state = AESND_STATE.lock();
        state.voicesstopped = false;
        let index = state.currab;
        state.command.out_buf =
            (state.audio_buffer[index].as_ptr() as usize as u32) & !SYS_BASE_UNCACHED;
    }
    if let Err(error) = dispatch_voice(0xface_0010) {
        report(&error);
    }
}

pub(super) fn init() -> Result<(), Error> {
    dsp::init();
    ogc_audio::init(None);
    ogc_audio::stop_dma();
    ogc_audio::set_dsp_sample_rate(AI_SAMPLERATE_48KHZ as u8);
    let mut state = AESND_STATE.lock();
    if state.stopping {
        return Err(Error::ResetPending);
    }
    if !state.initialized {
        #[cfg(gamecube)]
        aram::initialize()?;
        replace_state(&mut state, true);
        for buffer in &mut state.audio_buffer {
            unsafe { dc_flush_range(buffer.as_mut_ptr().cast(), SND_BUFFERSIZE) };
        }
        unsafe { dc_flush_range(state.mute_buffer.as_mut_ptr().cast(), SND_BUFFERSIZE) };
        unsafe { dc_flush_range(CODE.0.as_ptr().cast_mut().cast(), CODE.0.len() as u32) };
        let task = HARDWARE.task.get().cast::<dsp::DspTask>();
        unsafe {
            task.write(dsp::DspTask::default());
            (*task).prio.write(255);
            (*task).iram_maddr =
                ((CODE.0.as_ptr() as usize as u32) & !SYS_BASE_UNCACHED) as usize as *mut c_void;
            (*task).iram_len = CODE.0.len() as u32;
            (*task).dram_maddr = ((HARDWARE.dram.get() as usize as u32) & !SYS_BASE_UNCACHED)
                as usize as *mut c_void;
            (*task).dram_len = DSP_DRAM_BYTES as u32;
            (*task).init_vec = 0x10;
            (*task).init_cb = Some(task_init);
            (*task).res_cb = None;
            (*task).req_cb = Some(task_request);
            (*task).done_cb = Some(task_done);
            dc_flush_range(HARDWARE.dram.get().cast(), DSP_DRAM_BYTES as u32);
        }
        drop(state);
        if unsafe { dsp::add_task(task) }.is_null() {
            AESND_STATE.lock().initialized = false;
            return Err(Error::TaskRejected);
        }
        state = AESND_STATE.lock();
    }
    let index = state.currab;
    let ptr = state.audio_buffer[index].as_ptr() as usize as u32;
    drop(state);
    ogc_audio::register_dma_callback(Some(audio_dma_callback));
    ogc_audio::init_dma(ptr, SND_BUFFERSIZE);
    ogc_audio::start_dma();
    Ok(())
}

pub(super) fn reset() -> Result<(), Error> {
    let can_wait_for_interrupt = unsafe { ppc_mfmsr() } & MSR_EE != 0;
    {
        let mut state = AESND_STATE.lock();
        if !state.initialized && !state.stopping {
            return Ok(());
        }
        state.stopping = true;
    }
    ogc_audio::stop_dma();
    ogc_audio::register_dma_callback(None);
    send(0xface_dead)?;
    if !can_wait_for_interrupt {
        return Err(Error::ResetPending);
    }
    let started = lwp::now_ticks();
    while AESND_STATE.lock().stopping {
        if lwp::now_ticks().wrapping_sub(started) >= RESET_TIMEOUT_TICKS {
            return Err(Error::ResetPending);
        }
        core::hint::spin_loop();
    }
    #[cfg(gamecube)]
    aram::release()?;
    Ok(())
}

// GameCube stream addresses are ARAM addresses; Wii accelerator addresses are
// physical MRAM. The same DSP command and request engine serves both targets.
#[cfg(gamecube)]
mod aram {
    use super::*;
    use ogc_aram::arq;
    struct Storage {
        sizes: UnsafeCell<[u32; MAX_VOICES as usize]>,
        blocks: UnsafeCell<[u32; MAX_VOICES as usize]>,
        requests: UnsafeCell<[arq::ArqRequest; MAX_VOICES as usize]>,
        identities: UnsafeCell<[Option<VoiceIdentity>; MAX_VOICES as usize]>,
        pending: [core::sync::atomic::AtomicBool; MAX_VOICES as usize],
        reserved: core::sync::atomic::AtomicBool,
    }
    // SAFETY: Permanent request and transfer storage; init/reset and request
    // submission are serialized by AESND critical sections and ARQ callbacks.
    unsafe impl Sync for Storage {}
    static STORAGE: Storage = Storage {
        sizes: UnsafeCell::new([0; MAX_VOICES as usize]),
        blocks: UnsafeCell::new([0; MAX_VOICES as usize]),
        requests: UnsafeCell::new([const { arq::ArqRequest::new() }; MAX_VOICES as usize]),
        identities: UnsafeCell::new([None; MAX_VOICES as usize]),
        pending: [const { core::sync::atomic::AtomicBool::new(false) }; MAX_VOICES as usize],
        reserved: core::sync::atomic::AtomicBool::new(false),
    };
    pub(super) fn initialize() -> Result<(), Error> {
        if STORAGE.reserved.load(core::sync::atomic::Ordering::Acquire) {
            return release();
        }
        // AR's stack allocator cannot safely attach a second allocation table.
        if ogc_aram::check_init() != 0 {
            return Err(Error::AramUnavailable);
        }
        let base = ogc_aram::init(STORAGE.sizes.get().cast(), MAX_VOICES);
        if base == 0 || ogc_aram::get_size() < base + MAX_VOICES * DSP_STREAMBUFFER_SIZE * 2 {
            return Err(Error::AramUnavailable);
        }
        arq::init();
        for index in 0..MAX_VOICES as usize {
            unsafe { (*STORAGE.blocks.get())[index] = ogc_aram::alloc(DSP_STREAMBUFFER_SIZE * 2) };
        }
        STORAGE
            .reserved
            .store(true, core::sync::atomic::Ordering::Release);
        Ok(())
    }
    pub(super) fn address(index: usize) -> u32 {
        unsafe { (*STORAGE.blocks.get())[index] }
    }
    pub(super) fn stage(
        state: &mut AesndState,
        index: usize,
        offset: usize,
        size: u32,
    ) -> Result<(), Error> {
        let request = unsafe { STORAGE.requests.get().cast::<arq::ArqRequest>().add(index) };
        if STORAGE.pending[index].swap(true, core::sync::atomic::Ordering::AcqRel) {
            return Err(Error::AramBusy);
        }
        unsafe {
            (*STORAGE.identities.get())[index] = Some(VoiceIdentity {
                session: state.session,
                generation: state.voice_generation[index],
            });
        }
        let ptr = unsafe { state.stream_buffer[index].as_ptr().add(offset) };
        unsafe {
            arq::post_request_async(
                request,
                arq::ArqTransfer {
                    owner: index as u32,
                    dir: arq::ARQ_MRAMTOARAM,
                    prio: arq::ARQ_PRIO_HI,
                    aram_addr: address(index) + offset as u32,
                    mram_addr: (ptr as usize as u32) & !SYS_BASE_UNCACHED,
                    len: size,
                },
                Some(staged),
            );
        }
        Ok(())
    }
    unsafe extern "C" fn staged(request: *mut arq::ArqRequest) {
        let mut state = AESND_STATE.lock();
        let index = unsafe { (*request).owner as usize };
        if index >= MAX_VOICES as usize {
            report(&Error::AramUnavailable);
            return;
        }
        STORAGE.pending[index].store(false, core::sync::atomic::Ordering::Release);
        if state.initialized
            && unsafe { (*STORAGE.identities.get())[index] }.is_some_and(|identity| {
                state.session == identity.session
                    && state.voice_generation[index] == identity.generation
            })
        {
            mark_aram_voice_running(&mut state, index);
        }
    }
    pub(super) fn release() -> Result<(), Error> {
        if STORAGE
            .pending
            .iter()
            .any(|pending| pending.load(core::sync::atomic::Ordering::Acquire))
        {
            return Err(Error::AramBusy);
        }
        // Keep this fixed reservation across reset. AR_Free is a global LIFO
        // pop and could release another client's newer allocation. Permanent
        // staging also protects an outstanding DMA on a reset failure.
        Ok(())
    }
}
pub(super) fn staging_address(state: &AesndState, index: usize) -> u32 {
    #[cfg(gamecube)]
    {
        let _ = state;
        aram::address(index)
    }
    #[cfg(not(gamecube))]
    {
        (state.stream_buffer[index].as_ptr() as usize as u32) & !SYS_BASE_UNCACHED
    }
}

pub(super) fn publish_staging(state: &mut AesndState, index: usize, offset: usize, size: u32) {
    let ptr = unsafe { state.stream_buffer[index].as_mut_ptr().add(offset) };
    unsafe { dc_flush_range(ptr.cast(), size) };
    #[cfg(gamecube)]
    if let Err(error) = aram::stage(state, index, offset, size) {
        request_voice_mut(state, index).flags |= VOICE_STOPPED;
        report(&error);
    }
}
