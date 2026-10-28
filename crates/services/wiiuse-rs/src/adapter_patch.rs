//! Bluetooth adapter firmware patch sequence used by the Wii Bluetooth host.
//!
//! The command order and payloads match libogc's `lwbt/bte.c` patch path.
//! The firmware blocks are distributed under `LICENSE-libogc`.

const VENDOR_OGF: u16 = 0x3f;
const PATCH_START_OCF: u16 = 0x4f;
const PATCH_CONTINUE_OCF: u16 = 0x4c;
const RESET_OGF: u16 = 0x03;
const RESET_OCF: u16 = 0x03;

const PATCH_KICK: &[u8] = &[0];
const PATCH_BLOCK_0: &[u8; 184] = &[
    0x70, 0x99, 0x08, 0x00, 0x88, 0x43, 0xd1, 0x07, 0x09, 0x0c, 0x08, 0x43, 0xa0, 0x62, 0x19, 0x23,
    0xdb, 0x01, 0x33, 0x80, 0x7c, 0xf7, 0x88, 0xf8, 0x28, 0x76, 0x80, 0xf7, 0x17, 0xff, 0x43, 0x78,
    0xeb, 0x70, 0x19, 0x23, 0xdb, 0x01, 0x33, 0x87, 0x7c, 0xf7, 0xbc, 0xfb, 0x0b, 0x60, 0xa3, 0x7b,
    0x01, 0x49, 0x0b, 0x60, 0x90, 0xf7, 0x96, 0xfb, 0xd8, 0x1d, 0x08, 0x00, 0x00, 0xf0, 0x04, 0xf8,
    0x00, 0x23, 0x79, 0xf7, 0xe3, 0xfa, 0x00, 0x00, 0x00, 0xb5, 0x00, 0x23, 0x11, 0x49, 0x0b, 0x60,
    0x1d, 0x21, 0xc9, 0x03, 0x0b, 0x60, 0x7d, 0x20, 0x80, 0x01, 0x01, 0x38, 0xfd, 0xd1, 0x0e, 0x4b,
    0x0e, 0x4a, 0x13, 0x60, 0x47, 0x20, 0x00, 0x21, 0x96, 0xf7, 0x96, 0xff, 0x46, 0x20, 0x00, 0x21,
    0x96, 0xf7, 0x92, 0xff, 0x0a, 0x4a, 0x13, 0x68, 0x0a, 0x48, 0x03, 0x40, 0x13, 0x60, 0x0a, 0x4a,
    0x13, 0x68, 0x0a, 0x48, 0x03, 0x40, 0x13, 0x60, 0x09, 0x4a, 0x13, 0x68, 0x09, 0x48, 0x03, 0x40,
    0x13, 0x60, 0x00, 0xbd, 0x24, 0x80, 0x0e, 0x00, 0x81, 0x03, 0x0f, 0xfe, 0x5c, 0x00, 0x0f, 0x00,
    0x60, 0xfc, 0x0e, 0x00, 0xfe, 0xff, 0x00, 0x00, 0xfc, 0xfc, 0x0e, 0x00, 0xff, 0x9f, 0x00, 0x00,
    0x30, 0xfc, 0x0e, 0x00, 0x7f, 0xff, 0x00, 0x00,
];
const PATCH_BLOCK_1: &[u8; 92] = &[
    0x07, 0x20, 0xbc, 0x65, 0x01, 0x00, 0x84, 0x42, 0x09, 0xd2, 0x84, 0x42, 0x09, 0xd1, 0x21, 0x84,
    0x5a, 0x00, 0x00, 0x83, 0xf0, 0x74, 0xff, 0x09, 0x0c, 0x08, 0x43, 0x22, 0x00, 0x61, 0x00, 0x00,
    0x83, 0xf0, 0x40, 0xfc, 0x00, 0x00, 0x00, 0x00, 0x23, 0xcc, 0x9f, 0x01, 0x00, 0x6f, 0xf0, 0xe4,
    0xfc, 0x03, 0x28, 0x7d, 0xd1, 0x24, 0x3c, 0x62, 0x01, 0x00, 0x28, 0x20, 0x00, 0xe0, 0x60, 0x8d,
    0x23, 0x68, 0x25, 0x04, 0x12, 0x01, 0x00, 0x20, 0x1c, 0x20, 0x1c, 0x24, 0xe0, 0xb0, 0x21, 0x26,
    0x74, 0x2f, 0x00, 0x00, 0x86, 0xf0, 0x18, 0xfd, 0x21, 0x4f, 0x3b, 0x60,
];

/// A command in the adapter patch or a request to reconfigure after reset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterPatchAction {
    /// Send the HCI command with this OGF, OCF, and parameter block.
    Command {
        ogf: u16,
        ocf: u16,
        params: &'static [u8],
    },
    /// Continue the existing host initialization after the patch reset.
    Reconfigure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterPatchError {
    InvalidState,
    UnexpectedOpcode { expected: u16, received: u16 },
    CommandFailed { status: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Idle,
    AwaitingStart,
    AwaitingBlock0,
    AwaitingBlock1,
    AwaitingReset,
    Reconfiguring,
    Complete,
    Failed,
}

/// Tracks the controller firmware patch through checked HCI completions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterPatch {
    phase: Phase,
}

impl AdapterPatch {
    pub const fn new() -> Self {
        Self { phase: Phase::Idle }
    }

    /// Begin the vendor patch. Call only after stored link keys are read.
    pub fn start(&mut self) -> Result<AdapterPatchAction, AdapterPatchError> {
        if self.phase != Phase::Idle {
            return Err(AdapterPatchError::InvalidState);
        }
        self.phase = Phase::AwaitingStart;
        Ok(Self::command(PATCH_START_OCF, PATCH_KICK))
    }

