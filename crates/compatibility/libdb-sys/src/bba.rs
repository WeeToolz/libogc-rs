//! Private Broadband Adapter transport used by the libdb TCP debugger.
//!
//! This module owns the EXI/BBA device path and raw Ethernet packet I/O. It is
//! intentionally independent of the private ARP/IP/TCP layer that will sit
//! above it.

use core::ffi::c_void;
use core::hint::spin_loop;

use ogc_exi as exi;
use ogc_lwp_driver::{TICKS_PER_SECOND, now_ticks};

const CHANNEL: i32 = exi::EXI_CHANNEL_0;
const DEVICE: i32 = exi::EXI_DEVICE_2;
const SPEED: i32 = exi::EXI_SPEED32MHZ;

const BBA_CID: u32 = 0x0402_0200;
const BBA_MIN_PACKET_SIZE: usize = 60;
pub(crate) const BBA_TX_MAX_PACKET_SIZE: usize = 1518;
pub(crate) const BBA_RX_MAX_PACKET_SIZE: usize = 1540;

const BBA_CMD_IRMASK_ALL: u8 = 0x00;
const BBA_CMD_IRMASK_NONE: u8 = 0xf8;

const BBA_NCRA: u32 = 0x00;
const BBA_NCRA_RESET: u8 = 1 << 0;
const BBA_NCRA_ST0: u8 = 1 << 1;
const BBA_NCRA_ST1: u8 = 1 << 2;
const BBA_NCRA_SR: u8 = 1 << 3;
const BBA_NCRB: u32 = 0x01;
const BBA_NCRB_CA: u8 = 1 << 1;
const BBA_NCRB_AB: u8 = 1 << 4;
const BBA_IMR: u32 = 0x08;
const BBA_IMR_FIFOEIM: u8 = 1 << 5;
const BBA_IR: u32 = 0x09;
const BBA_IR_FRAGI: u8 = 1 << 0;
const BBA_IR_RI: u8 = 1 << 1;
const BBA_IR_TI: u8 = 1 << 2;
const BBA_IR_REI: u8 = 1 << 3;
const BBA_IR_TEI: u8 = 1 << 4;
const BBA_IR_FIFOEI: u8 = 1 << 5;
const BBA_IR_BUSEI: u8 = 1 << 6;
const BBA_IR_RBFI: u8 = 1 << 7;
const BBA_BP: u32 = 0x0a;
const BBA_TLBP: u32 = 0x0c;
const BBA_RWP: u32 = 0x16;
const BBA_RRP: u32 = 0x18;
const BBA_RHBP: u32 = 0x1a;
const BBA_NAFR_PAR0: u32 = 0x20;
const BBA_NWAYS: u32 = 0x31;
const BBA_NWAYS_LS10: u8 = 1 << 0;
const BBA_NWAYS_LS100: u8 = 1 << 1;
const BBA_GCA: u32 = 0x32;
const BBA_GCA_ARXERRB: u8 = 1 << 3;
const BBA_TXFIFOCNT: u32 = 0x3e;
const BBA_WRTXFIFOD: u32 = 0x48;
const BBA_MISC2: u32 = 0x50;
const BBA_MISC2_AUTORCVR: u8 = 1 << 7;

const BBA_RX_STATUS_FAE: u8 = 1 << 2;
const BBA_RX_STATUS_RERR: u8 = 1 << 7;

