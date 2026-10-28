#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

pub use ogc_filesystem_sys::DiscInterface;
pub use u32;

pub const DEVICE_TYPE_WII_SD: u32 =
    (b'W' as u32) << 24 | (b'I' as u32) << 16 | (b'S' as u32) << 8 | b'D' as u32;

mod sdio_rules;

#[cfg(target_arch = "powerpc")]
mod imp {
    use super::sdio_rules;
    use super::*;
    use core::ptr::{addr_of, addr_of_mut, copy_nonoverlapping};
    use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

    use ogc_cache::dc_invalidate_range;
    use ogc_ipc::{FileDescriptor, IPC_OPEN_READ, IoctlBuffers, IoctlvEntry, open_file};
    use ogc_lwp_sys::sleep_ticks;

    const PAGE_SIZE_512: usize = 512;
    const BOUNCE_SECTORS: usize = 8;
    const BOUNCE_BYTES: usize = PAGE_SIZE_512 * BOUNCE_SECTORS;

    const SDIOHCR_HOSTCONTROL: u8 = 0x28;
    const SDIOHCR_POWERCONTROL: u8 = 0x29;
    const SDIOHCR_CLOCKCONTROL: u8 = 0x2c;
    const SDIOHCR_TIMEOUTCONTROL: u8 = 0x2e;
    const SDIOHCR_SOFTWARERESET: u8 = 0x2f;

    const SDIOHCR_HOSTCONTROL_4BIT: u8 = 0x02;
    const SDIOHCR_HOSTCONTROL_HS: u8 = 0x04;
    const SDIO_DEFAULT_TIMEOUT: u32 = 0x0e;

    const IOCTL_SDIO_WRITEHCREG: i32 = 0x01;
    const IOCTL_SDIO_READHCREG: i32 = 0x02;
    const IOCTL_SDIO_RESETCARD: i32 = 0x04;
    const IOCTL_SDIO_SETCLK: i32 = 0x06;
    const IOCTL_SDIO_SENDCMD: i32 = 0x07;
    const IOCTL_SDIO_GETSTATUS: i32 = 0x0b;

    const SDIOCMD_TYPE_AC: u32 = 3;
    const SDIOCMD_TYPE_BC: u32 = sdio_rules::SDIOCMD_TYPE_BC;

    const SDIO_RESPONSE_R1: u32 = 1;
    const SDIO_RESPONSE_R1B: u32 = 2;
    const SDIO_RESPONSE_R2: u32 = 3;
    const SDIO_RESPONSE_R3: u32 = 4;
    const SDIO_RESPONSE_R5: u32 = 6;
    const SDIO_RESPONSE_R6: u32 = 7;

    const SDIO_CMD_GOIDLE: u32 = 0x00;
    const SDIO_CMD_ALL_SENDCID: u32 = 0x02;
    const SDIO_CMD_SENDRCA: u32 = 0x03;
    const SDIO_CMD_SWITCHFUNC: u32 = 0x06;
    const SDIO_CMD_SELECT: u32 = 0x07;
    const SDIO_CMD_DESELECT: u32 = 0x07;
    const SDIO_CMD_SENDIFCOND: u32 = 0x08;
    const SDIO_CMD_SENDCSD: u32 = 0x09;
    const SDIO_CMD_SETBLOCKLEN: u32 = 0x10;
    const SDIO_CMD_READMULTIBLOCK: u32 = 0x12;
    const SDIO_CMD_WRITEMULTIBLOCK: u32 = 0x19;
    const SDIO_CMD_APPCMD: u32 = 0x37;

    const SDIO_ACMD_SETBUSWIDTH: u32 = 0x06;
    const SDIO_ACMD_SENDOPCOND: u32 = 0x29;

    const SDIO_STATUS_CARD_INSERTED: i32 = 0x1;
    const SDIO_STATUS_CARD_INITIALIZED: i32 = 0x10000;
    const SDIO_STATUS_CARD_SDHC: i32 = 0x100000;
    const SDIO_READY_RETRIES: usize = 10;
    const SDIO_RETRY_DELAY_TICKS: u64 = 607_500; // 10 ms at the Wii's 60.75 MHz timebase.

    #[inline]
    fn block_offset(sector: u32, sdhc: bool) -> Option<u32> {
        if sdhc {
            Some(sector)
        } else {
            sector.checked_mul(PAGE_SIZE_512 as u32)
        }
    }

