use core::task::{Context, Poll, Waker};
use ogc_lwp_driver::{LwpClock, LwpDeferredExecutor, LwpInterruptWake, LwpTimerService};
use ogc_runtime::driver::{
    AsyncOperation, BlockDevice, DebugTransport, HidTransport, Lifecycle, SubsystemState,
};
use ogc_runtime::{
    Deadline, DeferredExecutor, InterruptWake, MonotonicClock, ThreadId, TimerService, WakeToken,
    WiiError,
};
use ogc_system_probe::probe_support::ProbeLogger;

fn deferred_noop(_argument: usize) {}

fn probe_lwp_context_constructor(logger: &mut ProbeLogger) {
    let context = ogc_lwp_driver::LwpContext::zeroed();
    logger.record(
        "LWP context constructor zeroes all register state",
        context.pc == 0
            && context.msr == 0
            && context.cr == 0
            && context.lr == 0
            && context.ctr == 0
            && context.xer == 0
            && context.fpscr == 0
            && context.gpr.iter().all(|value| *value == 0)
            && context.fpr.iter().all(|value| *value == 0)
            && context.gqr.iter().all(|value| *value == 0)
            && context.ps.iter().all(|value| *value == 0),
    );
}

struct LifecycleContract {
    state: SubsystemState,
}

impl Lifecycle for LifecycleContract {
    fn state(&self) -> SubsystemState {
        self.state
    }

    fn initialize(&mut self) -> Result<(), WiiError> {
        self.state = SubsystemState::Ready;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), WiiError> {
        self.state = SubsystemState::Uninitialized;
        Ok(())
    }
}

struct BlockDeviceContract {
    bytes: [u8; 8],
    flushed: bool,
}

impl BlockDevice for BlockDeviceContract {
    fn block_size(&self) -> u32 {
        4
    }

    fn block_count(&self) -> u64 {
        2
    }

    fn read_blocks(&mut self, lba: u64, output: &mut [u8]) -> Result<(), WiiError> {
        let start = lba as usize * self.block_size() as usize;
        let end = start.saturating_add(output.len());
        if output.len() % self.block_size() as usize != 0 || end > self.bytes.len() {
            return Err(WiiError::InvalidArgument);
        }
        output.copy_from_slice(&self.bytes[start..end]);
        Ok(())
    }

    fn write_blocks(&mut self, lba: u64, input: &[u8]) -> Result<(), WiiError> {
        let start = lba as usize * self.block_size() as usize;
        let end = start.saturating_add(input.len());
        if input.len() % self.block_size() as usize != 0 || end > self.bytes.len() {
            return Err(WiiError::InvalidArgument);
        }
        self.bytes[start..end].copy_from_slice(input);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), WiiError> {
        self.flushed = true;
        Ok(())
    }
}

struct AsyncContract {
    cancelled: bool,
}

impl AsyncOperation for AsyncContract {
    type Output = u32;

    fn poll(&mut self, _context: &mut Context<'_>) -> Poll<Result<Self::Output, WiiError>> {
        if self.cancelled {
            Poll::Ready(Err(WiiError::Cancelled))
        } else {
            Poll::Ready(Ok(0x51a7))
        }
    }

    fn cancel(&mut self) -> Result<(), WiiError> {
        self.cancelled = true;
        Ok(())
    }
}

struct DebugTransportContract {
    opened: bool,
    bytes_written: usize,
}

impl DebugTransport for DebugTransportContract {
    fn open(&mut self) -> Result<(), WiiError> {
        self.opened = true;
        Ok(())
    }

    fn read(&mut self, output: &mut [u8]) -> Result<usize, WiiError> {
        if !self.opened {
            return Err(WiiError::NotInitialized);
        }
        let response = [0x2a, 0x7c, 0x91];
        let count = output.len().min(response.len());
        output[..count].copy_from_slice(&response[..count]);
        Ok(count)
    }

    fn write(&mut self, input: &[u8]) -> Result<usize, WiiError> {
        if !self.opened {
            return Err(WiiError::NotInitialized);
        }
        self.bytes_written = self.bytes_written.saturating_add(input.len());
        Ok(input.len())
    }

    fn close(&mut self) -> Result<(), WiiError> {
        self.opened = false;
        Ok(())
    }
}

struct HidTransportContract {
    sent: [u8; 4],
    sent_len: usize,
}

impl HidTransport for HidTransportContract {
    fn send_report(&mut self, report: &[u8]) -> Result<(), WiiError> {
        if report.len() > self.sent.len() {
            return Err(WiiError::InvalidArgument);
        }
        self.sent[..report.len()].copy_from_slice(report);
        self.sent_len = report.len();
        Ok(())
    }

    fn receive_report(&mut self, output: &mut [u8]) -> Result<usize, WiiError> {
        let response = [0xc1, 0x0d, 0x42];
        let count = output.len().min(response.len());
        output[..count].copy_from_slice(&response[..count]);
        Ok(count)
    }
}

