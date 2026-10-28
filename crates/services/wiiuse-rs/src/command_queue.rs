pub const WII_COMMAND_QUEUE_CAPACITY: usize = 32;
pub const WII_COMMAND_MAX_BYTES: usize = 22;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckReport {
    pub command: u8,
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpansionSetupAction {
    WriteEnableOne,
    WriteEnableTwo,
    ReadCalibration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandTag {
    General,
    ExpansionEnableOne,
    ExpansionEnableTwo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpansionSetupStep {
    Idle,
    AwaitEnableOneAck,
    AwaitEnableTwoAck,
    AwaitCalibration,
}

/// Tracks the three asynchronous steps that enable and read an expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpansionSetup {
    step: ExpansionSetupStep,
}

impl ExpansionSetup {
    pub const fn new() -> Self {
        Self {
            step: ExpansionSetupStep::Idle,
        }
    }

    pub fn start(&mut self) -> Option<ExpansionSetupAction> {
        if self.step != ExpansionSetupStep::Idle {
            return None;
        }
        self.step = ExpansionSetupStep::AwaitEnableOneAck;
        Some(ExpansionSetupAction::WriteEnableOne)
    }

    /// Advance only when the active write receives its matching ACK.
    pub fn acknowledge(&mut self, ack: AckReport) -> Option<ExpansionSetupAction> {
        if ack.command != 0x16 {
            return None;
        }
        match self.step {
            ExpansionSetupStep::AwaitEnableOneAck => {
                self.step = ExpansionSetupStep::AwaitEnableTwoAck;
                Some(ExpansionSetupAction::WriteEnableTwo)
            }
            ExpansionSetupStep::AwaitEnableTwoAck => {
                self.step = ExpansionSetupStep::AwaitCalibration;
                Some(ExpansionSetupAction::ReadCalibration)
            }
            ExpansionSetupStep::Idle | ExpansionSetupStep::AwaitCalibration => None,
        }
    }

    pub fn complete(&mut self) {
        self.step = ExpansionSetupStep::Idle;
    }

    pub const fn is_active(&self) -> bool {
        !matches!(self.step, ExpansionSetupStep::Idle)
    }
}

impl Default for ExpansionSetup {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode the complete five-byte `0x22` report, including both button bytes.
pub fn decode_ack_report(report: &[u8]) -> Option<AckReport> {
    if report.len() < 5 || report[0] != 0x22 {
        return None;
    }
    Some(AckReport {
        command: report[3],
        status: report[4],
    })
}

pub fn encode_wii_write_command(addr: u32, data: &[u8]) -> Option<[u8; WII_COMMAND_MAX_BYTES]> {
    if data.is_empty() || data.len() > 16 {
        return None;
    }
    let mut raw = [0u8; WII_COMMAND_MAX_BYTES];
    raw[0] = 0x16;
    raw[1..5].copy_from_slice(&addr.to_be_bytes());
    raw[5] = (data.len() & 0x0f) as u8;
    raw[6..6 + data.len()].copy_from_slice(data);
    Some(raw)
}

pub fn encode_wii_read_command(addr: u32, len: u16) -> Option<[u8; 7]> {
    if len == 0 {
        return None;
    }
    let mut raw = [0u8; 7];
    raw[0] = 0x17;
    raw[1..5].copy_from_slice(&addr.to_be_bytes());
    raw[5..7].copy_from_slice(&len.to_be_bytes());
    Some(raw)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandQueueError {
    EmptyCommand,
    CommandTooLong,
    Full,
    UnexpectedCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandEnqueue {
    Started,
    Queued,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AckCompletion {
    Ignored,
    Completed {
        failed: bool,
        recovery_queued: bool,
        completed: WiiCommand,
        next: Option<WiiCommand>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WiiCommand {
    bytes: [u8; WII_COMMAND_MAX_BYTES],
    len: u8,
    tag: CommandTag,
}

impl WiiCommand {
    const EMPTY: Self = Self {
        bytes: [0; WII_COMMAND_MAX_BYTES],
        len: 0,
        tag: CommandTag::General,
    };

    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    pub fn command(&self) -> u8 {
        self.bytes[0]
    }

    pub const fn tag(&self) -> CommandTag {
        self.tag
    }

    fn from_bytes(bytes: &[u8], tag: CommandTag) -> Result<Self, CommandQueueError> {
        if bytes.is_empty() {
            return Err(CommandQueueError::EmptyCommand);
        }
        if bytes.len() > WII_COMMAND_MAX_BYTES {
            return Err(CommandQueueError::CommandTooLong);
        }
        let mut command = Self::EMPTY;
        command.bytes[..bytes.len()].copy_from_slice(bytes);
        command.len = bytes.len() as u8;
        command.tag = tag;
        Ok(command)
    }
}

/// Serializes Wii Remote output commands until their ACK, status, or read reply.
///
/// C's `wiiuse_send_next_command` sends one command and waits for its matching
/// report before advancing the linked command queue. This fixed-capacity queue
/// provides the same ordering without allocation in the PowerPC runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WiiCommandQueue {
    commands: [WiiCommand; WII_COMMAND_QUEUE_CAPACITY],
    len: u8,
}

impl WiiCommandQueue {
    pub const fn new() -> Self {
        Self {
            commands: [WiiCommand::EMPTY; WII_COMMAND_QUEUE_CAPACITY],
            len: 0,
        }
    }

    pub fn enqueue(&mut self, bytes: &[u8]) -> Result<CommandEnqueue, CommandQueueError> {
        self.enqueue_tagged(bytes, CommandTag::General)
    }

    pub fn enqueue_tagged(
        &mut self,
        bytes: &[u8],
        tag: CommandTag,
    ) -> Result<CommandEnqueue, CommandQueueError> {
        let command = WiiCommand::from_bytes(bytes, tag)?;
        let len = usize::from(self.len);
        if len == WII_COMMAND_QUEUE_CAPACITY {
            return Err(CommandQueueError::Full);
        }
        self.commands[len] = command;
        self.len += 1;
        Ok(if len == 0 {
            CommandEnqueue::Started
        } else {
            CommandEnqueue::Queued
        })
    }

    pub fn active(&self) -> Option<&WiiCommand> {
        (self.len != 0).then(|| &self.commands[0])
    }

    /// Complete an ACKed command only when its report ID matches the head.
    /// Apply C `event_ack` ordering, including status recovery after pending work.
    pub fn complete_ack_report(
        &mut self,
        ack: AckReport,
        recovery_status: &[u8],
    ) -> Result<AckCompletion, CommandQueueError> {
        let Some(active) = self.active() else {
            return Ok(AckCompletion::Ignored);
        };
        if active.command() != ack.command || matches!(ack.command, 0x15 | 0x17) {
            return Ok(AckCompletion::Ignored);
        }
        let recovery = if ack.status != 0 {
            Some(WiiCommand::from_bytes(
                recovery_status,
                CommandTag::General,
            )?)
        } else {
            None
        };
        let completed = self.commands[0];
        self.remove_active();
        if let Some(recovery) = recovery {
            // Removing the active command made one queue slot available.
            let len = usize::from(self.len);
            self.commands[len] = recovery;
            self.len += 1;
        }
        let next = self.active().copied();
        Ok(AckCompletion::Completed {
            failed: ack.status != 0,
            recovery_queued: recovery.is_some(),
            completed,
            next,
        })
    }

    /// Complete a status or read command when its reply is complete.
    pub fn complete_response(
        &mut self,
        command_id: u8,
    ) -> Result<Option<WiiCommand>, CommandQueueError> {
        let active = self
            .active()
            .ok_or(CommandQueueError::UnexpectedCompletion)?;
        if active.command() != command_id || !matches!(command_id, 0x15 | 0x17) {
            return Err(CommandQueueError::UnexpectedCompletion);
        }
        self.remove_active();
        Ok(self.active().copied())
    }

    fn remove_active(&mut self) {
        let len = usize::from(self.len);
        self.commands.copy_within(1..len, 0);
        self.commands[len - 1] = WiiCommand::EMPTY;
        self.len -= 1;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Cancel all queued commands after an output transport rejects a send.
    pub fn clear(&mut self) {
        self.commands = [WiiCommand::EMPTY; WII_COMMAND_QUEUE_CAPACITY];
        self.len = 0;
    }
}

impl Default for WiiCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enqueue(queue: &mut WiiCommandQueue, bytes: &[u8]) {
        queue.enqueue(bytes).unwrap();
    }

    #[test]
    fn commands_wait_for_ack_status_and_read_completion_in_order() {
        let mut queue = WiiCommandQueue::new();
        assert_eq!(
            queue.enqueue_tagged(&[0x16, 0x04, 0xa4], CommandTag::ExpansionEnableOne,),
            Ok(CommandEnqueue::Started)
        );
        assert_eq!(
            queue.enqueue_tagged(&[0x16, 0x04, 0xa4, 0xfb], CommandTag::ExpansionEnableTwo,),
            Ok(CommandEnqueue::Queued)
        );
        enqueue(&mut queue, &[0x17, 0x04, 0xa4, 0x00, 0x20]);

        assert_eq!(queue.active().unwrap().command(), 0x16);
        assert_eq!(
            queue.complete_ack_report(
                AckReport {
                    command: 0x12,
                    status: 0,
                },
                &[0x15, 0],
            ),
            Ok(AckCompletion::Ignored)
        );
        assert_eq!(queue.active().unwrap().command(), 0x16);

        let AckCompletion::Completed {
            failed: true,
            recovery_queued: true,
            completed,
            next: Some(next),
        } = queue
            .complete_ack_report(
                AckReport {
                    command: 0x16,
                    status: 1,
                },
                &[0x15, 0],
            )
            .unwrap()
        else {
            panic!("failed ACK must append recovery after pending commands");
        };
        assert_eq!(completed.tag(), CommandTag::ExpansionEnableOne);
        assert_eq!(next.command(), 0x16);
        assert_eq!(queue.active().unwrap().command(), 0x16);

        let AckCompletion::Completed {
            failed: false,
            recovery_queued: false,
            completed,
            next: Some(next),
        } = queue
            .complete_ack_report(
                AckReport {
                    command: 0x16,
                    status: 0,
                },
                &[0x15, 0],
            )
            .unwrap()
        else {
            panic!("successful write ACK must start the read");
        };
        assert_eq!(completed.tag(), CommandTag::ExpansionEnableTwo);
        assert_eq!(next.command(), 0x17);
        assert_eq!(
            queue.complete_ack_report(
                AckReport {
                    command: 0x17,
                    status: 0,
                },
                &[0x15, 0],
            ),
            Ok(AckCompletion::Ignored)
        );
        assert_eq!(queue.active().unwrap().command(), 0x17);
        assert_eq!(
            queue.complete_response(0x17).unwrap().unwrap().bytes(),
            &[0x15, 0]
        );
        assert_eq!(queue.complete_response(0x15), Ok(None));
        assert!(queue.is_empty());
    }

    #[test]
    fn malformed_and_full_enqueues_leave_the_active_command_intact() {
        let mut queue = WiiCommandQueue::new();
        assert!(queue.enqueue(&[]).is_err());
        assert_eq!(
            queue.enqueue(&[0x16; WII_COMMAND_MAX_BYTES + 1]),
            Err(CommandQueueError::CommandTooLong)
        );
        assert_eq!(queue.enqueue(&[0x16]), Ok(CommandEnqueue::Started));
        for _ in 1..WII_COMMAND_QUEUE_CAPACITY {
            enqueue(&mut queue, &[0x16]);
        }
        assert_eq!(queue.enqueue(&[0x16]), Err(CommandQueueError::Full));
        assert_eq!(queue.active().unwrap().bytes(), &[0x16]);
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn failed_ack_keeps_pending_commands_and_appends_recovery_when_queue_is_full() {
        let mut queue = WiiCommandQueue::new();
        assert_eq!(queue.enqueue(&[0x16]), Ok(CommandEnqueue::Started));
        for _ in 1..WII_COMMAND_QUEUE_CAPACITY {
            enqueue(&mut queue, &[0x16]);
        }
        let AckCompletion::Completed {
            failed: true,
            recovery_queued: true,
            next: Some(next),
            ..
        } = queue
            .complete_ack_report(
                AckReport {
                    command: 0x16,
                    status: 1,
                },
                &[0x15, 0],
            )
            .unwrap()
        else {
            panic!("failed ACK must preserve the pending commands");
        };
        assert_eq!(next.command(), 0x16);
        assert_eq!(queue.active().unwrap().command(), 0x16);
        for _ in 0..WII_COMMAND_QUEUE_CAPACITY - 1 {
            queue
                .complete_ack_report(
                    AckReport {
                        command: 0x16,
                        status: 0,
                    },
                    &[0x15, 0],
                )
                .unwrap();
        }
        assert_eq!(queue.active().unwrap().bytes(), &[0x15, 0]);
    }

    #[test]
    fn ack_decoder_reads_command_and_error_after_both_button_bytes() {
        assert_eq!(
            decode_ack_report(&[0x22, 0x00, 0x00, 0x16, 0x01]),
            Some(AckReport {
                command: 0x16,
                status: 0x01,
            })
        );
        assert_eq!(decode_ack_report(&[0x22, 0x00, 0x16, 0x00]), None);
        assert_eq!(decode_ack_report(&[0x21, 0x00, 0x00, 0x16, 0]), None);
    }

    #[test]
    fn expansion_memory_commands_use_fixed_wire_addresses_and_lengths() {
        let enable_one = encode_wii_write_command(0x04a4_00f0, &[0x55]).unwrap();
        assert_eq!(
            &enable_one[..7],
            &[0x16, 0x04, 0xa4, 0x00, 0xf0, 0x01, 0x55]
        );
        assert!(enable_one[7..].iter().all(|byte| *byte == 0));

        let enable_two = encode_wii_write_command(0x04a4_00fb, &[0x00]).unwrap();
        assert_eq!(
            &enable_two[..7],
            &[0x16, 0x04, 0xa4, 0x00, 0xfb, 0x01, 0x00]
        );

        assert_eq!(
            encode_wii_read_command(0x04a4_0020, 224),
            Some([0x17, 0x04, 0xa4, 0x00, 0x20, 0x00, 0xe0])
        );
        assert!(encode_wii_write_command(0x04a4_00f0, &[]).is_none());
        assert!(encode_wii_write_command(0x04a4_00f0, &[0; 17]).is_none());
        assert!(encode_wii_read_command(0x04a4_0020, 0).is_none());
    }

    #[test]
    fn expansion_setup_waits_for_each_write_ack_before_the_next_operation() {
        let mut setup = ExpansionSetup::new();
        assert_eq!(setup.start(), Some(ExpansionSetupAction::WriteEnableOne));
        assert_eq!(setup.start(), None);
        assert_eq!(
            setup.acknowledge(AckReport {
                command: 0x12,
                status: 0,
            }),
            None
        );
        assert_eq!(
            setup.acknowledge(AckReport {
                command: 0x16,
                status: 1,
            }),
            Some(ExpansionSetupAction::WriteEnableTwo)
        );
        assert_eq!(
            setup.acknowledge(AckReport {
                command: 0x16,
                status: 0,
            }),
            Some(ExpansionSetupAction::ReadCalibration)
        );
        assert_eq!(
            setup.acknowledge(AckReport {
                command: 0x16,
                status: 0,
            }),
            None
        );
        assert!(setup.is_active());
        setup.complete();
        assert!(!setup.is_active());
    }

    #[test]
    fn expansion_setup_queues_each_exact_operation_after_its_ack_and_recovery_status() {
        let mut setup = ExpansionSetup::new();
        let mut queue = WiiCommandQueue::new();

        assert_eq!(setup.start(), Some(ExpansionSetupAction::WriteEnableOne));
        let first = encode_wii_write_command(0x04a4_00f0, &[0x55]).unwrap();
        assert_eq!(
            queue.enqueue_tagged(&first, CommandTag::ExpansionEnableOne),
            Ok(CommandEnqueue::Started)
        );

        let failed = AckReport {
            command: 0x16,
            status: 1,
        };
        let AckCompletion::Completed {
            completed,
            next: Some(status),
            failed: true,
            recovery_queued: true,
        } = queue.complete_ack_report(failed, &[0x15, 0]).unwrap()
        else {
            panic!("failed write ACK must start the status recovery command");
        };
        assert_eq!(completed.tag(), CommandTag::ExpansionEnableOne);
        assert_eq!(status.bytes(), &[0x15, 0]);

        assert_eq!(
            setup.acknowledge(failed),
            Some(ExpansionSetupAction::WriteEnableTwo)
        );
        let second = encode_wii_write_command(0x04a4_00fb, &[0x00]).unwrap();
        assert_eq!(
            queue.enqueue_tagged(&second, CommandTag::ExpansionEnableTwo),
            Ok(CommandEnqueue::Queued)
        );
        let Some(next) = queue.complete_response(0x15).unwrap() else {
            panic!("status completion must start the second write");
        };
        assert_eq!(
            &next.bytes()[..7],
            &[0x16, 0x04, 0xa4, 0x00, 0xfb, 0x01, 0x00]
        );

        let succeeded = AckReport {
            command: 0x16,
            status: 0,
        };
        let AckCompletion::Completed {
            completed,
            next: None,
            failed: false,
            recovery_queued: false,
        } = queue.complete_ack_report(succeeded, &[0x15, 0]).unwrap()
        else {
            panic!("second write ACK must complete before its read is queued");
        };
        assert_eq!(completed.tag(), CommandTag::ExpansionEnableTwo);
        assert_eq!(
            setup.acknowledge(succeeded),
            Some(ExpansionSetupAction::ReadCalibration)
        );
        let read = encode_wii_read_command(0x04a4_0020, 224).unwrap();
        assert_eq!(queue.enqueue(&read), Ok(CommandEnqueue::Started));
        assert_eq!(
            queue.active().unwrap().bytes(),
            &[0x17, 0x04, 0xa4, 0x00, 0x20, 0x00, 0xe0]
        );
        assert_eq!(queue.complete_response(0x17), Ok(None));
        setup.complete();
        assert!(!setup.is_active());
        assert!(queue.is_empty());
    }
}
