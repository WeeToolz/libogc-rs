mod powerpc;

pub use powerpc::Context as LwpContext;

pub(crate) use powerpc::{
    Context, context_switch, current_msr, current_r2, current_r13, disable_interrupts,
    enable_interrupts, in_exception, read_timebase, restore_interrupts, write_decrementer,
};
