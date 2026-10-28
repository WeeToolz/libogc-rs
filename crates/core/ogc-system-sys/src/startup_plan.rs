//! Ordered early startup plan shared by the PowerPC runtime and host tests.

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StartupStep {
    LowMemory,
    IpcBuffers,
    LwpWorkspace,
    ResetQueue,
    Exi,
    Sram,
    Si,
    Dsp,
    MemoryProtection,
    Callbacks,
    IpcClient,
    ResetIrq,
}

impl StartupStep {
    pub const fn stage(self) -> u32 {
        match self {
            Self::LowMemory => 1,
            Self::IpcBuffers => 2,
            Self::LwpWorkspace => 3,
            Self::ResetQueue => 4,
            Self::Exi => 5,
            Self::Sram => 6,
            Self::Si => 7,
            Self::Dsp => 8,
            Self::MemoryProtection => 9,
            Self::Callbacks => 10,
            Self::IpcClient => 11,
            Self::ResetIrq => 12,
        }
    }

    const fn completion_bit(self) -> u32 {
        match self {
            Self::LowMemory => 1 << 0,
            Self::IpcBuffers => 1 << 1,
            Self::LwpWorkspace => 1 << 2,
            Self::ResetQueue => 1 << 3,
            Self::Exi => 1 << 4,
            Self::Sram => 1 << 5,
            Self::Si => 1 << 6,
            Self::Dsp => 1 << 7,
            Self::MemoryProtection => 1 << 8,
            Self::Callbacks => 1 << 9,
            Self::IpcClient | Self::ResetIrq => 1 << 10,
        }
    }
}

const WII_STEPS: [StartupStep; 11] = [
    StartupStep::LowMemory,
    StartupStep::IpcBuffers,
    StartupStep::LwpWorkspace,
    StartupStep::ResetQueue,
    StartupStep::Exi,
    StartupStep::Sram,
    StartupStep::Si,
    StartupStep::Dsp,
    StartupStep::MemoryProtection,
    StartupStep::Callbacks,
    StartupStep::IpcClient,
];

const WII_IPL_STEPS: [StartupStep; 10] = [
    StartupStep::LowMemory,
    StartupStep::IpcBuffers,
    StartupStep::LwpWorkspace,
    StartupStep::ResetQueue,
    StartupStep::Exi,
    StartupStep::Sram,
    StartupStep::Si,
    StartupStep::Dsp,
    StartupStep::Callbacks,
    StartupStep::IpcClient,
];

const GAMECUBE_STEPS: [StartupStep; 10] = [
    StartupStep::LowMemory,
    StartupStep::LwpWorkspace,
    StartupStep::ResetQueue,
    StartupStep::Exi,
    StartupStep::Sram,
    StartupStep::Si,
    StartupStep::Dsp,
    StartupStep::MemoryProtection,
    StartupStep::Callbacks,
    StartupStep::ResetIrq,
];

const GAMECUBE_IPL_STEPS: [StartupStep; 9] = [
    StartupStep::LowMemory,
    StartupStep::LwpWorkspace,
    StartupStep::ResetQueue,
    StartupStep::Exi,
    StartupStep::Sram,
    StartupStep::Si,
    StartupStep::Dsp,
    StartupStep::Callbacks,
    StartupStep::ResetIrq,
];

pub const fn steps(is_wii: bool, in_ipl: bool) -> &'static [StartupStep] {
    match (is_wii, in_ipl) {
        (true, false) => &WII_STEPS,
        (true, true) => &WII_IPL_STEPS,
        (false, false) => &GAMECUBE_STEPS,
        (false, true) => &GAMECUBE_IPL_STEPS,
    }
}

#[cfg(test)]
mod tests {
    use super::{StartupStep, run, steps};

    #[test]
    fn wii_steps_match_the_libogc_startup_order() {
        assert_eq!(
            steps(true, false),
            &[
                StartupStep::LowMemory,
                StartupStep::IpcBuffers,
                StartupStep::LwpWorkspace,
                StartupStep::ResetQueue,
                StartupStep::Exi,
                StartupStep::Sram,
                StartupStep::Si,
                StartupStep::Dsp,
                StartupStep::MemoryProtection,
                StartupStep::Callbacks,
                StartupStep::IpcClient,
            ]
        );
    }

    #[test]
    fn gc_steps_omit_wii_ipc_and_skip_memprotect_in_ipl() {
        for in_ipl in [false, true] {
            let selected = steps(false, in_ipl);
            assert_eq!(selected.last(), Some(&StartupStep::ResetIrq));
            assert!(!selected.contains(&StartupStep::IpcBuffers));
            assert_eq!(selected.contains(&StartupStep::MemoryProtection), !in_ipl);
        }
    }

    #[test]
    fn ipl_startup_skips_memory_protection_for_both_platforms() {
        for is_wii in [false, true] {
            assert!(!steps(is_wii, true).contains(&StartupStep::MemoryProtection));
        }
    }

    #[test]
    fn injected_operations_record_order_and_continue_after_sram_failure() {
        let mut order = Vec::new();
        let mut stages = Vec::new();
        let mut completed = Vec::new();
        let mut failed = Vec::new();
        let result = run(
            steps(true, false),
            |step| {
                order.push(step);
                step != StartupStep::Sram
            },
            |stage| stages.push(stage),
            |bit| completed.push(bit),
            |stage| failed.push(stage),
        );

        assert_eq!(order, steps(true, false));
        assert_eq!(stages, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(result.completed_mask, 0x7df);
        assert_eq!(result.failed_stage, 6);
        assert_eq!(failed, [6]);
        assert_eq!(completed.len(), 10);
    }

    #[test]
    fn injected_wii_operations_complete_all_startup_bits() {
        let result = run(steps(true, false), |_| true, |_| {}, |_| {}, |_| {});
        assert_eq!(result.completed_mask, 0x7ff);
        assert_eq!(result.failed_stage, 0);
    }

    #[test]
    fn injected_gamecube_operations_match_the_non_wii_completion_mask() {
        let result = run(steps(false, false), |_| true, |_| {}, |_| {}, |_| {});
        assert_eq!(result.completed_mask, 0x7fd);
        assert_eq!(result.failed_stage, 0);
    }

    #[test]
    fn injected_gamecube_ipl_operations_skip_ipc_and_memory_protection() {
        let result = run(steps(false, true), |_| true, |_| {}, |_| {}, |_| {});
        assert_eq!(result.completed_mask, 0x6fd);
        assert_eq!(result.failed_stage, 0);
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StartupResult {
    pub completed_mask: u32,
    pub failed_stage: u32,
}

/// Runs each step in order and records progress as each operation completes.
/// A failed operation records its stage and does not stop later steps. The
/// current runtime uses this for the nonfatal SRAM read failure.
pub fn run<F, S, C, E>(
    steps: &[StartupStep],
    mut operation: F,
    mut set_stage: S,
    mut mark_complete: C,
    mut mark_failed: E,
) -> StartupResult
where
    F: FnMut(StartupStep) -> bool,
    S: FnMut(u32),
    C: FnMut(u32),
    E: FnMut(u32),
{
    let mut result = StartupResult::default();
    for step in steps {
        let stage = step.stage();
        set_stage(stage);
        if operation(*step) {
            let bit = step.completion_bit();
            result.completed_mask |= bit;
            mark_complete(bit);
        } else {
            result.failed_stage = stage;
            mark_failed(stage);
        }
    }
    result
}