const BBA_INIT_TLBP: u16 = 0x00;
const BBA_INIT_BP: u16 = 0x01;
const BBA_INIT_RHBP: u16 = 0x0f;
const BBA_INIT_RWP: u16 = BBA_INIT_BP;
const BBA_INIT_RRP: u16 = BBA_INIT_BP;
const BBA_RX_RING_TOP: usize = ((BBA_INIT_RHBP as usize) + 1) << 8;
const BBA_RX_RING_BOTTOM: usize = (BBA_INIT_RRP as usize) << 8;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum BbaError {
    ExiLock,
    ExiTransfer,
    NotPresent,
    Challenge,
    LinkDown,
    PacketTooLarge,
    BufferTooSmall,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct BbaDevice {
    revision: u8,
    device_id: u16,
    acstart: u8,
    mac: [u8; 6],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct RxDescriptor {
    status: u8,
    packet_len: u16,
    next_packet_ptr: u16,
}

struct ExiLock;

impl ExiLock {
    fn acquire() -> Result<Self, BbaError> {
        if unsafe { exi::lock(CHANNEL, DEVICE, None) } == 0 {
            Err(BbaError::ExiLock)
        } else {
            Ok(Self)
        }
    }
}

impl Drop for ExiLock {
    fn drop(&mut self) {
        let _ = exi::unlock(CHANNEL);
    }
}

struct Selection;

impl Selection {
    fn acquire() -> Result<Self, BbaError> {
        if exi::select(CHANNEL, DEVICE, SPEED) == 0 {
            Err(BbaError::ExiTransfer)
        } else {
            Ok(Self)
        }
    }
}

impl Drop for Selection {
    fn drop(&mut self) {
        let _ = exi::deselect(CHANNEL);
    }
}

impl BbaDevice {
    pub(crate) fn initialize() -> Result<Self, BbaError> {
        let mut device = Self {
            revision: 0,
            device_id: 0xd107,
            acstart: 0x4e,
            mac: [0; 6],
        };

        {
            let _lock = ExiLock::acquire()?;
            if read_cid()? != BBA_CID {
                return Err(BbaError::NotPresent);
            }
            device.initialize_locked()?;
        }

        device.challenge_response()?;

        for _ in 0..10_000 {
            delay_microseconds(500);
            let _lock = match ExiLock::acquire() {
                Ok(lock) => lock,
                Err(_) => continue,
            };
            if link_state()? {
                return Ok(device);
            }
        }
        Err(BbaError::LinkDown)
    }

    pub(crate) const fn mac_address(&self) -> [u8; 6] {
        self.mac
    }

    pub(crate) fn poll_frame(&mut self, frame: &mut [u8]) -> Result<Option<usize>, BbaError> {
        let _lock = match ExiLock::acquire() {
            Ok(lock) => lock,
            Err(BbaError::ExiLock) => return Ok(None),
            Err(error) => return Err(error),
        };

        let command_status = cmd_read_u8(0x03)?;
        if command_status != 0 {
            cmd_write_u8(0x02, BBA_CMD_IRMASK_ALL)?;
            match command_status {
                status if status & 0x80 != 0 => {}
                status if status & 0x40 != 0 => {
                    self.initialize_locked()?;
                    cmd_write_u8(0x03, 0x40)?;
                    cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
                    return Ok(None);
                }
                status if status & 0x20 != 0 => {
                    cmd_write_u8(0x03, 0x20)?;
                    cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
                    return Ok(None);
                }
                status if status & 0x10 != 0 => {
                    self.answer_challenge_locked()?;
                    cmd_write_u8(0x03, 0x10)?;
                    cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
                    return Ok(None);
                }
                status if status & 0x08 != 0 => {
                    cmd_write_u8(0x03, 0x08)?;
                    cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
                    return Ok(None);
                }
                _ => {}
            }
        }

        let interrupt = read_u8(BBA_IR)? & read_u8(BBA_IMR)?;

        // The C driver drains the receive ring only while handling RI/RBFI and
        // can process up to 16 frames in one poll. This transport returns one
        // frame to its caller at a time, so it also checks the ring directly.
        // That preserves packets left after a prior single-frame return.
        let received = receive_one_locked(frame)?;
        clear_interrupts(interrupt)?;
        if command_status & 0x80 != 0 {
            cmd_write_u8(0x03, 0x80)?;
        }
        if command_status != 0 {
            cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
        }
        Ok(received)
    }

    pub(crate) fn send_frame(&mut self, frame: &[u8]) -> Result<(), BbaError> {
        if frame.len() > BBA_TX_MAX_PACKET_SIZE {
            return Err(BbaError::PacketTooLarge);
        }

        let _lock = ExiLock::acquire()?;
        if !link_state()? {
            return Err(BbaError::LinkDown);
        }

        while read_u8(BBA_NCRA)? & (BBA_NCRA_ST0 | BBA_NCRA_ST1) != 0 {
            spin_loop();
        }

        write_u12(BBA_TXFIFOCNT, frame.len() as u16)?;
        let _selection = Selection::acquire()?;
        write_data_register(BBA_WRTXFIFOD)?;
        write_selected(frame)?;
        if frame.len() < BBA_MIN_PACKET_SIZE {
            // C transmits bytes from an uninitialized stack padding array here.
            // Reading those bytes is undefined behavior, so Rust deliberately
            // sends zero padding while keeping the same minimum frame length.
            let padding = [0u8; BBA_MIN_PACKET_SIZE];
            write_selected(&padding[..BBA_MIN_PACKET_SIZE - frame.len()])?;
        }
        drop(_selection);

        let control = read_u8(BBA_NCRA)?;
        write_u8(BBA_NCRA, (control & !BBA_NCRA_ST0) | BBA_NCRA_ST1)
    }

    fn initialize_locked(&mut self) -> Result<(), BbaError> {
        self.reset_locked()?;
        self.revision = cmd_read_u8(0x01)?;
        cmd_write(0x04, &self.device_id.to_be_bytes())?;
        cmd_write_u8(0x05, self.acstart)?;

        write_u8(0x5b, read_u8(0x5b)? & !0x80)?;
        write_u8(0x5e, 0x01)?;
        write_u8(0x5c, read_u8(0x5c)? | 0x04)?;

        receive_init()?;
        read_memory(BBA_NAFR_PAR0, &mut self.mac)?;
        write_u8(BBA_IR, 0xff)?;
        write_u8(BBA_IMR, !BBA_IMR_FIFOEIM)?;
        cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)
    }

    fn reset_locked(&self) -> Result<(), BbaError> {
        write_u8(0x60, 0x00)?;
        delay_microseconds(10_000);
        let _ = cmd_read_u8_slow(0x0f)?;
        delay_microseconds(10_000);
        write_u8(BBA_NCRA, BBA_NCRA_RESET)?;
        write_u8(BBA_NCRA, 0x00)
    }

    fn challenge_response(&mut self) -> Result<(), BbaError> {
        let mut count = 0usize;
        while count < 100 {
            count += 1;
            let _lock = match ExiLock::acquire() {
                Ok(lock) => lock,
                Err(BbaError::ExiLock) => continue,
                Err(error) => return Err(error),
            };
            let status = cmd_read_u8(0x03)?;
            if status == 0 {
                continue;
            }
            cmd_write_u8(0x02, BBA_CMD_IRMASK_ALL)?;
            if status & 0x10 != 0 {
                self.answer_challenge_locked()?;
                cmd_write_u8(0x03, 0x10)?;
                cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
                count = 0;
            } else if status & 0x08 != 0 {
                cmd_write_u8(0x03, 0x08)?;
                cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
                return Ok(());
            } else {
                cmd_write_u8(0x02, BBA_CMD_IRMASK_NONE)?;
            }
        }
        Err(BbaError::Challenge)
    }

    fn answer_challenge_locked(&self) -> Result<(), BbaError> {
        cmd_write_u8(0x05, self.acstart)?;
        let mut challenge_bytes = [0u8; 4];
        cmd_read(0x08, &mut challenge_bytes)?;
        let challenge = u32::from_be_bytes(challenge_bytes);
        let response = calculate_response(self.revision, self.device_id, challenge);
        cmd_write(0x09, &response.to_be_bytes())
    }
}

fn delay_microseconds(microseconds: u64) {
    if microseconds == 0 {
        return;
    }
    let ticks = (TICKS_PER_SECOND.saturating_mul(microseconds) / 1_000_000).max(1);
    let start = now_ticks();
    while now_ticks().wrapping_sub(start) < ticks {
        spin_loop();
    }
}

fn read_cid() -> Result<u32, BbaError> {
    let _selection = Selection::acquire()?;
    let mut command = [0u8; 2];
    transfer_immediate(&mut command, exi::EXI_WRITE as i32)?;
    let mut cid = [0u8; 4];
    transfer_immediate(&mut cid, exi::EXI_READ as i32)?;
    Ok(u32::from_be_bytes(cid))
}

fn receive_init() -> Result<(), BbaError> {
    write_u8(BBA_NCRB, BBA_NCRB_CA | BBA_NCRB_AB)?;
    write_u8(BBA_MISC2, BBA_MISC2_AUTORCVR)?;
    write_u12(BBA_TLBP, BBA_INIT_TLBP)?;
    write_u12(BBA_BP, BBA_INIT_BP)?;
    write_u12(BBA_RWP, BBA_INIT_RWP)?;
    write_u12(BBA_RRP, BBA_INIT_RRP)?;
    write_u12(BBA_RHBP, BBA_INIT_RHBP)?;
    write_u8(BBA_GCA, BBA_GCA_ARXERRB)?;
    write_u8(BBA_NCRA, BBA_NCRA_SR)
}

fn link_state() -> Result<bool, BbaError> {
    let status = read_u8(BBA_NWAYS)?;
    Ok(status & (BBA_NWAYS_LS10 | BBA_NWAYS_LS100) != 0)
}

fn receive_one_locked(frame: &mut [u8]) -> Result<Option<usize>, BbaError> {
    // Bound malformed-frame skipping to the C driver's receive budget. Unlike
    // the C loop, an invalid descriptor advances RRP. The C path can otherwise
    // spin forever on the same corrupt descriptor; that hang is not useful
    // parity and is kept as a narrow safety/robustness divergence.
    for _ in 0..16 {
        let rwp = read_u12(BBA_RWP)?;
        let rrp = read_u12(BBA_RRP)?;
        if rrp == rwp {
            return Ok(None);
        }

        let mut descriptor_bytes = [0u8; 4];
        read_memory((rrp as u32) << 8, &mut descriptor_bytes)?;
        let descriptor = decode_descriptor(descriptor_bytes);
        let next = descriptor.next_packet_ptr & 0x0fff;

        let packet_len = descriptor.packet_len as usize;
        if packet_len < 4 || packet_len - 4 > BBA_RX_MAX_PACKET_SIZE {
            write_u12(BBA_RRP, next)?;
            continue;
        }
        if descriptor.status & (BBA_RX_STATUS_RERR | BBA_RX_STATUS_FAE) != 0 {
            write_u12(BBA_RRP, next)?;
            continue;
        }

        let payload_len = packet_len - 4;
        if frame.len() < payload_len {
            return Err(BbaError::BufferTooSmall);
        }
        let payload_start = ((rrp as usize) << 8) + 4;
        read_ring(payload_start, &mut frame[..payload_len])?;
        write_u12(BBA_RRP, next)?;
        return Ok(Some(payload_len));
    }
    Ok(None)
}

fn read_ring(start: usize, output: &mut [u8]) -> Result<(), BbaError> {
    let first_len = output.len().min(BBA_RX_RING_TOP.saturating_sub(start));
    if first_len != 0 {
        read_memory(start as u32, &mut output[..first_len])?;
    }
    if first_len < output.len() {
        read_memory(BBA_RX_RING_BOTTOM as u32, &mut output[first_len..])?;
    }
    Ok(())
}

fn clear_interrupts(status: u8) -> Result<(), BbaError> {
    for bit in [
        BBA_IR_FRAGI,
        BBA_IR_RI,
        BBA_IR_REI,
        BBA_IR_TI,
        BBA_IR_TEI,
        BBA_IR_FIFOEI,
        BBA_IR_BUSEI,
        BBA_IR_RBFI,
    ] {
        if status & bit != 0 {
            write_u8(BBA_IR, bit)?;
        }
    }
    Ok(())
}

fn decode_descriptor(bytes: [u8; 4]) -> RxDescriptor {
    // The C code reads the descriptor as a 32-bit value, byte-swaps it from
    // little endian, then accesses a PowerPC big-endian bitfield laid out as
    // status:8, packet_len:12, next_packet_ptr:12.
    let raw = u32::from_le_bytes(bytes);
    RxDescriptor {
        status: (raw >> 24) as u8,
        packet_len: ((raw >> 12) & 0x0fff) as u16,
        next_packet_ptr: (raw & 0x0fff) as u16,
    }
}

fn calculate_response(revision: u8, device_id: u16, value: u32) -> u32 {
    let i0 = (value >> 24) as u8;
    let i1 = (value >> 16) as u8;
    let i2 = (value >> 8) as u8;
    let i3 = value as u8;
    let device_hi = (device_id >> 8) as u8;
    let device_lo = device_id as u8;

    let c0 = i0
        .wrapping_add(i1.wrapping_mul(0xc1))
        .wrapping_add(0x18)
        .wrapping_add(revision)
        ^ i3.wrapping_mul(i2).wrapping_add(0x90);
    let c1 = i1.wrapping_add(i2).wrapping_add(0x90) ^ c0.wrapping_add(i0).wrapping_sub(0xc1);
    let c2 = i2.wrapping_add(0xc8)
        ^ c0.wrapping_add((device_hi.wrapping_add(revision.wrapping_mul(0x23))) ^ 0x19);
    let c3 = i0.wrapping_add(0xc1) ^ i3.wrapping_add((device_lo.wrapping_add(0xc8)) ^ 0x90);

    u32::from_be_bytes([c0, c1, c2, c3])
}

fn cmd_read_u8(reg: u32) -> Result<u8, BbaError> {
    let mut byte = [0u8; 1];
    cmd_read(reg, &mut byte)?;
    Ok(byte[0])
}

fn cmd_read_u8_slow(reg: u32) -> Result<u8, BbaError> {
    let _selection = Selection::acquire()?;
    let mut request = ((reg as u16) << 8).to_be_bytes();
    transfer_immediate(&mut request, exi::EXI_WRITE as i32)?;
    let mut byte = [0u8; 1];
    transfer_extended(&mut byte, exi::EXI_READ as i32)?;
    delay_microseconds(200);
    Ok(byte[0])
}

fn cmd_read(reg: u32, output: &mut [u8]) -> Result<(), BbaError> {
    let _selection = Selection::acquire()?;
    let mut request = ((reg as u16) << 8).to_be_bytes();
    transfer_immediate(&mut request, exi::EXI_WRITE as i32)?;
    transfer_extended(output, exi::EXI_READ as i32)
}

fn cmd_write_u8(reg: u32, value: u8) -> Result<(), BbaError> {
    cmd_write(reg, &[value])
}

fn cmd_write(reg: u32, input: &[u8]) -> Result<(), BbaError> {
    let _selection = Selection::acquire()?;
    let mut request = (((reg as u16) << 8) | 0x4000).to_be_bytes();
    transfer_immediate(&mut request, exi::EXI_WRITE as i32)?;
    write_selected(input)
}

fn read_u8(reg: u32) -> Result<u8, BbaError> {
    let mut byte = [0u8; 1];
    read_memory(reg, &mut byte)?;
    Ok(byte[0])
}

fn write_u8(reg: u32, value: u8) -> Result<(), BbaError> {
    write_memory(reg, &[value])
}

fn read_u12(reg: u32) -> Result<u16, BbaError> {
    let low = read_u8(reg)? as u16;
    let high = (read_u8(reg + 1)? as u16) & 0x0f;
    Ok(low | (high << 8))
}

fn write_u12(reg: u32, value: u16) -> Result<(), BbaError> {
    write_u8(reg, value as u8)?;
    write_u8(reg + 1, ((value >> 8) & 0x0f) as u8)
}

fn read_memory(reg: u32, output: &mut [u8]) -> Result<(), BbaError> {
    let _selection = Selection::acquire()?;
    read_data_register(reg)?;
    transfer_extended(output, exi::EXI_READ as i32)
}

fn write_memory(reg: u32, input: &[u8]) -> Result<(), BbaError> {
    let _selection = Selection::acquire()?;
    write_data_register(reg)?;
    write_selected(input)
}

fn read_data_register(reg: u32) -> Result<(), BbaError> {
    let mut request = ((reg << 8) | 0x8000_0000).to_be_bytes();
    transfer_immediate(&mut request, exi::EXI_WRITE as i32)
}

fn write_data_register(reg: u32) -> Result<(), BbaError> {
    let mut request = ((reg << 8) | 0xc000_0000).to_be_bytes();
    transfer_immediate(&mut request, exi::EXI_WRITE as i32)
}

fn write_selected(input: &[u8]) -> Result<(), BbaError> {
    if input.is_empty() {
        return Ok(());
    }
    // EXI's C ABI takes a mutable pointer for both read and write transfers.
    // EXI_WRITE does not modify the caller's bytes.
    let ptr = input.as_ptr() as *mut u8;
    if unsafe {
        exi::imm_ex(
            CHANNEL,
            ptr.cast::<c_void>(),
            input.len() as u32,
            exi::EXI_WRITE as i32,
        )
    } == 0
    {
        Err(BbaError::ExiTransfer)
    } else {
        Ok(())
    }
}

fn transfer_extended(buffer: &mut [u8], mode: i32) -> Result<(), BbaError> {
    if buffer.is_empty() {
        return Ok(());
    }
    if unsafe {
        exi::imm_ex(
            CHANNEL,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len() as u32,
            mode,
        )
    } == 0
    {
        Err(BbaError::ExiTransfer)
    } else {
        Ok(())
    }
}

fn transfer_immediate(buffer: &mut [u8], mode: i32) -> Result<(), BbaError> {
    if unsafe {
        exi::imm(
            CHANNEL,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len() as u32,
            mode,
            None,
        )
    } == 0
    {
        return Err(BbaError::ExiTransfer);
    }
    if exi::sync(CHANNEL) == 0 {
        Err(BbaError::ExiTransfer)
    } else {
        Ok(())
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn descriptor_decode_matches_c_bitfield_layout() {
        let raw = (0x84u32 << 24) | (0x5a3u32 << 12) | 0x0bcdu32;
        let descriptor = decode_descriptor(raw.to_le_bytes());
        assert_eq!(descriptor.status, 0x84);
        assert_eq!(descriptor.packet_len, 0x5a3);
        assert_eq!(descriptor.next_packet_ptr, 0x0bcd);
    }

    #[test]
    fn challenge_response_uses_wrapping_byte_arithmetic() {
        assert_eq!(calculate_response(0x12, 0xd107, 0x89ab_cdef), 0x6d3d_5e04);
    }

    #[test]
    fn transport_limits_match_reference_driver() {
        assert_eq!(BBA_TX_MAX_PACKET_SIZE, 1518);
        assert_eq!(BBA_RX_MAX_PACKET_SIZE, 1540);
        assert_eq!(BBA_MIN_PACKET_SIZE, 60);
    }
}