    /// Validate one command completion and return the next required action.
    pub fn command_complete(
        &mut self,
        opcode: u16,
        status: u8,
    ) -> Result<AdapterPatchAction, AdapterPatchError> {
        let expected = match self.phase {
            Phase::AwaitingStart | Phase::AwaitingBlock1 => hci_opcode(VENDOR_OGF, PATCH_START_OCF),
            Phase::AwaitingBlock0 => hci_opcode(VENDOR_OGF, PATCH_CONTINUE_OCF),
            Phase::AwaitingReset => hci_opcode(RESET_OGF, RESET_OCF),
            Phase::Idle | Phase::Reconfiguring | Phase::Complete | Phase::Failed => {
                return Err(AdapterPatchError::InvalidState);
            }
        };
        if opcode != expected {
            return Err(AdapterPatchError::UnexpectedOpcode {
                expected,
                received: opcode,
            });
        }
        if status != 0 {
            self.phase = Phase::Failed;
            return Err(AdapterPatchError::CommandFailed { status });
        }

        match self.phase {
            Phase::AwaitingStart => {
                self.phase = Phase::AwaitingBlock0;
                Ok(Self::command(PATCH_CONTINUE_OCF, PATCH_BLOCK_0))
            }
            Phase::AwaitingBlock0 => {
                self.phase = Phase::AwaitingBlock1;
                Ok(Self::command(PATCH_START_OCF, PATCH_BLOCK_1))
            }
            Phase::AwaitingBlock1 => {
                self.phase = Phase::AwaitingReset;
                Ok(AdapterPatchAction::Command {
                    ogf: RESET_OGF,
                    ocf: RESET_OCF,
                    params: &[],
                })
            }
            Phase::AwaitingReset => {
                self.phase = Phase::Reconfiguring;
                Ok(AdapterPatchAction::Reconfigure)
            }
            Phase::Idle | Phase::Reconfiguring | Phase::Complete | Phase::Failed => {
                Err(AdapterPatchError::InvalidState)
            }
        }
    }

    /// Mark completion only after the Bluetooth host reconfiguration succeeds.
    pub fn reconfiguration_complete(&mut self) -> Result<(), AdapterPatchError> {
        if self.phase != Phase::Reconfiguring {
            return Err(AdapterPatchError::InvalidState);
        }
        self.phase = Phase::Complete;
        Ok(())
    }

    pub const fn is_complete(&self) -> bool {
        matches!(self.phase, Phase::Complete)
    }

    const fn command(ocf: u16, params: &'static [u8]) -> AdapterPatchAction {
        AdapterPatchAction::Command {
            ogf: VENDOR_OGF,
            ocf,
            params,
        }
    }
}

impl Default for AdapterPatch {
    fn default() -> Self {
        Self::new()
    }
}

const fn hci_opcode(ogf: u16, ocf: u16) -> u16 {
    (ogf << 10) | (ocf & 0x03ff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }

    fn command(action: AdapterPatchAction) -> (u16, &'static [u8]) {
        match action {
            AdapterPatchAction::Command { ogf, ocf, params } => (hci_opcode(ogf, ocf), params),
            AdapterPatchAction::Reconfigure => panic!("expected an HCI command"),
        }
    }

    #[test]
    fn adapter_patch_uses_exact_payloads_and_command_order() {
        let mut patch = AdapterPatch::new();
        let (opcode, params) = command(patch.start().unwrap());
        assert_eq!(opcode, hci_opcode(VENDOR_OGF, PATCH_START_OCF));
        assert_eq!(params, &[0]);

        let action = patch.command_complete(opcode, 0).unwrap();
        let (opcode, params) = command(action);
        assert_eq!(opcode, hci_opcode(VENDOR_OGF, PATCH_CONTINUE_OCF));
        assert_eq!(params.len(), 184);
        assert_eq!(crc32(params), 0xf138_06d1);

        let action = patch.command_complete(opcode, 0).unwrap();
        let (opcode, params) = command(action);
        assert_eq!(opcode, hci_opcode(VENDOR_OGF, PATCH_START_OCF));
        assert_eq!(params.len(), 92);
        assert_eq!(crc32(params), 0x070a_34e3);

        let action = patch.command_complete(opcode, 0).unwrap();
        let (opcode, params) = command(action);
        assert_eq!(opcode, hci_opcode(RESET_OGF, RESET_OCF));
        assert!(params.is_empty());
        assert_eq!(
            patch.command_complete(opcode, 0),
            Ok(AdapterPatchAction::Reconfigure)
        );
        assert!(!patch.is_complete());
        patch.reconfiguration_complete().unwrap();
        assert!(patch.is_complete());
    }

    #[test]
    fn adapter_patch_rejects_wrong_completion_and_nonzero_status() {
        let mut patch = AdapterPatch::new();
        let _ = patch.start().unwrap();
        assert_eq!(
            patch.command_complete(0x0c03, 0),
            Err(AdapterPatchError::UnexpectedOpcode {
                expected: hci_opcode(VENDOR_OGF, PATCH_START_OCF),
                received: 0x0c03,
            })
        );
        assert_eq!(
            patch.command_complete(hci_opcode(VENDOR_OGF, PATCH_START_OCF), 0x0c),
            Err(AdapterPatchError::CommandFailed { status: 0x0c })
        );
        assert_eq!(
            patch.command_complete(hci_opcode(VENDOR_OGF, PATCH_START_OCF), 0),
            Err(AdapterPatchError::InvalidState)
        );
    }
}