    #[inline]
    fn transfer_succeeded(command_result: i32, _deselect_result: i32) -> bool {
        // BUG FIX: C ignores the deselect result and returns the command result.
        command_result >= 0
    }

    #[inline]
    fn transfer_request_valid(initialized: bool, buffer: *const c_void, _num_sectors: u32) -> bool {
        // C permits zero-sector requests to reach the selected-card transfer path.
        initialized && !buffer.is_null()
    }

    #[inline]
    fn status_has_card(status: i32) -> bool {
        // BUG FIX: A negative IOS error value must not pass the inserted-bit check.
        status >= 0 && status & SDIO_STATUS_CARD_INSERTED == SDIO_STATUS_CARD_INSERTED
    }

    #[repr(C, align(32))]
    struct AlignedBytes<const N: usize>([u8; N]);

    #[repr(C, align(64))]
    struct AlignedFunctionBytes([u8; sdio_rules::SDIO_BC_BUFFER_ALIGNMENT]);

    const _: () = assert!(
        core::mem::align_of::<AlignedFunctionBytes>() == sdio_rules::SDIO_BC_BUFFER_ALIGNMENT
    );

    #[repr(C, align(32))]
    struct AlignedWords<const N: usize>([u32; N]);

    #[repr(C, align(32))]
    struct AlignedSdioRequest(SdioRequest);

    #[repr(C, align(32))]
    struct AlignedSdioResponse(SdioResponse);

    #[repr(C)]
    struct SdioRequest {
        cmd: u32,
        cmd_type: u32,
        rsp_type: u32,
        arg: u32,
        blk_cnt: u32,
        blk_size: u32,
        dma_addr: *mut c_void,
        isdma: u32,
        pad0: u32,
    }

    #[repr(C)]
    struct SdioResponse {
        rsp_fields: [u32; 3],
        acmd12_response: u32,
    }

    static FD: AtomicI32 = AtomicI32::new(-1);
    static RCA: AtomicU32 = AtomicU32::new(0);
    static SDHC: AtomicBool = AtomicBool::new(false);
    static INITIALIZED: AtomicBool = AtomicBool::new(false);

    static mut CSD: AlignedBytes<16> = AlignedBytes([0; 16]);
    static mut CID: AlignedBytes<16> = AlignedBytes([0; 16]);
    static mut FUNCTIONS: AlignedFunctionBytes = AlignedFunctionBytes([0; 64]);
    static mut RW_BUFFER: AlignedBytes<BOUNCE_BYTES> = AlignedBytes([0; BOUNCE_BYTES]);

    #[inline]
    fn fd() -> Option<FileDescriptor> {
        let raw = FD.load(Ordering::Acquire);
        (raw >= 0).then(|| FileDescriptor::new(raw))
    }

    #[inline]
    fn set_fd(raw: i32) {
        FD.store(raw, Ordering::Release);
    }

    #[inline]
    fn rca_arg() -> u32 {
        RCA.load(Ordering::Acquire) << 16
    }

