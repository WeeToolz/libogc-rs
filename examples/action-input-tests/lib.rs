#[cfg(test)]
#[path = "../shared/controller_input_types.rs"]
mod input_model;
#[cfg(test)]
#[path = "../action_input/action_input.rs"]
mod action_input;
#[cfg(test)]
mod controller_input {
    pub(crate) use crate::action_input;
    pub use crate::input_model::Input;
}
#[cfg(test)]
#[path = "../shared/half_controller_input.rs"]
mod half_controller_input;

#[cfg(test)]
pub use action_input::*;
