//! Wii Remote memory-read report assembly.

const READ_DATA_BUFFER_BYTES: usize = 224;
const COVERAGE_WORDS: usize = READ_DATA_BUFFER_BYTES.div_ceil(u32::BITS as usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ReadFragmentResult {
    Incomplete,
    Complete(usize),
    Error(u8),
}

/// Collects one Wii Remote memory read and tracks which requested bytes arrived.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ReadAssembly {
    addr: u32,
    requested: usize,
    received: usize,
    buffer: [u8; READ_DATA_BUFFER_BYTES],
    covered: [u32; COVERAGE_WORDS],
}

impl ReadAssembly {
    pub const fn new(addr: u32, requested: usize) -> Self {
        Self {
            addr,
            requested,
            received: 0,
            buffer: [0; READ_DATA_BUFFER_BYTES],
            covered: [0; COVERAGE_WORDS],
        }
    }

    pub const fn requested(&self) -> usize {
        self.requested
    }

    pub const fn received(&self) -> usize {
        self.received
    }

    pub const fn buffer(&self) -> &[u8; READ_DATA_BUFFER_BYTES] {
        &self.buffer
    }

    /// Add one `0x21` report fragment.
    ///
    /// Duplicate bytes are accepted only when their contents match. The read
    /// completes after every requested byte arrives, regardless of fragment order.
    pub fn apply_report(&mut self, report: &[u8]) -> ReadFragmentResult {
        if report.len() < 6 || report[0] != 0x21 {
            return ReadFragmentResult::Error(0xff);
        }

        // Report ID and both core button bytes precede read status.
        let status = report[3];
        let error = status & 0x0f;
        if error != 0 {
            return ReadFragmentResult::Error(error);
        }

        let len = usize::from(status >> 4) + 1;
        if report.len() < 6 + len {
            return ReadFragmentResult::Incomplete;
        }
        if self.requested == 0 || self.requested > READ_DATA_BUFFER_BYTES {
            return ReadFragmentResult::Error(0xfe);
        }

        let offset = u16::from_be_bytes([report[4], report[5]]) as usize;
        // Read reports contain the low 16 bits of the requested memory address.
        let base = (self.addr & 0xffff) as usize;
        if offset < base {
            return ReadFragmentResult::Error(0xfe);
        }
        let start = offset - base;
        let end = match start.checked_add(len) {
            Some(end) if end <= self.requested => end,
            _ => return ReadFragmentResult::Error(0xfe),
        };
        let data = &report[6..6 + len];

        if (start..end)
            .zip(data)
            .any(|(index, byte)| self.is_covered(index) && self.buffer[index] != *byte)
        {
            return ReadFragmentResult::Error(0xfd);
        }

        for (index, byte) in (start..end).zip(data) {
            if !self.is_covered(index) {
                self.buffer[index] = *byte;
                self.mark_covered(index);
                self.received += 1;
            }
        }

        if self.received == self.requested {
            ReadFragmentResult::Complete(self.requested)
        } else {
            ReadFragmentResult::Incomplete
        }
    }

    fn is_covered(&self, index: usize) -> bool {
        self.covered[index / u32::BITS as usize] & (1 << (index % u32::BITS as usize)) != 0
    }

    fn mark_covered(&mut self, index: usize) {
        self.covered[index / u32::BITS as usize] |= 1 << (index % u32::BITS as usize);
    }
}

impl Default for ReadAssembly {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HandshakeAction, InitialHandshake};

    #[test]
    fn calibration_report_keeps_both_button_bytes_out_of_read_header() {
        let mut read = ReadAssembly::new(0x16, 8);
        // ID, two nonzero button bytes, size/error, address, eight data bytes.
        let report = [
            0x21, 0x08, 0x01, 0x70, 0x00, 0x16, 0x80, 0x81, 0x82, 0, 0xa0, 0xa1, 0xa2, 0,
        ];
        assert_eq!(read.apply_report(&report), ReadFragmentResult::Complete(8));
        assert_eq!(
            &read.buffer()[..8],
            &[0x80, 0x81, 0x82, 0, 0xa0, 0xa1, 0xa2, 0]
        );
        assert_eq!(read.received(), 8);
    }

    #[test]
    fn duplicate_fragments_do_not_claim_missing_bytes() {
        let mut read = ReadAssembly::new(0, 8);
        let fragment = [0x21, 0, 0, 0x30, 0, 0, 0xaa, 0xbb, 0xcc, 0xdd];
        assert_eq!(read.apply_report(&fragment), ReadFragmentResult::Incomplete);
        assert_eq!(read.apply_report(&fragment), ReadFragmentResult::Incomplete);
        assert_eq!(read.received(), 4);
        assert_eq!(&read.buffer()[..8], &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0]);
    }

    #[test]
    fn out_of_order_fragments_complete_when_all_requested_bytes_arrive() {
        let mut read = ReadAssembly::new(0, 8);
        assert_eq!(read.requested(), 8);
        let upper = [0x21, 0, 0, 0x30, 0, 4, 5, 6, 7, 8];
        let lower = [0x21, 0, 0, 0x30, 0, 0, 1, 2, 3, 4];
        assert_eq!(read.apply_report(&upper), ReadFragmentResult::Incomplete);
        assert_eq!(read.apply_report(&lower), ReadFragmentResult::Complete(8));
        assert_eq!(&read.buffer()[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn conflicting_overlap_fails_without_changing_assembled_bytes() {
        let mut read = ReadAssembly::new(0, 8);
        let first = [0x21, 0, 0, 0x30, 0, 0, 1, 2, 3, 4];
        let conflict = [0x21, 0, 0, 0x30, 0, 0, 1, 2, 3, 9];
        assert_eq!(read.apply_report(&first), ReadFragmentResult::Incomplete);
        assert_eq!(
            read.apply_report(&conflict),
            ReadFragmentResult::Error(0xfd)
        );
        assert_eq!(&read.buffer()[..4], &[1, 2, 3, 4]);
        assert_eq!(read.received(), 4);
    }

    #[test]
    fn fragments_outside_the_requested_range_are_rejected() {
        let mut read = ReadAssembly::new(0x04a4_00fa, 6);
        let valid = [0x21, 0, 0, 0x50, 0x00, 0xfa, 1, 2, 3, 4, 5, 6];
        assert_eq!(read.apply_report(&valid), ReadFragmentResult::Complete(6));
        let extra = [0x21, 0, 0, 0x10, 0x01, 0x00, 7, 8];
        assert_eq!(read.apply_report(&extra), ReadFragmentResult::Error(0xfe));
    }

    #[test]
    fn failed_initial_calibration_read_restarts_status_handshake() {
        let mut handshake = InitialHandshake::new();
        assert_eq!(
            handshake.status_report(false),
            Ok(HandshakeAction::ReadCalibration)
        );

        let mut calibration = ReadAssembly::new(0x16, 8);
        let failed_report = [0x21, 0, 0, 0x01, 0, 0];
        assert_eq!(
            calibration.apply_report(&failed_report),
            ReadFragmentResult::Error(1)
        );

        handshake.retry();
        assert_eq!(
            handshake.status_report(false),
            Ok(HandshakeAction::ReadCalibration)
        );
    }
}