fn probe_driver_trait_contracts(logger: &mut ProbeLogger) {
    logger.section("SHARED RUNTIME DRIVER TRAIT CONTRACTS");

    let mut lifecycle = LifecycleContract {
        state: SubsystemState::Uninitialized,
    };
    let initial_state = lifecycle.state();
    let initialized = lifecycle.initialize().is_ok();
    logger.record(
        "runtime Lifecycle state and initialize return expected states",
        initial_state == SubsystemState::Uninitialized
            && initialized
            && lifecycle.state() == SubsystemState::Ready,
    );
    let shutdown = lifecycle.shutdown().is_ok();
    logger.record(
        "runtime Lifecycle shutdown returns to uninitialized state",
        shutdown && lifecycle.state() == SubsystemState::Uninitialized,
    );

    let mut block = BlockDeviceContract {
        bytes: [0; 8],
        flushed: false,
    };
    logger.record(
        "runtime BlockDevice reports independent size and count",
        block.block_size() == 4 && block.block_count() == 2,
    );
    let write_ok = block.write_blocks(1, &[0x10, 0x20, 0x30, 0x40]).is_ok();
    let write_rejects_out_of_range =
        block.write_blocks(2, &[1, 2, 3, 4]) == Err(WiiError::InvalidArgument);
    logger.record(
        "runtime BlockDevice writes a block and rejects out-of-range writes",
        write_ok
            && write_rejects_out_of_range
            && block.bytes == [0, 0, 0, 0, 0x10, 0x20, 0x30, 0x40],
    );
    let mut read = [0; 4];
    let read_ok = block.read_blocks(1, &mut read).is_ok();
    let read_rejects_unaligned =
        block.read_blocks(0, &mut [0; 3]) == Err(WiiError::InvalidArgument);
    logger.record(
        "runtime BlockDevice reads the selected block and rejects invalid lengths",
        read_ok && read == [0x10, 0x20, 0x30, 0x40] && read_rejects_unaligned,
    );
    let flush_ok = block.flush().is_ok();
    logger.record(
        "runtime BlockDevice flush reports completion",
        flush_ok && block.flushed,
    );

    let mut operation = AsyncContract { cancelled: false };
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    logger.record(
        "runtime AsyncOperation poll returns its ready value",
        operation.poll(&mut context) == Poll::Ready(Ok(0x51a7)),
    );
    let cancel_ok = operation.cancel().is_ok();
    logger.record(
        "runtime AsyncOperation cancel changes the mock completion",
        cancel_ok && operation.poll(&mut context) == Poll::Ready(Err(WiiError::Cancelled)),
    );

    let mut debug = DebugTransportContract {
        opened: false,
        bytes_written: 0,
    };
    let open_ok = debug.open().is_ok();
    let mut debug_read = [0; 3];
    let read_count = debug.read(&mut debug_read).unwrap_or(0);
    let write_count = debug.write(&[1, 2, 3, 4]).unwrap_or(0);
    logger.record(
        "runtime DebugTransport opens and transfers independent bytes",
        open_ok
            && read_count == 3
            && debug_read == [0x2a, 0x7c, 0x91]
            && write_count == 4
            && debug.bytes_written == 4,
    );
    let close_ok = debug.close().is_ok();
    logger.record(
        "runtime DebugTransport rejects reads after close",
        close_ok && !debug.opened && debug.read(&mut debug_read) == Err(WiiError::NotInitialized),
    );

    let mut hid = HidTransportContract {
        sent: [0; 4],
        sent_len: 0,
    };
    let send_ok = hid.send_report(&[0x31, 0x42]).is_ok();
    logger.record(
        "runtime HidTransport sends the requested report",
        send_ok && hid.sent_len == 2 && hid.sent[..2] == [0x31, 0x42],
    );
    let mut hid_read = [0; 3];
    let receive_count = hid.receive_report(&mut hid_read).unwrap_or(0);
    logger.record(
        "runtime HidTransport receives the expected report",
        receive_count == 3 && hid_read == [0xc1, 0x0d, 0x42],
    );
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("SHARED RUNTIME SCHEDULER CONTRACTS");
    probe_driver_trait_contracts(logger);
    probe_lwp_context_constructor(logger);

    let thread = ThreadId::new(core::num::NonZeroU32::new(7).unwrap());
    logger.record(
        "runtime thread ID returns its scheduler handle",
        thread.is_some_and(|thread| thread.get() == 7),
    );
    let null = ThreadId::new(core::num::NonZeroU32::new(u32::MAX).unwrap());
    logger.record(
        "runtime thread ID rejects the null sentinel",
        null.is_none(),
    );

    let clock = LwpClock;
    let now = clock.now();
    logger.record("runtime monotonic clock returns a tick value", now > 0);
    logger.record(
        "runtime clock reports the LWP timebase rate",
        clock.ticks_per_second() == ogc_lwp_driver::TICKS_PER_SECOND,
    );

    let mut timer = LwpTimerService;
    logger.record(
        "runtime timer rejects the null wake token",
        timer.register(Deadline(now), WakeToken(0)) == Err(WiiError::InvalidArgument),
    );
    logger.record(
        "runtime timer cancellation accepts an absent token",
        timer.cancel(WakeToken(0)).is_ok(),
    );

    let executor = LwpDeferredExecutor;
    logger.record(
        "runtime deferred executor accepts a callback",
        executor.enqueue(deferred_noop, 0).is_ok(),
    );
    logger.record(
        "runtime deferred executor rejects a null wake token",
        executor.wake(WakeToken(0)) == Err(WiiError::InvalidArgument),
    );

    let interrupt_wake = LwpInterruptWake;
    logger.record(
        "runtime interrupt wake rejects a null token",
        interrupt_wake.wake_from_interrupt(WakeToken(0)) == Err(WiiError::InvalidArgument),
    );
}