    #[inline]
    unsafe fn bytes_of<T>(value: &T) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts((value as *const T).cast::<u8>(), core::mem::size_of::<T>())
        }
    }

    #[inline]
    unsafe fn bytes_of_mut<T>(value: &mut T) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                (value as *mut T).cast::<u8>(),
                core::mem::size_of::<T>(),
            )
        }
    }

    #[inline]
    unsafe fn static_bytes_mut<const N: usize>(ptr: *mut AlignedBytes<N>) -> &'static mut [u8] {
        unsafe { core::slice::from_raw_parts_mut((*ptr).0.as_mut_ptr(), N) }
    }

    #[inline]
    unsafe fn static_bytes<const N: usize>(ptr: *const AlignedBytes<N>) -> &'static [u8] {
        unsafe { core::slice::from_raw_parts((*ptr).0.as_ptr(), N) }
    }

    #[inline]
    fn delay_10ms() {
        sleep_ticks(SDIO_RETRY_DELAY_TICKS);
    }

    fn open_slot() -> bool {
        match open_file(c"/dev/sdio/slot0", IPC_OPEN_READ) {
            Ok(fd) => {
                set_fd(fd.raw());
                true
            }
            Err(_) => {
                set_fd(-1);
                false
            }
        }
    }

    fn close_slot() -> bool {
        let raw = FD.swap(-1, Ordering::AcqRel);
        if raw >= 0 {
            FileDescriptor::new(raw).close().is_ok()
        } else {
            false
        }
    }

    fn reopen_slot() -> bool {
        let _ = close_slot();
        open_slot()
    }

    fn ioctl_words<const IN: usize, const OUT: usize>(
        ioctl: i32,
        input: Option<&mut AlignedWords<IN>>,
        output: Option<&mut AlignedWords<OUT>>,
    ) -> i32 {
        let Some(fd) = fd() else {
            return -1;
        };
        let input = input.map(|words| unsafe { bytes_of_mut(&mut words.0) });
        let output = output.map(|words| unsafe { bytes_of_mut(&mut words.0) });
        fd.ioctl(ioctl, IoctlBuffers::from_slices(input, output))
    }

    struct Command<'a> {
        cmd: u32,
        cmd_type: u32,
        rsp_type: u32,
        arg: u32,
        blk_cnt: u32,
        blk_size: u32,
        buffer: *mut u8,
        reply: Option<&'a mut [u8]>,
    }

    macro_rules! send_command {
        ($cmd:expr_2021, $cmd_type:expr_2021, $rsp_type:expr_2021, $arg:expr_2021, $blk_cnt:expr_2021, $blk_size:expr_2021, $buffer:expr_2021, $reply:expr_2021 $(,)?) => {
            send_command(Command {
                cmd: $cmd,
                cmd_type: $cmd_type,
                rsp_type: $rsp_type,
                arg: $arg,
                blk_cnt: $blk_cnt,
                blk_size: $blk_size,
                buffer: $buffer,
                reply: $reply,
            })
        };
    }

    fn send_command(command: Command<'_>) -> i32 {
        let Command {
            cmd,
            cmd_type,
            rsp_type,
            arg,
            blk_cnt,
            blk_size,
            buffer,
            reply,
        } = command;
        let Some(fd) = fd() else {
            return -1;
        };
        let Some(data_len) = sdio_rules::transfer_length(blk_cnt, blk_size) else {
            return ogc_ipc::IPC_EINVAL;
        };
        if !sdio_rules::broadcast_range_valid(cmd_type, buffer as usize, data_len) {
            return ogc_ipc::IPC_EINVAL;
        }
        let mut request = AlignedSdioRequest(SdioRequest {
            cmd,
            cmd_type,
            rsp_type,
            arg,
            blk_cnt,
            blk_size,
            dma_addr: buffer.cast::<c_void>(),
            isdma: (!buffer.is_null()) as u32,
            pad0: 0,
        });
        let mut response = AlignedSdioResponse(SdioResponse {
            rsp_fields: [0; 3],
            acmd12_response: 0,
        });

        let rc = if request.0.isdma != 0 || SDHC.load(Ordering::Acquire) {
            let empty = [];
            let data = if buffer.is_null() || data_len == 0 {
                &empty[..]
            } else {
                unsafe { core::slice::from_raw_parts(buffer.cast_const(), data_len) }
            };
            let rc = fd.ioctlv(
                IOCTL_SDIO_SENDCMD,
                2,
                [
                    IoctlvEntry::In(unsafe { bytes_of(&request.0) }),
                    IoctlvEntry::In(data),
                    IoctlvEntry::Out(unsafe { bytes_of_mut(&mut response.0) }),
                ],
            );
            if !buffer.is_null() && data_len > 0 {
                unsafe {
                    dc_invalidate_range(buffer.cast_const().cast(), data_len as u32);
                }
            }
            rc
        } else {
            fd.ioctl(
                IOCTL_SDIO_SENDCMD,
                IoctlBuffers::from_slices(
                    Some(unsafe { bytes_of_mut(&mut request.0) }),
                    Some(unsafe { bytes_of_mut(&mut response.0) }),
                ),
            )
        };

        if rc >= 0
            && let Some(reply) = reply
            && reply.len() <= core::mem::size_of::<SdioResponse>()
        {
            unsafe {
                copy_nonoverlapping(
                    (&response.0 as *const SdioResponse).cast::<u8>(),
                    reply.as_mut_ptr(),
                    reply.len(),
                );
            }
        }
        rc
    }

    fn set_clock(clock: u32) -> i32 {
        let mut input = AlignedWords([clock]);
        ioctl_words::<1, 1>(IOCTL_SDIO_SETCLK, Some(&mut input), None)
    }

    fn get_status() -> i32 {
        let Some(fd) = fd() else {
            return -1;
        };
        let mut status = AlignedWords([0]);
        let rc = fd.ioctl(
            IOCTL_SDIO_GETSTATUS,
            IoctlBuffers::from_slices(None, Some(unsafe { bytes_of_mut(&mut status.0) })),
        );
        if rc < 0 { rc } else { status.0[0] as i32 }
    }

    fn reset_card() -> i32 {
        let Some(fd) = fd() else {
            return -1;
        };
        let mut status = AlignedWords([0]);
        RCA.store(0, Ordering::Release);
        let rc = fd.ioctl(
            IOCTL_SDIO_RESETCARD,
            IoctlBuffers::from_slices(None, Some(unsafe { bytes_of_mut(&mut status.0) })),
        );
        if rc < 0 {
            rc
        } else {
            RCA.store(status.0[0] >> 16, Ordering::Release);
            (status.0[0] & 0xffff) as i32
        }
    }

    fn get_hcr(reg: u8, size: u8) -> Result<u32, i32> {
        let mut query = AlignedWords([reg as u32, 0, 0, size as u32, 0, 0]);
        let mut value = AlignedWords([0]);
        let rc = ioctl_words(IOCTL_SDIO_READHCREG, Some(&mut query), Some(&mut value));
        if rc < 0 { Err(rc) } else { Ok(value.0[0]) }
    }

    fn set_hcr(reg: u8, size: u8, data: u32) -> i32 {
        let mut query = AlignedWords([reg as u32, 0, 0, size as u32, data, 0]);
        ioctl_words::<6, 1>(IOCTL_SDIO_WRITEHCREG, Some(&mut query), None)
    }

    fn wait_hcr(reg: u8, size: u8, unset: bool, mask: u32) -> i32 {
        for _ in 0..10 {
            let Ok(value) = get_hcr(reg, size) else {
                return -1;
            };
            if (unset && value & mask == 0) || (!unset && value & mask != 0) {
                return 0;
            }
            delay_10ms();
        }
        -1
    }

    fn set_host_buswidth(enable: bool, reg: u8) -> i32 {
        let Ok(mut hc_reg) = get_hcr(SDIOHCR_HOSTCONTROL, 1) else {
            return -1;
        };
        hc_reg &= 0xff;
        hc_reg &= !(reg as u32);
        if enable {
            hc_reg |= reg as u32;
        }
        set_hcr(SDIOHCR_HOSTCONTROL, 1, hc_reg)
    }

    fn get_rca() -> i32 {
        let mut rca = 0u32;
        let rc = send_command!(
            SDIO_CMD_SENDRCA,
            0,
            SDIO_RESPONSE_R5,
            0,
            0,
            0,
            core::ptr::null_mut(),
            Some(unsafe { bytes_of_mut(&mut rca) }),
        );
        if rc < 0 {
            rc
        } else {
            RCA.store(rca >> 16, Ordering::Release);
            (rca & 0xffff) as i32
        }
    }

    fn select_card() -> i32 {
        send_command!(
            SDIO_CMD_SELECT,
            SDIOCMD_TYPE_AC,
            SDIO_RESPONSE_R1B,
            rca_arg(),
            0,
            0,
            core::ptr::null_mut(),
            None,
        )
    }

    fn deselect_card() -> i32 {
        send_command!(
            SDIO_CMD_DESELECT,
            SDIOCMD_TYPE_AC,
            SDIO_RESPONSE_R1B,
            0,
            0,
            0,
            core::ptr::null_mut(),
            None,
        )
    }

    fn set_block_length(len: u32) -> i32 {
        send_command!(
            SDIO_CMD_SETBLOCKLEN,
            SDIOCMD_TYPE_AC,
            SDIO_RESPONSE_R1,
            len,
            0,
            0,
            core::ptr::null_mut(),
            None,
        )
    }

    fn set_card_buswidth(width: u32) -> i32 {
        let value = if width == 4 { 0x0002 } else { 0x0000 };
        let rc = send_command!(
            SDIO_CMD_APPCMD,
            SDIOCMD_TYPE_AC,
            SDIO_RESPONSE_R1,
            rca_arg(),
            0,
            0,
            core::ptr::null_mut(),
            None,
        );
        if rc < 0 {
            return rc;
        }
        let rc = send_command!(
            SDIO_ACMD_SETBUSWIDTH,
            SDIOCMD_TYPE_AC,
            SDIO_RESPONSE_R1,
            value,
            0,
            0,
            core::ptr::null_mut(),
            None,
        );
        if rc < 0 {
            return rc;
        }
        set_host_buswidth(width == 4, SDIOHCR_HOSTCONTROL_4BIT)
    }

    fn get_csd() -> i32 {
        send_command!(
            SDIO_CMD_SENDCSD,
            SDIOCMD_TYPE_AC,
            SDIO_RESPONSE_R2,
            rca_arg(),
            0,
            0,
            core::ptr::null_mut(),
            Some(unsafe { static_bytes_mut(addr_of_mut!(CSD)) }),
        )
    }

    fn get_cid() -> i32 {
        send_command!(
            SDIO_CMD_ALL_SENDCID,
            0,
            SDIO_RESPONSE_R2,
            rca_arg(),
            0,
            0,
            core::ptr::null_mut(),
            Some(unsafe { static_bytes_mut(addr_of_mut!(CID)) }),
        )
    }

    fn card_command_acceptance() -> u16 {
        let csd = unsafe { static_bytes(addr_of!(CSD)) };
        ((csd[9] as u16) << 4) | (((csd[10] >> 4) & 0x0f) as u16)
    }

    fn init_io() -> bool {
        let _ = reset_card();
        let status = get_status();
        if status < 0 || status & SDIO_STATUS_CARD_INSERTED == 0 {
            return false;
        }

        if status & SDIO_STATUS_CARD_INITIALIZED == 0 {
            if !reopen_slot() {
                return false;
            }
            if set_hcr(SDIOHCR_SOFTWARERESET, 1, 7) < 0 {
                return fail_reset();
            }
            if wait_hcr(SDIOHCR_SOFTWARERESET, 1, true, 7) < 0 {
                return fail_reset();
            }
            let _ = set_hcr(0x34, 4, 0x13f00c3);
            let _ = set_hcr(0x38, 4, 0x13f00c3);

            SDHC.store(true, Ordering::Release);
            if set_hcr(SDIOHCR_POWERCONTROL, 1, 0xe) < 0 {
                return fail_reset();
            }
            if set_hcr(SDIOHCR_POWERCONTROL, 1, 0xf) < 0 {
                return fail_reset();
            }
            if set_hcr(SDIOHCR_CLOCKCONTROL, 2, 0) < 0 {
                return fail_reset();
            }
            if set_hcr(SDIOHCR_CLOCKCONTROL, 2, 0x101) < 0 {
                return fail_reset();
            }
            if wait_hcr(SDIOHCR_CLOCKCONTROL, 2, false, 2) < 0 {
                return fail_reset();
            }
            if set_hcr(SDIOHCR_CLOCKCONTROL, 2, 0x107) < 0 {
                return fail_reset();
            }
            if set_hcr(SDIOHCR_TIMEOUTCONTROL, 1, SDIO_DEFAULT_TIMEOUT) < 0 {
                return fail_reset();
            }
            if send_command!(SDIO_CMD_GOIDLE, 0, 0, 0, 0, 0, core::ptr::null_mut(), None) < 0 {
                return fail_reset();
            }

            let mut response = SdioResponse {
                rsp_fields: [0; 3],
                acmd12_response: 0,
            };
            if send_command!(
                SDIO_CMD_SENDIFCOND,
                0,
                SDIO_RESPONSE_R6,
                0x1aa,
                0,
                0,
                core::ptr::null_mut(),
                Some(unsafe { bytes_of_mut(&mut response) }),
            ) < 0
            {
                return fail_reset();
            }
            if response.rsp_fields[0] & 0xff != 0xaa {
                return fail_reset();
            }

            let mut ready = false;
            for _ in 0..SDIO_READY_RETRIES {
                if send_command!(
                    SDIO_CMD_APPCMD,
                    SDIOCMD_TYPE_AC,
                    SDIO_RESPONSE_R1,
                    0,
                    0,
                    0,
                    core::ptr::null_mut(),
                    None,
                ) < 0
                {
                    return fail_reset();
                }
                if send_command!(
                    SDIO_ACMD_SENDOPCOND,
                    0,
                    SDIO_RESPONSE_R3,
                    0x4030_0000,
                    0,
                    0,
                    core::ptr::null_mut(),
                    Some(unsafe { bytes_of_mut(&mut response) }),
                ) < 0
                {
                    return fail_reset();
                }
                if response.rsp_fields[0] & (1 << 31) != 0 {
                    ready = true;
                    break;
                }
                delay_10ms();
            }
            if !ready {
                return fail_reset();
            }
            SDHC.store(response.rsp_fields[0] & (1 << 30) != 0, Ordering::Release);
            if get_cid() < 0 || get_rca() < 0 {
                return fail_reset();
            }
        } else {
            SDHC.store(status & SDIO_STATUS_CARD_SDHC != 0, Ordering::Release);
        }

        if RCA.load(Ordering::Acquire) == 0 && get_rca() < 0 {
            return fail_reset();
        }

        if get_csd() < 0 || select_card() < 0 {
            return false;
        }
        if set_block_length(PAGE_SIZE_512 as u32) < 0 {
            let _ = deselect_card();
            return false;
        }
        if set_card_buswidth(4) < 0 {
            let _ = deselect_card();
            return false;
        }

        let mut clock = 1;
        if card_command_acceptance() & (1 << 10) != 0 {
            let functions = unsafe {
                core::slice::from_raw_parts_mut(
                    addr_of_mut!(FUNCTIONS).cast::<u8>(),
                    core::mem::size_of::<AlignedFunctionBytes>(),
                )
            };
            if send_command!(
                SDIO_CMD_SWITCHFUNC,
                SDIOCMD_TYPE_BC,
                SDIO_RESPONSE_R1,
                0x00ff_fff0,
                1,
                functions.len() as u32,
                functions.as_mut_ptr(),
                None,
            ) < 0
            {
                let _ = deselect_card();
                return false;
            }
            if u16::from_ne_bytes([functions[12], functions[13]]) & (1 << 1) != 0 {
                if send_command!(
                    SDIO_CMD_SWITCHFUNC,
                    SDIOCMD_TYPE_BC,
                    SDIO_RESPONSE_R1,
                    0x80ff_fff1,
                    1,
                    functions.len() as u32,
                    functions.as_mut_ptr(),
                    None,
                ) < 0
                {
                    let _ = deselect_card();
                    return false;
                }
                if functions[16] & 0x0f == 1 {
                    if set_host_buswidth(true, SDIOHCR_HOSTCONTROL_HS) < 0 {
                        let _ = deselect_card();
                        return false;
                    }
                    clock = 0;
                }
            }
        }

        if set_clock(clock) < 0 {
            let _ = deselect_card();
            return false;
        }

        let _ = deselect_card();
        true
    }

    fn fail_reset() -> bool {
        let _ = set_hcr(SDIOHCR_SOFTWARERESET, 1, 7);
        let _ = wait_hcr(SDIOHCR_SOFTWARERESET, 1, true, 7);
        let _ = reopen_slot();
        false
    }

    pub fn startup() -> bool {
        if INITIALIZED.load(Ordering::Acquire) {
            return true;
        }
        if !open_slot() {
            return false;
        }
        if !init_io() {
            deinitialize();
            return false;
        }
        INITIALIZED.store(true, Ordering::Release);
        true
    }

    fn deinitialize() -> bool {
        // BUG FIX: C reports success after the close attempt, even if IOS_Close fails.
        close_slot();
        INITIALIZED.store(false, Ordering::Release);
        RCA.store(0, Ordering::Release);
        SDHC.store(false, Ordering::Release);
        true
    }

    pub fn shutdown() -> bool {
        if !INITIALIZED.load(Ordering::Acquire) {
            return false;
        }
        deinitialize()
    }

    pub fn is_inserted() -> bool {
        INITIALIZED.load(Ordering::Acquire) && status_has_card(get_status())
    }

    pub fn read_sectors(sector: u32, mut num_sectors: u32, buffer: *mut c_void) -> bool {
        // BUG FIX: C passes zero-sector requests through after selecting the
        // card; the Rust guard previously returned false before that path.
        if !transfer_request_valid(INITIALIZED.load(Ordering::Acquire), buffer, num_sectors) {
            return false;
        }
        if select_card() < 0 {
            return false;
        }

        let mut ret = 0;
        let mut sector = sector;
        let mut ptr = buffer.cast::<u8>();
        if (ptr as usize) & 0x1f != 0 {
            while num_sectors > 0 {
                let sectors_now = num_sectors.min(BOUNCE_SECTORS as u32);
                let Some(block_offset) = block_offset(sector, SDHC.load(Ordering::Acquire)) else {
                    ret = -1;
                    break;
                };
                let bounce = unsafe { static_bytes_mut(addr_of_mut!(RW_BUFFER)) };
                ret = send_command!(
                    SDIO_CMD_READMULTIBLOCK,
                    SDIOCMD_TYPE_AC,
                    SDIO_RESPONSE_R1,
                    block_offset,
                    sectors_now,
                    PAGE_SIZE_512 as u32,
                    bounce.as_mut_ptr(),
                    None,
                );
                if ret < 0 {
                    break;
                }
                unsafe {
                    copy_nonoverlapping(bounce.as_ptr(), ptr, sectors_now as usize * PAGE_SIZE_512);
                    ptr = ptr.add(sectors_now as usize * PAGE_SIZE_512);
                }
                num_sectors -= sectors_now;
                if num_sectors == 0 {
                    break;
                }
                let Some(next_sector) = sector.checked_add(sectors_now) else {
                    ret = -1;
                    break;
                };
                sector = next_sector;
            }
        } else {
            let Some(block_offset) = block_offset(sector, SDHC.load(Ordering::Acquire)) else {
                deselect_card();
                return false;
            };
            ret = send_command!(
                SDIO_CMD_READMULTIBLOCK,
                SDIOCMD_TYPE_AC,
                SDIO_RESPONSE_R1,
                block_offset,
                num_sectors,
                PAGE_SIZE_512 as u32,
                buffer.cast::<u8>(),
                None,
            );
        }

        let deselect_result = deselect_card();
        transfer_succeeded(ret, deselect_result)
    }

    pub fn write_sectors(sector: u32, mut num_sectors: u32, buffer: *const c_void) -> bool {
        // BUG FIX: Match C's zero-sector request behavior for writes too.
        if !transfer_request_valid(INITIALIZED.load(Ordering::Acquire), buffer, num_sectors) {
            return false;
        }
        if select_card() < 0 {
            return false;
        }

        let mut ret = 0;
        let mut sector = sector;
        let mut ptr = buffer.cast::<u8>();
        if (ptr as usize) & 0x1f != 0 {
            while num_sectors > 0 {
                let sectors_now = num_sectors.min(BOUNCE_SECTORS as u32);
                let Some(block_offset) = block_offset(sector, SDHC.load(Ordering::Acquire)) else {
                    ret = -1;
                    break;
                };
                let bounce = unsafe { static_bytes_mut(addr_of_mut!(RW_BUFFER)) };
                unsafe {
                    copy_nonoverlapping(
                        ptr,
                        bounce.as_mut_ptr(),
                        sectors_now as usize * PAGE_SIZE_512,
                    );
                    ptr = ptr.add(sectors_now as usize * PAGE_SIZE_512);
                }
                ret = send_command!(
                    SDIO_CMD_WRITEMULTIBLOCK,
                    SDIOCMD_TYPE_AC,
                    SDIO_RESPONSE_R1,
                    block_offset,
                    sectors_now,
                    PAGE_SIZE_512 as u32,
                    bounce.as_mut_ptr(),
                    None,
                );
                if ret < 0 {
                    break;
                }
                num_sectors -= sectors_now;
                if num_sectors == 0 {
                    break;
                }
                let Some(next_sector) = sector.checked_add(sectors_now) else {
                    ret = -1;
                    break;
                };
                sector = next_sector;
            }
        } else {
            let Some(block_offset) = block_offset(sector, SDHC.load(Ordering::Acquire)) else {
                deselect_card();
                return false;
            };
            ret = send_command!(
                SDIO_CMD_WRITEMULTIBLOCK,
                SDIOCMD_TYPE_AC,
                SDIO_RESPONSE_R1,
                block_offset,
                num_sectors,
                PAGE_SIZE_512 as u32,
                buffer as *mut u8,
                None,
            );
        }

        let deselect_result = deselect_card();
        transfer_succeeded(ret, deselect_result)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            FD, SDIO_RETRY_DELAY_TICKS, SDIO_STATUS_CARD_INSERTED, deinitialize, delay_10ms,
            status_has_card, transfer_request_valid, transfer_succeeded,
        };
        use ogc_core::powerpc::read_timebase;

        #[test]
        fn deinitialize_succeeds_when_no_slot_is_open() {
            FD.store(-1, super::Ordering::Release);
            assert!(deinitialize());
        }

        #[test]
        fn transfer_result_uses_the_command_result_only() {
            assert!(transfer_succeeded(0, -1));
            assert!(!transfer_succeeded(-1, 0));
        }

        #[test]
        fn zero_sector_request_is_not_rejected_before_card_selection() {
            let buffer = 0x1000usize as *const c_void;
            assert!(transfer_request_valid(true, buffer, 0));
            assert!(!transfer_request_valid(true, core::ptr::null(), 0));
        }

        #[test]
        fn failed_status_read_does_not_report_inserted_card() {
            assert!(!status_has_card(-3));
            assert!(status_has_card(SDIO_STATUS_CARD_INSERTED));
            assert!(!status_has_card(0));
        }

        #[test]
        fn retry_delay_waits_at_least_ten_milliseconds() {
            let start = read_timebase();
            delay_10ms();
            assert!(read_timebase().wrapping_sub(start) >= SDIO_RETRY_DELAY_TICKS);
        }
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn startup() -> bool {
    imp::startup()
}

