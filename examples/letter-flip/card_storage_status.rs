//! Visible persistence state shared with the renderer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardStatus {
    Ready,
    NoCard,
    Error,
    Pending,
    Saving,
    Saved,
    Recovered,
}
