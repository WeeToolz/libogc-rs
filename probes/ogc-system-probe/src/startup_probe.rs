use ogc_system::{self as system, SystemInitStep, SystemPreMainStep};

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("STARTUP PROBE");

    let before = system::time();
    let runtime = system::snapshot_runtime_calls();
    let pre_main = system::snapshot_pre_main();
    let startup = system::snapshot_startup();
    let expected_init = system::system_init_sequence(true, false, false);
    let expected_premain = system::system_premain_sequence(true);
    let after = system::time();

    logger.note_u64("startup time before", before);
    logger.note_u64("startup time after", after);
    logger.note_u32("expected init steps", expected_init.len() as u32);
    logger.note_u32("expected premain steps", expected_premain.len() as u32);

    logger.record("startup time monotonic", after >= before);
    logger.record("runtime init called", runtime.init_called);
    logger.record("runtime pre-main called", runtime.pre_main_called);
    logger.record("runtime pre-main completed", pre_main.completed);
    logger.note_u32("runtime pre-main failed stage", pre_main.failed_stage);
    logger.note_i32("runtime pre-main result", pre_main.result);
    logger.record("low memory init called", startup.low_mem_init_called);
    logger.record(
        "wii init sequence includes ipc client",
        contains_init(expected_init, SystemInitStep::IpcClientInit),
    );
    logger.record(
        "wii pre-main sequence includes ios init",
        contains_premain(expected_premain, SystemPreMainStep::InitializeIosSubsystems),
    );
}

fn contains_init(steps: &[SystemInitStep], wanted: SystemInitStep) -> bool {
    steps.iter().copied().any(|step| step == wanted)
}

fn contains_premain(steps: &[SystemPreMainStep], wanted: SystemPreMainStep) -> bool {
    steps.iter().copied().any(|step| step == wanted)
}