#[cfg(not(target_arch = "powerpc"))]
unsafe extern "C" fn startup() -> bool {
    false
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn is_inserted() -> bool {
    imp::is_inserted()
}

#[cfg(not(target_arch = "powerpc"))]
unsafe extern "C" fn is_inserted() -> bool {
    false
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn read_sectors(sector: u32, num_sectors: u32, buffer: *mut c_void) -> bool {
    imp::read_sectors(sector, num_sectors, buffer)
}

#[cfg(not(target_arch = "powerpc"))]
unsafe extern "C" fn read_sectors(_sector: u32, _num_sectors: u32, _buffer: *mut c_void) -> bool {
    false
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn write_sectors(sector: u32, num_sectors: u32, buffer: *const c_void) -> bool {
    imp::write_sectors(sector, num_sectors, buffer)
}

#[cfg(not(target_arch = "powerpc"))]
unsafe extern "C" fn write_sectors(
    _sector: u32,
    _num_sectors: u32,
    _buffer: *const c_void,
) -> bool {
    false
}

unsafe extern "C" fn clear_status() -> bool {
    true
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn shutdown() -> bool {
    imp::shutdown()
}

#[cfg(not(target_arch = "powerpc"))]
unsafe extern "C" fn shutdown() -> bool {
    false
}

pub static WII_SD_INTERFACE: DiscInterface = DiscInterface {
    io_type: DEVICE_TYPE_WII_SD,
    features: 0x0000_0103,
    startup: Some(startup),
    is_inserted: Some(is_inserted),
    read_sectors: Some(read_sectors),
    write_sectors: Some(write_sectors),
    clear_status: Some(clear_status),
    shutdown: Some(shutdown),
};

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn disc_interface_layout_is_host_stable() {
        assert_eq!(offset_of!(DiscInterface, io_type), 0);
        assert_eq!(offset_of!(DiscInterface, features), 4);
        assert!(offset_of!(DiscInterface, startup) > offset_of!(DiscInterface, features));
        assert!(offset_of!(DiscInterface, shutdown) > offset_of!(DiscInterface, clear_status));
        assert_eq!(align_of::<DiscInterface>(), align_of::<usize>());
        assert!(size_of::<DiscInterface>() >= 32);
    }

    #[test]
    fn exported_interface_has_wii_sd_features_and_callbacks() {
        assert_eq!(WII_SD_INTERFACE.io_type, DEVICE_TYPE_WII_SD);
        assert_eq!(WII_SD_INTERFACE.features, 0x0000_0103);
        assert!(WII_SD_INTERFACE.startup.is_some());
        assert!(WII_SD_INTERFACE.is_inserted.is_some());
        assert!(WII_SD_INTERFACE.read_sectors.is_some());
        assert!(WII_SD_INTERFACE.write_sectors.is_some());
        assert!(WII_SD_INTERFACE.clear_status.is_some());
        assert!(WII_SD_INTERFACE.shutdown.is_some());
    }

    #[test]
    fn host_backend_only_reports_clear_status_success() {
        let mut sector = [0u8; 512];
        unsafe {
            assert!(!WII_SD_INTERFACE.startup.is_some_and(|callback| callback()));
            assert!(
                !WII_SD_INTERFACE
                    .is_inserted
                    .is_some_and(|callback| callback())
            );
            assert!(
                !WII_SD_INTERFACE
                    .write_sectors
                    .is_some_and(|callback| callback(0, 1, sector.as_ptr().cast()))
            );
            assert!(
                !WII_SD_INTERFACE
                    .read_sectors
                    .is_some_and(|callback| callback(0, 1, sector.as_mut_ptr().cast()))
            );
            assert!(
                WII_SD_INTERFACE
                    .clear_status
                    .is_some_and(|callback| callback())
            );
            assert!(!WII_SD_INTERFACE.shutdown.is_some_and(|callback| callback()));
        }
    }
}
